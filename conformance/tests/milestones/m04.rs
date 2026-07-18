//! M04 exit-gate born-passing test: the `lim repairs` golden (Algorithm E).
//!
//! (The two-step-DAG crash matrix lives in `tests/crash.rs` as
//! `crash_matrix_two_step_dag` — crash_ prefix → serialized nextest group.)

use liminal_conformance::harness::{ToyRun, all_scenarios};

/// `lim repairs` output golden (M04 Algorithm E, D04.8 — no timestamps).
/// Drive one accepted disjoint save (undo available) and, in the same run, an
/// accepted DAG repair; snapshot the two record lines with UUIDs redacted.
#[test]
fn repairs_output_golden() {
    let scenarios = all_scenarios().expect("scenarios");

    // A disjoint save → one accepted "save-promotion" record (undo available).
    let disjoint = scenarios
        .iter()
        .find(|s| s.scenario.id == "disjoint_safe_repair")
        .expect("disjoint_safe_repair");
    let run = ToyRun::new("repairs-golden").expect("workspace");
    run.exec_scenario(disjoint).expect("exec");

    let out = run.lim(&["repairs"]).expect("lim repairs runs");
    assert!(out.status.success(), "lim repairs exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Redact the UUID so the golden is stable (a golden is spec — T1).
    let redacted = redact_uuids(&stdout);
    insta::assert_snapshot!(redacted);
}

/// Replace every `repair:<uuid>` (and bare uuids) with `repair:<id>`.
fn redact_uuids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 36 <= bytes.len() && is_uuid(&s[i..i + 36]) {
            out.push_str("<id>");
            i += 36;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn is_uuid(s: &str) -> bool {
    s.chars().enumerate().all(|(j, c)| match j {
        8 | 13 | 18 | 23 => c == '-',
        _ => c.is_ascii_hexdigit(),
    })
}
