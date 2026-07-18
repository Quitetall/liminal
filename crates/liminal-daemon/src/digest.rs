//! World digest (M02 Algorithm E): a byte-exact hash of the observable world.
//!
//! Used by the crash matrix to assert recovery idempotence: two recoveries of
//! the same workspace must produce the same digest.

use std::collections::BTreeMap;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use liminal_graph::{GraphStore, ns};
use liminal_id::ContentHash;

/// Hash every regular non-excluded file under `root`, sorted by relative path.
pub fn workspace_file_hashes(root: &Utf8Path) -> io::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                // Skip the store subtree.
                if path.file_name() == Some("state") {
                    continue;
                }
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_string();
                // Exclude harness files.
                if rel == "crash-trace.log" || rel == "recover-trace.log" || rel == "scenario.toml"
                {
                    continue;
                }
                let bytes = std::fs::read(&path)?;
                out.insert(rel, ContentHash::of(&bytes).to_hex());
            }
        }
    }
    Ok(out)
}

/// Full world digest: store head + file hashes + intent records + overlay keys.
pub fn world_digest(
    root: &Utf8Path,
    store: &GraphStore,
) -> Result<String, liminal_graph::StoreError> {
    use std::fmt::Write as _;

    let mut h = blake3::Hasher::new();
    h.update(b"liminal-world-digest/v1\n");

    let head = store.head()?;
    h.update(format!("head:{}\n", head.0).as_bytes());

    let files = workspace_file_hashes(root)
        .map_err(|e| liminal_graph::StoreError::Corrupt(e.to_string()))?;
    for (rel, hex) in &files {
        h.update(format!("file:{rel}:{hex}\n").as_bytes());
    }

    let intents = store.scan_aux(ns::ILRP_INTENT)?;
    for (key, value) in &intents {
        let json = serde_json::to_string(value).unwrap_or_default();
        h.update(format!("intent:{key}:{json}\n").as_bytes());
    }

    let overlays = store.scan_aux(ns::JUR_OVERLAY)?;
    for (key, _) in &overlays {
        h.update(format!("overlay:{key}\n").as_bytes());
    }

    let mut hex = String::with_capacity(64);
    for b in h.finalize().as_bytes() {
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

/// A NORMALIZED world digest (M04 Algorithm F): a canonical dump of files +
/// graph + all `jur.*` aux, with every UUID replaced by a first-appearance
/// ordinal and volatile timestamp fields dropped. Two workspaces that differ
/// only by minted ids and clocks produce the SAME normalized digest — the
/// basis for the promotion-equivalence check (drive `save` vs. hand-build the
/// identical plan; the worlds must be observationally equal).
pub fn normalized_world_digest(
    root: &Utf8Path,
    store: &GraphStore,
) -> Result<String, liminal_graph::StoreError> {
    use std::fmt::Write as _;

    let mut dump = String::new();

    // Files (already content-addressed, no ids to normalize).
    let files = workspace_file_hashes(root)
        .map_err(|e| liminal_graph::StoreError::Corrupt(e.to_string()))?;
    for (rel, hex) in &files {
        let _ = writeln!(dump, "file:{rel}:{hex}");
    }

    // Graph nodes + relations, canonical JSON in id order.
    for node in store.nodes()? {
        let _ = writeln!(
            dump,
            "node:{}",
            serde_json::to_string(&node).unwrap_or_default()
        );
    }
    for relation in store.relations()? {
        let _ = writeln!(
            dump,
            "relation:{}",
            serde_json::to_string(&relation).unwrap_or_default()
        );
    }

    // All jurisdiction aux namespaces (plans, decisions, repairs, aliases,
    // intents). Keyed and value-serialized; ids inside get normalized below.
    for ns_name in [
        ns::JUR_PLAN,
        ns::JUR_DECISION,
        ns::JUR_REPAIR,
        ns::JUR_ALIAS,
        ns::ILRP_INTENT,
    ] {
        for (key, value) in store.scan_aux(ns_name)? {
            let _ = writeln!(
                dump,
                "aux:{ns_name}:{key}:{}",
                serde_json::to_string(&value).unwrap_or_default()
            );
        }
    }

    let normalized = normalize_ids_and_drop_timestamps(&dump);
    Ok(ContentHash::of(normalized.as_bytes()).to_hex())
}

/// Replace every UUID with a first-appearance ordinal (`<id:0>`, `<id:1>`, …)
/// and drop volatile timestamp fields (`at`/`created_at`/`last_activity`/
/// `observed_at`) so two worlds that differ only by minted ids + clocks hash
/// equally.
fn normalize_ids_and_drop_timestamps(dump: &str) -> String {
    use std::fmt::Write as _;
    // 1. Drop timestamp fields: `"at":<...>` etc. up to the next comma or brace.
    //    Timestamps serialize as `{"0":<u128>}` or a number; a coarse strip that
    //    removes `"<field>":<balanced-or-scalar>` keeps the digest stable
    //    because both worlds carry the same field shape.
    let stripped = strip_timestamp_fields(dump);

    // 2. Normalize UUIDs by first appearance.
    let mut out = String::with_capacity(stripped.len());
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let bytes = stripped.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(uuid) = uuid_at(&stripped, i) {
            let n = seen.len();
            let ord = *seen.entry(uuid.clone()).or_insert(n);
            let _ = write!(out, "<id:{ord}>");
            i += uuid.len();
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// A canonical 36-char UUID starting at byte `i`, if present.
fn uuid_at(s: &str, i: usize) -> Option<String> {
    let bytes = s.as_bytes();
    if i + 36 > bytes.len() {
        return None;
    }
    let candidate = &s[i..i + 36];
    let ok = candidate.chars().enumerate().all(|(j, c)| match j {
        8 | 13 | 18 | 23 => c == '-',
        _ => c.is_ascii_hexdigit(),
    });
    // Reject if preceded/followed by another hex digit (partial match).
    let boundary_ok = (i == 0 || !bytes[i - 1].is_ascii_hexdigit())
        && (i + 36 == bytes.len() || !bytes[i + 36].is_ascii_hexdigit());
    (ok && boundary_ok).then(|| candidate.to_owned())
}

/// Remove `"<field>":<value>` for the volatile timestamp fields, treating the
/// value as everything up to the next top-level `,` or `}`.
fn strip_timestamp_fields(s: &str) -> String {
    const FIELDS: &[&str] = &[
        "\"at\":",
        "\"created_at\":",
        "\"last_activity\":",
        "\"observed_at\":",
    ];
    let mut out = s.to_owned();
    for field in FIELDS {
        while let Some(pos) = out.find(field) {
            // Find the end of the value: scan balancing braces/brackets.
            let val_start = pos + field.len();
            let end = value_end(&out, val_start);
            // Drop a trailing comma if the field was followed by one; else drop
            // a leading comma.
            let mut cut_start = pos;
            let mut cut_end = end;
            if out.as_bytes().get(end) == Some(&b',') {
                cut_end = end + 1;
            } else if pos > 0 && out.as_bytes()[pos - 1] == b',' {
                cut_start = pos - 1;
            }
            out.replace_range(cut_start..cut_end, "");
        }
    }
    out
}

/// The byte index just past a JSON value starting at `start` (scalar or a
/// balanced object/array).
fn value_end(s: &str, start: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = start;
    let mut depth = 0i32;
    let mut in_str = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    if depth == 0 {
                        return i;
                    }
                    depth -= 1;
                }
                b',' if depth == 0 => return i,
                _ => {}
            }
        }
        i += 1;
    }
    i
}
