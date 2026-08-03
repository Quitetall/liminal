//! M23 exit-gate tests: the fuzz corpus and the benchmark gate.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs`.
//!
//! ## One declared test is deliberately absent
//!
//! The packet also names `benchmark_gate_fails_on_regression_beyond_threshold`.
//! `liminal_xtask::bench::gate_repo` exists and does compare a candidate
//! against a baseline at a 10% median / 15% p95 threshold — but
//! `benches/baselines/` contains only `phase1-candidate.json`. There is no
//! `phase1.json` baseline to regress against, so M23 has not yet produced the
//! artifact the gate gates on.
//!
//! It is not written here for the same reason the three `lim fmt` tests are
//! absent from `m20.rs`: a test whose subject is incomplete can only assert a
//! tautology, and a tautology that passes is worse than a gap that is
//! recorded.

use camino::Utf8PathBuf;
use liminal_format::{Formatter, MarkdownFormatter};

fn corpus_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpora/regression/phase1")
}

fn minimized_artifacts() -> Vec<(Utf8PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![corpus_root()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                stack.push(path);
            } else if path.extension() == Some("bin")
                && let Ok(bytes) = std::fs::read(&path)
            {
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

/// D23: every minimized artifact in the corpus replays without panicking.
///
/// The corpus is enumerated, never listed: M17.5 found 23 minimized artifacts
/// sitting in `fuzz/artifacts/` while only 2 had been promoted, and a hardcoded
/// list means the next promotion is replayed by nobody.
#[test]
#[ignore = "Phase 1: M23 fuzz corpus (AM-17.2 quarantine)"]
fn fuzz_corpus_replays_every_minimized_artifact_without_panic() {
    let artifacts = minimized_artifacts();
    assert!(
        artifacts.len() >= 26,
        "the regression corpus has shrunk to {}; a corpus that quietly empties reports clean",
        artifacts.len()
    );
    let formatter = MarkdownFormatter::default();
    for (path, bytes) in artifacts {
        // Lossy conversion, matching the fuzz target. A replay harness that
        // skips invalid UTF-8 skips exactly the inputs the fuzzer found.
        let source = String::from_utf8_lossy(&bytes);
        let document = formatter
            .parse(&source)
            .unwrap_or_else(|e| panic!("{path}: parser must be total: {e:?}"));
        formatter
            .emit(&document)
            .unwrap_or_else(|e| panic!("{path}: emission must be total: {e:?}"));
    }
}

/// D23: replay is deterministic — the same artifact yields the same result
/// every time.
///
/// A regression corpus that replays nondeterministically cannot distinguish a
/// fix from a coincidence.
#[test]
#[ignore = "Phase 1: M23 fuzz corpus (AM-17.2 quarantine)"]
fn fuzz_replay_is_deterministic_from_recorded_seeds() {
    let formatter = MarkdownFormatter::default();
    for (path, bytes) in minimized_artifacts() {
        let source = String::from_utf8_lossy(&bytes);
        let first = formatter
            .emit(&formatter.parse(&source).expect("parse"))
            .expect("emit");
        for run in 1..4 {
            let again = formatter
                .emit(&formatter.parse(&source).expect("parse"))
                .expect("emit");
            assert_eq!(first, again, "{path}: replay differed on run {run}");
        }
    }
}

/// D23: every fuzz regression records the source basis that produced it.
///
/// The corpus convention is the explicit-syntax marker, which is what makes an
/// artifact replayable against the surface it was minimized from. An artifact
/// without it is a pile of bytes nobody can attribute.
#[test]
#[ignore = "Phase 1: M23 fuzz corpus (AM-17.2 quarantine)"]
fn fuzz_regressions_record_the_basis_that_produced_them() {
    for (path, bytes) in minimized_artifacts() {
        assert!(
            !bytes.is_empty(),
            "{path} is empty; an empty artifact reproduces nothing"
        );
        assert!(
            bytes.starts_with(b"#!liminal-explicit-v1"),
            "{path} carries no explicit-syntax marker, so the surface it was \
             minimized from is unrecoverable"
        );
    }
}
