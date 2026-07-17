//! M01 exit gate: crash-injector lethality and occurrence-counting proofs.
//!
//! These tests spawn `liminald __fire` as a subprocess, arm the crash
//! injector via `LIMINAL_CRASHPOINT`, and assert real SIGABRT death plus
//! durable hit tracing. They run in the nextest serialized crash group
//! (`test(/^crash_/)`).

use std::process::Command;

/// SIGABRT signal number on Unix (Phase -1 targets Linux/macOS;
/// Windows crash semantics are a Phase 2 concern).
const SIGABRT: i32 = 6;

/// Scratch dir keyed by test name + pid to avoid collisions in parallel runs.
fn scratch_dir(label: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("liminal-crash-test-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn crash_fault_abort_exits_with_sigabrt() {
    let dir = scratch_dir("sigabrt");
    let trace = dir.join("crash-trace.log");

    let status = Command::new(env!("CARGO_BIN_EXE_liminald"))
        .args(["__fire", "ilrp/after_intent_commit"])
        .env("LIMINAL_CRASHPOINT", "ilrp/after_intent_commit")
        .env("LIMINAL_CRASH_TRACE", &trace)
        .status()
        .unwrap();

    assert!(!status.success(), "must die, not exit 0");
    assert!(
        status.code().is_none(),
        "must die by signal, not exit code (got {:?})",
        status.code()
    );
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(SIGABRT),
        "must die by SIGABRT"
    );

    // The hit was durably traced BEFORE the abort.
    let contents = std::fs::read_to_string(&trace).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines,
        vec!["ilrp/after_intent_commit:1"],
        "trace must record exactly the armed boundary before abort"
    );
}

#[test]
fn crash_fault_unarmed_hit_traces_and_exits_cleanly() {
    let dir = scratch_dir("unarmed");
    let trace = dir.join("crash-trace.log");

    let output = Command::new(env!("CARGO_BIN_EXE_liminald"))
        .args(["__fire", "ilrp/after_ack"])
        .env_remove("LIMINAL_CRASHPOINT")
        .env("LIMINAL_CRASH_TRACE", &trace)
        .output()
        .unwrap();

    assert!(output.status.success(), "unarmed must exit 0");
    assert!(
        output.stdout.is_empty(),
        "unarmed must write nothing to stdout"
    );
    assert!(
        output.stderr.is_empty(),
        "unarmed must write nothing to stderr"
    );

    let contents = std::fs::read_to_string(&trace).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines,
        vec!["ilrp/after_ack:1"],
        "trace must record the hit even when unarmed"
    );
}

#[test]
fn crash_fault_occurrence_arming_counts_hits() {
    let dir = scratch_dir("occurrence");
    let trace = dir.join("crash-trace.log");

    // Fire 3 times, armed to abort on the 2nd occurrence.
    let status = Command::new(env!("CARGO_BIN_EXE_liminald"))
        .args(["__fire", "ilrp/after_external_apply", "3"])
        .env("LIMINAL_CRASHPOINT", "ilrp/after_external_apply:2")
        .env("LIMINAL_CRASH_TRACE", &trace)
        .status()
        .unwrap();

    assert!(!status.success(), "must die on the 2nd occurrence");
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(SIGABRT),
        "must die by SIGABRT"
    );

    // Only 2 hits recorded — the 3rd was never reached.
    let contents = std::fs::read_to_string(&trace).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines,
        vec!["ilrp/after_external_apply:1", "ilrp/after_external_apply:2"],
        "trace must record hits 1 and 2; hit 3 never reached"
    );
}
