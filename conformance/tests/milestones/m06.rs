//! M06 meta-test: completing M06 closes the R4 §10 toy — the whole
//! `phase_minus_1.rs` suite is active and no Phase -1 M2…M6 debt remains.

use camino::Utf8PathBuf;

/// The workspace root (conformance/ has a parent).
fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_owned()
}

/// `gate_minus1_0_suite_is_fully_active`: the twelve R4 §10 required results
/// all hold. Two mechanical checks:
/// (a) `phase_minus_1.rs` source contains ZERO `#[ignore` attributes; and
/// (b) `scan_workspace_debt` shows no remaining `Phase -1 M2`…`Phase -1 M6`
///     tags anywhere (M7+ tags legitimately remain — that is the ongoing
///     backlog).
#[test]
fn gate_minus1_0_suite_is_fully_active() {
    // The Phase -1 milestones whose debt must be fully cleared by M06.
    const CLOSED: &[&str] = &[
        "Phase -1 M2",
        "Phase -1 M3",
        "Phase -1 M4",
        "Phase -1 M5",
        "Phase -1 M6",
    ];

    // (a) No ignored test remains in the R4 §10 gate suite.
    let gate = repo_root().join("conformance/tests/phase_minus_1.rs");
    let src = std::fs::read_to_string(&gate).expect("read phase_minus_1.rs");
    let ignores: Vec<usize> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("#[ignore"))
        .map(|(i, _)| i + 1)
        .collect();
    assert!(
        ignores.is_empty(),
        "the R4 §10 gate suite must be fully active; #[ignore] still at lines {ignores:?}"
    );

    // (b) No Phase -1 M2…M6 debt tag remains anywhere in the workspace.
    let report =
        liminal_conformance::scan_workspace_debt(&repo_root()).expect("scan workspace debt");
    let lingering: Vec<String> = report
        .ignored_by_phase
        .keys()
        .filter(|tag| CLOSED.contains(&tag.as_str()))
        .cloned()
        .collect();
    assert!(
        lingering.is_empty(),
        "Phase -1 M2…M6 are complete; no such debt tag may remain, found: {lingering:?}"
    );
}
