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
