//! Append-only checksummed NDJSON segments (v4 §92: "append graph operations
//! atomically, use checksummed segments, recover after interrupted writes").
//!
//! Record framing, one per line:
//!
//! ```text
//! <crc32-hex-8> <canonical JSON of CommitRecord>\n
//! ```
//!
//! `append` fsyncs before returning — the ILRP fault points sit immediately
//! after these returns, so a kill between them is exactly a kill between
//! durable boundaries. Recovery accepts the longest checksummed prefix: a torn
//! tail is truncated; corruption *before* the tail is an error, never silently
//! skipped.

use std::fs;
use std::io::Write as _;

use camino::{Utf8Path, Utf8PathBuf};

use super::{CommitRecord, StoreError};

fn segment_path(dir: &Utf8Path, segment: u64) -> Utf8PathBuf {
    dir.join(format!("log.{segment:06}.ndjson"))
}

fn list_segments(dir: &Utf8Path, base: u64) -> Result<Vec<(u64, Utf8PathBuf)>, StoreError> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Some(rest) = name.strip_prefix("log.")
            && let Some(num) = rest.strip_suffix(".ndjson")
            && let Ok(n) = num.parse::<u64>()
            && n >= base
        {
            found.push((n, dir.join(&name)));
        }
    }
    found.sort_unstable_by_key(|(n, _)| *n);
    Ok(found)
}

/// The active append handle over the newest segment.
#[derive(Debug)]
pub(crate) struct SegmentLog {
    file: fs::File,
    segment: u64,
}

impl SegmentLog {
    /// Create a fresh empty segment (used at store creation and after a
    /// snapshot rotation).
    pub(crate) fn create(dir: &Utf8Path, segment: u64) -> Result<Self, StoreError> {
        let path = segment_path(dir, segment);
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        file.sync_all()?;
        sync_dir(dir)?;
        Ok(Self { file, segment })
    }

    /// Replay all segments numbered `>= base` in order, feeding each valid
    /// record to `apply`. A torn tail on the final segment is truncated
    /// (longest checksummed prefix); anything invalid earlier is
    /// [`StoreError::Corrupt`]. Returns the append handle over the last
    /// segment (creating segment `base` for a fresh store).
    pub(crate) fn recover(
        dir: &Utf8Path,
        base: u64,
        mut apply: impl FnMut(&CommitRecord) -> Result<(), String>,
    ) -> Result<Self, StoreError> {
        let segments = list_segments(dir, base)?;
        if segments.is_empty() {
            return Self::create(dir, base);
        }

        let last_index = segments.len() - 1;
        for (i, (number, path)) in segments.iter().enumerate() {
            let bytes = fs::read(path)?;
            let is_last_segment = i == last_index;
            if let Some(torn_at) = replay_segment(&bytes, is_last_segment, path, &mut apply)? {
                // Longest checksummed prefix: drop the torn tail (v4 §92).
                let file = fs::OpenOptions::new().write(true).open(path)?;
                file.set_len(torn_at)?;
                file.sync_all()?;
            }
            let _ = number;
        }

        let (segment, path) = segments[last_index].clone();
        let file = fs::OpenOptions::new().append(true).open(&path)?;
        Ok(Self { file, segment })
    }

    /// Append one record and fsync. The return of this function IS a durable
    /// boundary (v4 §92); nothing is buffered.
    pub(crate) fn append(&mut self, record: &CommitRecord) -> Result<(), StoreError> {
        let json = serde_json::to_string(record)
            .map_err(|e| StoreError::Corrupt(format!("unserializable record: {e}")))?;
        let crc = crc32fast::hash(json.as_bytes());
        let line = format!("{crc:08x} {json}\n");
        self.file.write_all(line.as_bytes())?;
        self.file.sync_all()?;
        Ok(())
    }

    /// The active segment number.
    pub(crate) fn current_segment(&self) -> u64 {
        self.segment
    }
}

/// Read-only replay of every retained segment from genesis (segment 0),
/// feeding each valid record to `apply` in revision order (M08.1 as-of reads).
/// Unlike [`SegmentLog::recover`] this NEVER mutates the log: a torn tail on the
/// final segment is simply ignored (the truncation offset is discarded), and the
/// snapshot is bypassed entirely — the toy never GCs segments, so replaying the
/// raw segments always reconstructs history from genesis (D08.1). `apply` may
/// choose to ignore records beyond a target revision.
pub(crate) fn replay_from_genesis(
    dir: &Utf8Path,
    mut apply: impl FnMut(&CommitRecord) -> Result<(), String>,
) -> Result<(), StoreError> {
    let segments = list_segments(dir, 0)?;
    let last_index = segments.len().saturating_sub(1);
    for (i, (_number, path)) in segments.iter().enumerate() {
        let bytes = fs::read(path)?;
        // Reuse the record framing/checksum parser; discard any torn-tail offset
        // (read-only replay never truncates).
        let _torn = replay_segment(&bytes, i == last_index, path, &mut apply)?;
    }
    Ok(())
}

/// Replay one segment's bytes. Returns `Ok(Some(offset))` if a torn tail
/// begins at `offset` and should be truncated (only permitted on the final
/// segment); `Ok(None)` if the segment replayed cleanly.
fn replay_segment(
    bytes: &[u8],
    is_last_segment: bool,
    path: &Utf8Path,
    apply: &mut impl FnMut(&CommitRecord) -> Result<(), String>,
) -> Result<Option<u64>, StoreError> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let rest = &bytes[offset..];
        let Some(nl) = rest.iter().position(|&b| b == b'\n') else {
            // Unterminated final chunk: a torn tail iff this is the last segment.
            return torn_or_corrupt(is_last_segment, true, offset, path);
        };
        let line = &rest[..nl];
        match parse_line(line) {
            Ok(record) => {
                apply(&record).map_err(|e| {
                    StoreError::Corrupt(format!("{path}: record at byte {offset} rejected: {e}"))
                })?;
                offset += nl + 1;
            }
            Err(reason) => {
                // A torn write can only damage the tail: an invalid line with
                // MORE content after it is corruption, never truncation.
                let has_content_after = bytes[offset + nl + 1..]
                    .iter()
                    .any(|b| !b.is_ascii_whitespace());
                if has_content_after {
                    return Err(StoreError::Corrupt(format!(
                        "{path}: invalid record at byte {offset} ({reason}) with valid data after it"
                    )));
                }
                return torn_or_corrupt(is_last_segment, false, offset, path);
            }
        }
    }
    Ok(None)
}

fn torn_or_corrupt(
    is_last_segment: bool,
    unterminated: bool,
    offset: usize,
    path: &Utf8Path,
) -> Result<Option<u64>, StoreError> {
    if is_last_segment {
        Ok(Some(offset as u64))
    } else {
        let kind = if unterminated {
            "unterminated"
        } else {
            "invalid"
        };
        Err(StoreError::Corrupt(format!(
            "{path}: {kind} record at byte {offset} in a non-final segment"
        )))
    }
}

fn parse_line(line: &[u8]) -> Result<CommitRecord, String> {
    let text = std::str::from_utf8(line).map_err(|_| "non-UTF-8 line".to_owned())?;
    let (crc_hex, json) = text
        .split_once(' ')
        .ok_or_else(|| "missing checksum field".to_owned())?;
    let expected = u32::from_str_radix(crc_hex, 16).map_err(|_| "bad checksum hex".to_owned())?;
    let actual = crc32fast::hash(json.as_bytes());
    if expected != actual {
        return Err(format!("checksum mismatch: {expected:08x} != {actual:08x}"));
    }
    serde_json::from_str(json).map_err(|e| format!("bad record JSON: {e}"))
}

pub(crate) fn sync_dir(dir: &Utf8Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir; // directory fsync is not portable off Unix
    Ok(())
}
