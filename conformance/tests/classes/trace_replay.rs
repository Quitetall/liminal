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
/// The golden is regenerated deliberately with `BLESS_DENOMINATOR_COUNTS=1`
/// and reviewed line-by-line (a golden is spec; M11.3 is a T1 act) —
/// changing a count afterward is a corpus-version bump, never an edit.
#[test]
fn denominator_counts_golden() {
    use std::fmt::Write as _;
    let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut rendered = String::from(
        "# §7.4 denominator counts — hand-labeled traces (M11.3, FROZEN)\n\n\
         Regenerate deliberately with BLESS_DENOMINATOR_COUNTS=1; a change\n\
         here is a corpus-version bump, never an edit (v4 §7.4; POLICY.md).\n",
    );
    for name in [
        "coalesce-basic",
        "session-timeout",
        "git-rootcause",
        "offline-transient",
    ] {
        let path = root.join(format!("fixtures/traces/labeled/{name}.trace.ndjson"));
        let ndjson = std::fs::read_to_string(&path).expect("labeled fixture exists");
        let trace = liminal_conformance::trace::Trace::parse(&ndjson).expect("fixture parses");
        let counts = liminal_conformance::denominator::count(&trace.events);
        let _ = write!(
            rendered,
            "\n## {name}\n\nops: {}\ntxns: {}\nsessions: {}\n",
            counts.ops, counts.txns, counts.sessions
        );
        if counts.incidents_by_cause.is_empty() {
            rendered.push_str("incidents: none\n");
        } else {
            for (cause, n) in &counts.incidents_by_cause {
                let _ = writeln!(rendered, "incident: {cause} = {n}");
            }
        }
    }

    let golden_path = root.join("golden/denominator_counts.md");
    if std::env::var("BLESS_DENOMINATOR_COUNTS").is_ok() || !golden_path.exists() {
        std::fs::write(&golden_path, &rendered).expect("write golden");
    }
    let golden = std::fs::read_to_string(&golden_path).expect("read denominator_counts.md");
    assert_eq!(
        rendered, golden,
        "denominator counts drifted from the frozen golden — the D11.1-D11.3 \
         constants are spec; if this is a deliberate corpus-version bump, \
         regenerate with BLESS_DENOMINATOR_COUNTS=1 and review the diff"
    );
}

/// -1.5: the full pipeline — importers, replay, denominators, scorecard —
/// runs end-to-end over the DEV corpus and emits the six per-profile metrics
/// (`liminal_conformance::Scorecard`).
#[test]
#[ignore = "Phase -1 M11: end-to-end conformance scorecard"]
fn scorecard_runs_end_to_end() {
    unimplemented!("run pipeline over corpora dev split; assert Scorecard shape per profile")
}
