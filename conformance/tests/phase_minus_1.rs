//! The executable Phase -1 exit gate (R4 §10) — one test per gate bullet.
//!
//! Every test here is the DEFINITION OF DONE for its milestone
//! (docs/implementation-plan.md). Un-ignoring a test means implementing it
//! for real against the harness; the assertion each must make is spelled out
//! in its doc comment and must not be weakened.

/// R4 §10: "Sound states are silent." Run every step of the sound-session
/// scenario, then `lim check`: exit 0 with BYTE-EMPTY stdout and stderr
/// (`liminal_conformance::assert_silent`), zero reconciliation items, zero
/// overlays. "No errors printed" is not silence.
#[test]
fn sound_states_are_silent() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};

    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "sound_session")
        .expect("sound_session scenario must exist");

    let run = ToyRun::new("sound-silent").expect("must create workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "sound scenario exec must succeed, stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    let check = run.check().expect("lim check must run");
    liminal_conformance::assert_silent(&check);
}

/// Law 3B / R4 §10: "No edit is rejected." Every write path in every scenario
/// — including the unavailable-Holder scenario — returns success; inadmissible
/// writes become durable Overlays, never errors.
#[test]
fn no_edit_is_rejected() {
    use liminal_conformance::harness::{NOT_YET_DRIVEN, ToyRun, all_scenarios};

    let scenarios = all_scenarios().expect("must load scenarios");
    let mut driven = 0;
    for scenario in &scenarios {
        if NOT_YET_DRIVEN.contains(&scenario.scenario.id.as_str()) {
            continue;
        }
        let run = ToyRun::new(&format!("no-reject-{}", scenario.scenario.id))
            .expect("must create workspace");
        let exec = run.exec_scenario(scenario).expect("exec must run");
        assert!(
            exec.status.success(),
            "scenario {} rejected a write path (exit {:?}); stderr: {}",
            scenario.scenario.id,
            exec.status.code(),
            String::from_utf8_lossy(&exec.stderr)
        );
        driven += 1;
    }
    assert!(driven > 0, "at least one scenario must be driven");
}

/// Law 3F / R4 §10: cross-Jurisdiction repair follows the mutation-local
/// composition rule — each proposed mutation is authorized by ITS OWN
/// subject's Jurisdiction; neither Contract commandeers the other subject.
#[test]
#[ignore = "Phase -1 M4: mutation-local conjunctive authorization"]
fn cross_jurisdiction_repair_is_mutation_local() {
    unimplemented!("damage the comment Relation's target; assert per-subject authorization")
}

/// R4 §4 / §10: Promotion and repair use ONE interpreter. Driving "save"
/// twice over identical inputs must yield observationally equal worlds — same
/// files, graph, decision log, and repair record — modulo minted ids and
/// clocks (v4 §112: observational equivalence via the normalized world digest,
/// M04 Algorithm F). Save IS Promotion IS a RepairPlan through the ONE
/// interpreter; there is no separate promotion mechanism to diverge.
#[test]
fn promotion_uses_the_same_repair_interpreter() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};

    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "disjoint_safe_repair")
        .expect("disjoint_safe_repair must exist");

    let run_a = ToyRun::new("promo-a").expect("workspace a");
    run_a.exec_scenario(scenario).expect("exec a");
    let digest_a =
        liminal_daemon::runner::normalized_digest(&run_a.root).expect("normalized digest a");

    let run_b = ToyRun::new("promo-b").expect("workspace b");
    run_b.exec_scenario(scenario).expect("exec b");
    let digest_b =
        liminal_daemon::runner::normalized_digest(&run_b.root).expect("normalized digest b");

    assert_eq!(
        digest_a, digest_b,
        "save-as-Promotion through the ONE interpreter must be observationally \
         reproducible (normalized worlds equal)"
    );

    // Sanity: the digest is not trivially empty (the save actually happened).
    assert_eq!(digest_a.len(), 64, "digest must be a 64-hex blake3");
}

/// R4 §6 / §10: a unique-but-unsafe merge candidate is NOT auto-accepted.
/// Determinism is necessary, never sufficient: with exactly one clean textual
/// merge INSIDE one paragraph, the repair must be offered for review, not
/// applied.
#[test]
fn unique_but_unsafe_candidate_is_not_auto_accepted() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};
    use liminal_daemon::ToyWorkspace;

    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "unsafe_unique_merge")
        .expect("unsafe_unique_merge must exist");

    let run = ToyRun::new("unsafe-unique").expect("must create workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "capture is never rejected (Law 3B); stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    // The decision must be NeedsReview (determinism ≠ safety) — no ILRP intent
    // was prepared, and the durable file is unchanged from its foreign-edited
    // state (nothing auto-applied). Scope the workspace so its exclusive store
    // lock is released before the `lim check` subprocess runs.
    {
        let ws = ToyWorkspace::open(&run.root).expect("must open workspace");
        let store = ws.store();
        let decisions = store
            .scan_aux(liminal_graph::ns::JUR_DECISION)
            .expect("scan decisions");
        assert_eq!(decisions.len(), 1, "exactly one repair decision");
        let decision: liminal_jurisdiction::RepairDecision =
            serde_json::from_value(decisions[0].1.clone()).expect("decode decision");
        assert!(
            matches!(
                decision,
                liminal_jurisdiction::RepairDecision::NeedsReview { .. }
            ),
            "unique-but-unsafe merge must NOT auto-apply, got {decision:?}"
        );
        let intents = store
            .scan_aux(liminal_graph::ns::ILRP_INTENT)
            .expect("scan intents");
        assert!(intents.is_empty(), "nothing auto-applied → no ILRP intent");
    }

    // `lim check` stays silent (the reconciliation item is the single incident;
    // the checker does not amplify it — R4 §2.3).
    let check = run.check().expect("lim check must run");
    liminal_conformance::assert_silent(&check);
}

/// Law 3B/3I / R4 §10: an offline edit is durable as an Overlay, survives
/// process kill, and appears in `lim overlays` (with age) WITHOUT any agenda
/// subsystem existing.
#[test]
#[ignore = "Phase -1 M5: overlay durability + reconciliation queue surfacing"]
fn offline_edit_is_durable_and_visible_in_lim_overlays() {
    unimplemented!("scenario: fixtures/scenarios/offline_holder_overlay.scenario.toml")
}

/// Law 3H / R4 §10: ILRP resumes correctly after termination at EVERY durable
/// boundary and reaches Committed, NeedsReview, or Aborted with no hidden
/// half-state, duplication, or lost work. The matrix is DERIVED from the
/// baseline hit trace (`ToyRun::crash_matrix`); recovery must be idempotent
/// (equal world digests on double recovery). Runs serialized (crash_ prefix →
/// nextest crash group).
///
/// Scope grows via `runnable_crash_scenarios()`: `dag_id_then_reattach` joins
/// at M4. The assertion never changes; only coverage grows.
#[test]
fn crash_ilrp_resumes_after_kill_at_every_boundary() {
    let scenarios = liminal_conformance::harness::runnable_crash_scenarios()
        .expect("must load runnable scenarios");
    assert!(
        !scenarios.is_empty(),
        "at least one crash scenario must be runnable"
    );
    for scenario in &scenarios {
        liminal_conformance::harness::ToyRun::crash_matrix(scenario)
            .unwrap_or_else(|e| panic!("crash matrix failed for {}: {e}", scenario.scenario.id));
    }
}

/// R4 §6 / §10: every automatically accepted repair is one-command revertible
/// (`lim repair undo <id>`), and undo after subsequent edits becomes a NEW
/// Basis-checked RepairPlan rather than a stale-byte overwrite.
#[test]
fn accepted_auto_repair_is_one_command_revertible() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};
    use liminal_daemon::ToyWorkspace;

    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "disjoint_safe_repair")
        .expect("disjoint_safe_repair must exist");

    let run = ToyRun::new("revertible").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec");
    assert!(exec.status.success(), "auto-apply must succeed");

    // A RepairRecord with all six R4 §6 fields was written; capture its id and
    // the post-save file bytes. Scope the store handle so the CLI can lock it.
    let (repair_id, post_save) = {
        let ws = ToyWorkspace::open(&run.root).expect("open");
        let records = ws.repairs().expect("repairs");
        assert_eq!(records.len(), 1, "exactly one accepted repair");
        let r = &records[0];
        // Six fields are structurally present (the type enforces it); assert
        // the load-bearing ones are populated.
        assert_eq!(r.selected_rule, "save-promotion");
        assert!(r.inverse.is_some(), "must record an inverse (revertible)");
        assert_eq!(r.applied_steps.len(), 1);
        (
            r.repair,
            std::fs::read_to_string(run.root.join("notes.md")).unwrap(),
        )
    };

    // `lim repair undo <id>` restores the pre-save state.
    let undo = run
        .lim(&["repair", "undo", &repair_id.to_string()])
        .expect("undo runs");
    assert!(undo.status.success(), "undo exit 0");
    let stdout = String::from_utf8_lossy(&undo.stdout);
    assert!(stdout.starts_with("undone: repair:"), "got {stdout:?}");
    let after_undo = std::fs::read_to_string(run.root.join("notes.md")).unwrap();
    assert_ne!(after_undo, post_save, "undo changed the file back");

    // Undo again after an external edit → the recorded inverse is stale, so
    // undo must queue a REVIEWABLE proposal, never overwrite with stale bytes.
    // Re-run the whole scenario in a fresh workspace, then externally edit the
    // file before undoing.
    let run2 = ToyRun::new("revertible-stale").expect("workspace");
    run2.exec_scenario(scenario).expect("exec2");
    let repair2 = {
        let ws = ToyWorkspace::open(&run2.root).expect("open2");
        ws.repairs().expect("repairs2")[0].repair
    };
    std::fs::write(run2.root.join("notes.md"), b"a wholly different file\n").unwrap();
    let undo2 = run2
        .lim(&["repair", "undo", &repair2.to_string()])
        .expect("undo2 runs");
    assert!(undo2.status.success(), "stale undo still exits 0");
    let stdout2 = String::from_utf8_lossy(&undo2.stdout);
    assert!(
        stdout2.starts_with("undo queued for review:"),
        "stale undo must be reviewable, got {stdout2:?}"
    );
    // The file was NOT overwritten with stale bytes.
    let after = std::fs::read_to_string(run2.root.join("notes.md")).unwrap();
    assert_eq!(
        after, "a wholly different file\n",
        "no stale-byte overwrite"
    );
}

/// Law 3J / R4 §10: `ClientScoped(neovim)`, `ClientScoped(phone)`, and
/// `DurableOnly` produce THREE intentional, individually-correct snapshots of
/// the same paragraph.
#[test]
#[ignore = "Phase -1 M6: perspective resolution"]
fn three_perspectives_yield_three_intentional_snapshots() {
    unimplemented!("two dirty clients over one file; capture three bases; compare payloads")
}

/// Law 3J / v4 §112: no captured Basis ever combines two clients' dirty
/// buffers for one durable subject. Property test over random interleavings
/// of edits/saves from both clients.
#[test]
#[ignore = "Phase -1 M6: anti-chimera property test (proptest over interleavings)"]
fn no_computation_sees_a_chimeric_basis() {
    unimplemented!(
        "proptest: for all interleavings, every Basis has <= 1 client's buffers per subject"
    )
}

/// v4 §7.5 / R4 §10: buffer generations invalidate ONLY dependent queries —
/// a phone edit invalidates phone-scoped memo entries; the neovim preview and
/// the DurableOnly export hit cache.
#[test]
#[ignore = "Phase -1 M6: component-granular invalidation over ComponentDeps"]
fn buffer_generations_invalidate_only_dependents() {
    unimplemented!("memo table keyed by (query, component set); assert selective invalidation")
}

/// v4 §7.7 example / R4 §5: the two-step repair DAG schedules source-ID
/// insertion strictly before Relation reattachment, verified from the
/// fault-trace ordering of the apply boundaries, and rejects any schedule
/// that runs reattachment first.
#[test]
#[ignore = "Phase -1 M4: repair DAG execution ordering"]
fn two_step_repair_dag_orders_id_insert_before_reattach() {
    unimplemented!("scenario: fixtures/scenarios/dag_id_then_reattach.scenario.toml")
}
