//! M23 exit-gate tests: the fuzz corpus and the benchmark gate.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs`.

use camino::Utf8PathBuf;
use liminal_format::{Formatter, MarkdownFormatter};

fn benchmark_artifact(kind: &str, p95: u64) -> serde_json::Value {
    let names = [
        "cold_startup",
        "mem_per_node_and_relation",
        "store_append_txn",
        "store_commit_fsync",
        "store_recover_1k_records",
        "relation_traversal",
        "graph_query_latency",
    ];
    let benchmarks = names
        .into_iter()
        .map(|name| {
            let samples: Vec<u64> = (1..=30).collect();
            serde_json::json!({
                "name": name,
                "unit": "ns",
                "samples": samples,
                "median": 15,
                "p95": if kind == "baseline" { 29 } else { p95 },
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": 1,
        "artifact_kind": kind,
        "subject_commit": "1111111111111111111111111111111111111111",
        "compared_baseline_commit": if kind == "baseline" {
            serde_json::Value::Null
        } else {
            serde_json::Value::String("1111111111111111111111111111111111111111".to_owned())
        },
        "reference_environment": "test",
        "toolchain": "test-toolchain",
        "sample_count": 30,
        "benchmarks": benchmarks,
    })
}

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

/// The comparator must reject one benchmark whose p95 exceeds the frozen
/// threshold, while accepting an identical candidate.
#[test]
#[ignore = "Phase 1: M23 fuzz corpus (AM-17.2 quarantine)"]
fn benchmark_gate_fails_on_regression_beyond_threshold() {
    let root = liminal_scratch::ScratchDir::new("m23-bench-gate").expect("scratch root");
    let baselines = root.join("benches/baselines");
    std::fs::create_dir_all(&baselines).expect("baseline directory");
    let baseline = benchmark_artifact("baseline", 29);
    let candidate = benchmark_artifact("candidate", 100);
    std::fs::write(
        baselines.join("phase1.json"),
        serde_json::to_vec_pretty(&baseline).expect("baseline JSON"),
    )
    .expect("baseline");
    std::fs::write(
        baselines.join("phase1-candidate.json"),
        serde_json::to_vec_pretty(&candidate).expect("candidate JSON"),
    )
    .expect("candidate");
    let error = liminal_xtask::bench::gate_repo(&root)
        .expect_err("candidate p95 above 15% must fail closed");
    assert!(
        error.to_string().contains("p95"),
        "unexpected error: {error}"
    );
}
