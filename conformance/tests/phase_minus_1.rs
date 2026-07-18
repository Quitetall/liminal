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

/// R4 §4 / §10: Promotion and repair use ONE interpreter. Driving "save" and
/// driving the equivalent RepairPlan by hand must yield identical store
/// state, decision-log entries, and Basis advancement (v4 §112:
/// observational equivalence).
#[test]
#[ignore = "Phase -1 M4: save-as-Promotion through the single RepairPlan interpreter"]
fn promotion_uses_the_same_repair_interpreter() {
    unimplemented!("compare world digests after save vs. hand-built RepairPlan")
}

/// R4 §6 / §10: a unique-but-unsafe merge candidate is NOT auto-accepted.
/// Determinism is necessary, never sufficient: with exactly one clean textual
/// merge INSIDE one paragraph, the repair must be offered for review, not
/// applied.
#[test]
#[ignore = "Phase -1 M4: domain safety predicate (structural disjointness)"]
fn unique_but_unsafe_candidate_is_not_auto_accepted() {
    unimplemented!("scenario: fixtures/scenarios/unsafe_unique_merge.scenario.toml")
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
#[ignore = "Phase -1 M4: decision log + Basis-checked undo"]
fn accepted_auto_repair_is_one_command_revertible() {
    unimplemented!("scenario: fixtures/scenarios/disjoint_safe_repair.scenario.toml")
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
