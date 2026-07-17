//! Full-state snapshots with atomic replacement (v4 §92: "write snapshots
//! with atomic replacement"): `snapshot.json.tmp → fsync → rename →
//! fsync(dir)`. Old segments are retained; the toy never garbage-collects.

use std::fs;
use std::io::Write as _;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use super::log::sync_dir;
use super::{State, StoreError};

const SNAPSHOT: &str = "snapshot.json";
const SNAPSHOT_TMP: &str = "snapshot.json.tmp";

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotFile {
    /// Replay resumes from segments numbered `>= segment`.
    segment: u64,
    state: State,
}

/// Load the snapshot if present. A leftover `.tmp` from a crash mid-snapshot
/// is ignored (the old snapshot, or an empty state plus full segment replay,
/// remains authoritative). A *corrupt* committed snapshot is an error — the
/// store never guesses (v4 §7.8 step 5).
pub(crate) fn load(dir: &Utf8Path) -> Result<(State, u64), StoreError> {
    let path = dir.join(SNAPSHOT);
    match fs::read(&path) {
        Ok(bytes) => {
            let snap: SnapshotFile = serde_json::from_slice(&bytes)
                .map_err(|e| StoreError::Corrupt(format!("{path}: bad snapshot: {e}")))?;
            Ok((snap.state, snap.segment))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((State::default(), 0)),
        Err(e) => Err(e.into()),
    }
}

/// Write a snapshot atomically. `next_segment` is where replay resumes after
/// this snapshot.
pub(crate) fn write(dir: &Utf8Path, state: &State, next_segment: u64) -> Result<(), StoreError> {
    let snap = SnapshotFile {
        segment: next_segment,
        state: state.clone(),
    };
    let json = serde_json::to_string_pretty(&snap)
        .map_err(|e| StoreError::Corrupt(format!("unserializable snapshot: {e}")))?;

    let tmp = dir.join(SNAPSHOT_TMP);
    let mut f = fs::File::create(&tmp)?;
    f.write_all(json.as_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, dir.join(SNAPSHOT))?;
    sync_dir(dir)?;
    Ok(())
}
