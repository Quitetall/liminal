//! §7.4/§114/-1.5 held-out trace replay class. The manifest lock is REAL
//! today; denominators and the scorecard pipeline are M11.

use camino::Utf8PathBuf;

fn heldout_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpora/heldout")
}

/// REAL today: every versioned held-out corpus directory (`heldout/v<N>/`)
/// must carry a `MANIFEST.b3` (BLAKE3) that exactly matches its tree. Any
/// drift fails the build — tuning against locked corpora is a process
/// violation (v4 §7.4; R4 §2.2). Vacuously green until M11 lands `v1/`.
#[test]
fn heldout_manifest_locked() {
    let root = heldout_root();
    let mut versions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            let name = path.file_name().unwrap_or_default();
            if path.is_dir() && name.starts_with('v') {
                versions.push(path);
            }
        }
    }
    for version_dir in versions {
        liminal_conformance::harness::verify_heldout_manifest(&version_dir)
            .expect("locked held-out corpus must match its manifest");
    }
}

/// -1.5: hand-labeled traces produce the EXACT expected Jurisdiction-sensitive
/// operation and session counts — the denominators are frozen BEFORE any
/// profile tuning (v4 §7.4: keystroke coalescing into semantic transactions,
/// 30-minute session timeout, root-cause incident coalescing).
#[test]
#[ignore = "Phase -1 M11: trace format + §7.4 denominator implementation"]
fn denominator_counts_golden() {
    unimplemented!("replay hand-labeled traces; compare counts to conformance/golden/")
}

/// -1.5: the full pipeline — importers, replay, denominators, scorecard —
/// runs end-to-end over the DEV corpus and emits the six per-profile metrics
/// (`liminal_conformance::Scorecard`).
#[test]
#[ignore = "Phase -1 M11: end-to-end conformance scorecard"]
fn scorecard_runs_end_to_end() {
    unimplemented!("run pipeline over corpora dev split; assert Scorecard shape per profile")
}
