//! M02 crash-matrix tests: derived crash matrix, recovery-never-guesses,
//! and meta-test that every fault point fires in some baseline.
//!
//! These run serialized (crash_ prefix → nextest crash group).

use liminal_conformance::harness::{ToyRun, runnable_crash_scenarios};

#[test]
fn crash_matrix_promote_single_step() {
    let scenarios = runnable_crash_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "promote_single_step")
        .expect("promote_single_step scenario must exist");

    ToyRun::crash_matrix(scenario).expect("crash matrix must pass for promote_single_step");
}

#[test]
fn crash_matrix_two_step_dag() {
    // The full two-step-DAG crash matrix (M04): kill at every ILRP boundary of
    // the `dag_accept` scenario (ID-insert file step, then RetargetRelation at
    // Finalize), recover, and assert Committed with idempotent double recovery.
    let scenarios = runnable_crash_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "dag_accept")
        .expect("dag_accept scenario must exist");

    ToyRun::crash_matrix(scenario).expect("crash matrix must pass for dag_accept");
}

#[test]
fn crash_recovery_never_guesses() {
    let scenarios = runnable_crash_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "promote_single_step")
        .expect("promote_single_step scenario must exist");

    // Baseline to get the trace.
    let baseline = ToyRun::new("never-guesses-baseline").expect("must create workspace");
    let trace = baseline.baseline(scenario).expect("baseline must pass");

    // Find the after_external_apply point.
    let apply_point = trace
        .hits
        .iter()
        .find(|(name, _)| name == "ilrp/after_external_apply")
        .expect("baseline must hit after_external_apply");

    let run = ToyRun::new("never-guesses-crash").expect("must create workspace");
    let _crash_state = run
        .run_to_crash(scenario, &apply_point.0, apply_point.1)
        .expect("must crash at after_external_apply");

    // Third-party mutation: write contents that match neither pre nor post hash.
    let target = run.root.join("notes.md");
    std::fs::write(&target, b"THIRD PARTY CONTENT THAT MATCHES NEITHER").expect("must write");

    // Recovery must land in NeedsReview.
    let r1 = run.recover().expect("recovery must succeed");
    assert_eq!(r1.terminals.len(), 1, "one intent expected");
    assert!(
        r1.terminals[0] == liminal_jurisdiction::IntentState::NeedsReview,
        "contested state must land in NeedsReview, got {:?}",
        r1.terminals[0]
    );

    // File bytes must be preserved (the third-party content).
    let file_bytes = std::fs::read(&target).expect("must read");
    assert_eq!(
        file_bytes, b"THIRD PARTY CONTENT THAT MATCHES NEITHER",
        "zero bytes lost — file preserved as-is"
    );

    // No staged files remain.
    let staged = liminal_source::scan_staged(&run.root).expect("must scan");
    assert!(staged.is_empty(), "no staged files should remain");

    // Recovery idempotence.
    let r2 = run.recover().expect("second recovery must succeed");
    assert_eq!(r2.world_digest, r1.world_digest, "digests must match");
    assert_eq!(r2.terminals, r1.terminals, "terminals must match");
}

#[test]
fn crash_meta_every_point_fires_in_some_baseline() {
    let scenarios = runnable_crash_scenarios().expect("must load scenarios");

    // Collect all points from all baselines.
    let mut all_points = std::collections::BTreeSet::new();
    for scenario in &scenarios {
        let run =
            ToyRun::new(&format!("meta-{}", scenario.scenario.id)).expect("must create workspace");
        let trace = run.baseline(scenario).expect("baseline must pass");
        for (name, occ) in &trace.hits {
            all_points.insert(format!("{name}:{occ}"));
        }
    }

    // Every registered crash point must appear in at least one baseline.
    for point in liminal_jurisdiction::CrashPoint::all() {
        // Check that at least one occurrence of this point fired.
        let found = all_points.iter().any(|key| key.starts_with(point.name()));
        assert!(
            found,
            "crash point {} never fired in any baseline — dead spec",
            point.name()
        );
    }
}
