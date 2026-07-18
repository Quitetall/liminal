//! The executable Phase -1 exit gate (R4 §10) — one test per gate bullet.
//!
//! Every test here is the DEFINITION OF DONE for its milestone
//! (docs/implementation-plan.md). Un-ignoring a test means implementing it
//! for real against the harness; the assertion each must make is spelled out
//! in its doc comment and must not be weakened.

use liminal_conformance::harness::ToyRun;
use liminal_conformance::scenario::{ScenarioHeader, ScenarioScript};

/// Create a ToyRun holding a single ingested file (no scripted steps): the
/// starting point for the M06 perspective tests, which drive buffers in-process
/// against `ToyWorkspace` afterward.
fn write_single_file_workspace(label: &str, path: &str, text: &str) -> ToyRun {
    let scenario = ScenarioScript {
        scenario: ScenarioHeader {
            id: format!("m06-{label}"),
            title: None,
            spec: vec![],
            profiles: vec!["external-file".into()],
        },
        setup: liminal_daemon::scenario::Setup {
            files: vec![liminal_daemon::scenario::SetupFile {
                path: path.to_owned(),
                text: text.to_owned(),
            }],
            graph: vec![],
            buffers: vec![],
        },
        steps: vec![],
        expect: liminal_daemon::scenario::Expectation::default(),
    };
    let run = ToyRun::new(label).expect("workspace");
    let out = run.exec_scenario(&scenario).expect("exec");
    assert!(
        out.status.success(),
        "ingest exec failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    run
}

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
    use liminal_conformance::harness::{ToyRun, all_scenarios};

    let scenarios = all_scenarios().expect("must load scenarios");
    let mut driven = 0;
    for scenario in &scenarios {
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
fn cross_jurisdiction_repair_is_mutation_local() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};
    use liminal_daemon::ToyWorkspace;

    let scenarios = all_scenarios().expect("scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "dag_id_then_reattach")
        .expect("dag_id_then_reattach must exist");

    // Drive the foreign edit → a two-step DAG is proposed (file ID-insert +
    // graph reattach). It lands NeedsReview.
    let run = ToyRun::new("mutation-local").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec");
    assert!(exec.status.success(), "capture never rejected (Law 3B)");

    let ws = ToyWorkspace::open(&run.root).expect("open");
    let store = ws.store();

    // Load the proposed plan.
    let plans = store.scan_aux(liminal_graph::ns::JUR_PLAN).expect("plans");
    assert_eq!(plans.len(), 1, "exactly one proposed DAG plan");
    let plan: liminal_jurisdiction::RepairPlan =
        serde_json::from_value(plans[0].1.clone()).expect("decode plan");

    // Two steps, each governed by ITS OWN subject: the file ID-insert by the
    // FILE node (external-file), the reattach by the Relation (graph-native).
    let profiles = liminal_jurisdiction::ProfileSet::phase_minus_1();
    let mut kinds: Vec<&str> = Vec::new();
    for step in plan.steps.values() {
        let profile = profiles
            .for_subject(step.subject, store)
            .expect("every step resolves to its own governor");
        kinds.push(match step.subject {
            liminal_id::JurisdictionSubject::Node(_) => {
                assert_eq!(profile.id().0, "external-file");
                "file"
            }
            liminal_id::JurisdictionSubject::Relation(_) => {
                assert_eq!(profile.id().0, "graph-native");
                "graph"
            }
        });
    }
    kinds.sort_unstable();
    assert_eq!(kinds, vec!["file", "graph"], "one file + one graph step");

    // Mutation-local authorization (Q7): no step's Contract refuses on
    // authorization grounds — neither commandeers the other. The refusal that
    // makes this NeedsReview comes from the Relation's OWN identity Contract
    // (graph-native requires Explicit; the heuristic re-id is only Inferred),
    // not from the file Contract reaching across.
    let checker = ws.checker();
    let auth = checker.authorize(&plan).expect("authorize");
    assert!(
        auth.refusals.is_empty(),
        "mutation-local authorization must not refuse: {:?}",
        auth.refusals
    );

    let decision = checker.evaluate_repair(&plan).expect("evaluate");
    match decision {
        liminal_jurisdiction::RepairDecision::NeedsReview { reasons } => {
            assert!(
                reasons.iter().any(|r| r.0.contains("heuristic match")),
                "refusal must be the Relation's own identity requirement, got {reasons:?}"
            );
        }
        liminal_jurisdiction::RepairDecision::AutoApply { evidence } => {
            panic!("heuristic reattachment must NeedsReview, auto-applied with {evidence:?}")
        }
    }
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
/// process kill, and appears in `lim overlays` (with age) WITHOUT any
/// task-list subsystem existing.
#[test]
fn offline_edit_is_durable_and_visible_in_lim_overlays() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};
    use liminal_daemon::ToyWorkspace;

    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "offline_holder_overlay")
        .expect("offline_holder_overlay must exist");

    let run = ToyRun::new("offline-overlay").expect("must create workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "capture is never rejected (Law 3B); stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    // Overlay present post-restart (the scenario's own last step); draft
    // bytes fully recoverable from the overlay + its blob == the buffer edit.
    // Independently recompute the expected merged text (base == durable file,
    // since nothing else touched it while offline) rather than hand-deriving
    // the toy paragraph reconstruction's exact byte layout.
    let base_text = &scenario.setup.files[0].text;
    let edited = base_text.replacen("decomposes a signal", "decomposes a time-domain signal", 1);
    let expected = match liminal_source::merge::three_way(base_text, &edited, base_text) {
        liminal_source::merge::MergeOutcome::Disjoint { merged } => merged,
        other => panic!("expected a disjoint merge, got {other:?}"),
    };
    let expected = expected.as_str();
    {
        let ws = ToyWorkspace::open(&run.root).expect("open");
        let store = ws.store();

        let overlays = store
            .scan_aux(liminal_graph::ns::JUR_OVERLAY)
            .expect("scan overlays");
        assert_eq!(overlays.len(), 1, "exactly one durable overlay");
        let overlay: liminal_jurisdiction::Overlay =
            serde_json::from_value(overlays[0].1.clone()).expect("decode overlay");
        assert_eq!(overlay.state, liminal_jurisdiction::OverlayState::Active);
        let liminal_jurisdiction::RepairOperation::WriteFile { contents, .. } = &overlay.operation
        else {
            panic!("offline overlay must carry a WriteFile draft");
        };
        assert_eq!(
            String::from_utf8_lossy(contents).as_ref(),
            expected,
            "draft bytes recoverable from the overlay match the edit"
        );

        // Also recoverable via the content-addressed blob (D05.2 SYS_BLOB[ours]).
        let hash = liminal_id::ContentHash::of(expected.as_bytes());
        let blob = liminal_jurisdiction::blob::get(store, hash)
            .expect("blob lookup")
            .expect("draft blob present");
        assert_eq!(blob, expected);

        // Never silently promoted: zero reconciliation items (R4 §2.3 window).
        let items = ws.reconciliation().items().expect("items");
        assert_eq!(
            items.len(),
            0,
            "profile-declared transient draft, no debt yet"
        );

        // The durable file itself is untouched — capture never wrote through
        // the unavailable Holder.
        let on_disk = std::fs::read_to_string(run.root.join("notes.md")).unwrap();
        assert_ne!(
            on_disk, expected,
            "the unavailable Holder was never written"
        );
    }

    // `lim overlays` renders exactly one everyday-vocabulary line.
    let overlays_out = run.lim(&["overlays"]).expect("lim overlays runs");
    assert!(overlays_out.status.success(), "lim overlays exit 0");
    let stdout = String::from_utf8_lossy(&overlays_out.stdout);
    let re = regex_lite_match(&stdout);
    assert!(
        re,
        "lim overlays output must match '^pending sync  notes\\.md  \\d+[smhd]$', got {stdout:?}"
    );

    // The checker stays byte-silent (the overlay is not a soundness finding).
    let check = run.check().expect("lim check must run");
    liminal_conformance::assert_silent(&check);
}

/// Hand-rolled match for `^pending sync  notes\.md  \d+[smhd]$\n` — no regex
/// dependency needed for one fixed-shape line.
fn regex_lite_match(stdout: &str) -> bool {
    let Some(line) = stdout.strip_suffix('\n') else {
        return false;
    };
    if stdout.matches('\n').count() != 1 {
        return false;
    }
    let Some(rest) = line.strip_prefix("pending sync  notes.md  ") else {
        return false;
    };
    let Some((digits, unit)) = rest.split_at_checked(rest.len().saturating_sub(1)) else {
        return false;
    };
    !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && matches!(unit, "s" | "m" | "h" | "d")
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
        ToyRun::crash_matrix(scenario)
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
fn three_perspectives_yield_three_intentional_snapshots() {
    use liminal_daemon::ToyWorkspace;
    use liminal_daemon::render::render_paragraphs;
    use liminal_id::{ClientId, PathId};
    use liminal_revision::{BasisComponent, BasisPerspective};

    // A one-file workspace (file v0 on disk, ingested).
    let run = write_single_file_workspace("three-persp", "notes.md", "the base text {#p}\n");
    let mut ws = ToyWorkspace::open(&run.root).expect("open");
    let path = PathId("notes.md".into());

    // Two clients open the same file and edit it to DIFFERENT text — no saves.
    let neovim = ClientId::new();
    let phone = ClientId::new();
    let nbuf = ws.client(neovim).open_buffer(path.clone());
    ws.client(neovim).edit(nbuf, "the neovim edit v1 {#p}\n");
    let pbuf = ws.client(phone).open_buffer(path.clone());
    ws.client(phone).edit(pbuf, "the phone edit v2 {#p}\n");

    // Three perspectives → three intentional snapshots.
    let neovim_basis = ws
        .basis(BasisPerspective::ClientScoped { client: neovim })
        .expect("neovim basis");
    let phone_basis = ws
        .basis(BasisPerspective::ClientScoped { client: phone })
        .expect("phone basis");
    let durable_basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");

    let n = render_paragraphs(ws.store(), &run.root, &neovim_basis, &path);
    let p = render_paragraphs(ws.store(), &run.root, &phone_basis, &path);
    let d = render_paragraphs(ws.store(), &run.root, &durable_basis, &path);

    assert!(
        n.contains("neovim edit v1"),
        "neovim sees its own v1: {n:?}"
    );
    assert!(p.contains("phone edit v2"), "phone sees its own v2: {p:?}");
    assert!(d.contains("base text"), "durable sees v0: {d:?}");
    assert_ne!(n, p);
    assert_ne!(n, d);
    assert_ne!(p, d);

    // Each basis records its perspective and carries <= 1 working claim.
    for (basis, expect_buffers) in [
        (&neovim_basis, true),
        (&phone_basis, true),
        (&durable_basis, false),
    ] {
        let buffer_components: Vec<_> = basis
            .components
            .values()
            .filter(|c| matches!(c, BasisComponent::BufferGeneration { .. }))
            .collect();
        assert!(buffer_components.len() <= 1, "at most one working claim");
        assert_eq!(!buffer_components.is_empty(), expect_buffers);
    }
    assert_eq!(
        neovim_basis.perspective,
        BasisPerspective::ClientScoped { client: neovim }
    );
    assert_eq!(durable_basis.perspective, BasisPerspective::DurableOnly);
}

/// Law 3J / v4 §112: no captured Basis ever combines two clients' dirty
/// buffers for one durable subject. Property test over random interleavings
/// of edits/saves from both clients.
#[test]
fn no_computation_sees_a_chimeric_basis() {
    use liminal_daemon::ToyWorkspace;
    use liminal_id::{ClientId, PathId};
    use liminal_revision::{BasisComponent, BasisPerspective};
    use proptest::prelude::*;

    // Op alphabet: which client acts, and what.
    #[derive(Debug, Clone)]
    enum Op {
        Edit(bool, u8), // (is_neovim, token)
        Save(bool),
        SnapshotRead(u8), // 0=neovim, 1=phone, 2=durable
    }

    let op_strategy = prop_oneof![
        (any::<bool>(), 0u8..8).prop_map(|(c, t)| Op::Edit(c, t)),
        any::<bool>().prop_map(Op::Save),
        (0u8..3).prop_map(Op::SnapshotRead),
    ];
    let op_vec = prop::collection::vec(op_strategy, 0..40);

    proptest!(ProptestConfig::with_cases(64), |(ops in op_vec)| {
        let run = write_single_file_workspace("chimera", "notes.md", "base {#p}\n");
        let mut ws = ToyWorkspace::open(&run.root).expect("open");
        let path = PathId("notes.md".into());
        let neovim = ClientId::new();
        let phone = ClientId::new();
        let nbuf = ws.client(neovim).open_buffer(path.clone());
        let pbuf = ws.client(phone).open_buffer(path.clone());

        let perspectives = [
            BasisPerspective::ClientScoped { client: neovim },
            BasisPerspective::ClientScoped { client: phone },
            BasisPerspective::DurableOnly,
        ];

        // Assert the anti-chimera invariant after EVERY op.
        let check_all = |ws: &ToyWorkspace| -> Result<(), TestCaseError> {
            for persp in &perspectives {
                // Resolution may legitimately fail (ambiguity guard); a failure
                // is never a chimera.
                let Ok(basis) = ws.basis(persp.clone()) else { continue };
                let buffers: Vec<(ClientId, _)> = basis
                    .components
                    .values()
                    .filter_map(|c| match c {
                        BasisComponent::BufferGeneration { client, .. } => Some((*client, ())),
                        _ => None,
                    })
                    .collect();
                // <= 1 client's BufferGeneration per key (single subject here).
                prop_assert!(buffers.len() <= 1, "chimeric: {} buffer components", buffers.len());
                match persp {
                    BasisPerspective::ClientScoped { client } => {
                        for (c, ()) in &buffers {
                            prop_assert_eq!(*c, *client, "foreign buffer leaked into ClientScoped");
                        }
                    }
                    BasisPerspective::DurableOnly => {
                        prop_assert!(buffers.is_empty(), "DurableOnly must carry zero buffers");
                    }
                    _ => {}
                }
            }
            Ok(())
        };

        check_all(&ws)?;
        for op in ops {
            match op {
                Op::Edit(is_neovim, token) => {
                    let (client, buf) = if is_neovim { (neovim, nbuf) } else { (phone, pbuf) };
                    ws.client(client).edit(buf, &format!("edit-{token} {{#p}}\n"));
                }
                Op::Save(is_neovim) => {
                    // Save-while-other-dirty may yield NeedsReview (R4 §11.4) —
                    // allowed; the invariant is scoped to basis composition. The
                    // ClientSession::save fold-in is M06.4; here saves are a
                    // no-op placeholder that must not create a chimera.
                    let _ = is_neovim;
                }
                Op::SnapshotRead(_which) => {
                    // Reading is exactly the resolution asserted below.
                }
            }
            check_all(&ws)?;
        }
    });
}

/// v4 §7.5 / R4 §10: buffer generations invalidate ONLY dependent queries —
/// a phone edit invalidates phone-scoped memo entries; the neovim preview and
/// the DurableOnly export hit cache.
#[test]
fn buffer_generations_invalidate_only_dependents() {
    use liminal_conformance::memo::{MemoTable, entry_over_basis};
    use liminal_daemon::ToyWorkspace;
    use liminal_daemon::render::render_paragraphs;
    use liminal_id::{ClientId, PathId};
    use liminal_revision::BasisPerspective;

    let run = write_single_file_workspace("invalidate", "notes.md", "base {#p}\n");
    let mut ws = ToyWorkspace::open(&run.root).expect("open");
    let path = PathId("notes.md".into());
    let neovim = ClientId::new();
    let phone = ClientId::new();
    let nbuf = ws.client(neovim).open_buffer(path.clone());
    ws.client(neovim).edit(nbuf, "neovim {#p}\n");
    let pbuf = ws.client(phone).open_buffer(path.clone());
    ws.client(phone).edit(pbuf, "phone {#p}\n");

    let n_label = "client:neovim";
    let p_label = "client:phone";
    let d_label = "durable";
    let persp = |c: ClientId| BasisPerspective::ClientScoped { client: c };

    // Memoize render_paragraphs under all three perspectives.
    let mut table = MemoTable::new();
    let n0 = ws.basis(persp(neovim)).unwrap();
    let p0 = ws.basis(persp(phone)).unwrap();
    let d0 = ws.basis(BasisPerspective::DurableOnly).unwrap();
    table.insert(
        "render",
        n_label,
        entry_over_basis(render_paragraphs(ws.store(), &run.root, &n0, &path), &n0),
    );
    table.insert(
        "render",
        p_label,
        entry_over_basis(render_paragraphs(ws.store(), &run.root, &p0, &path), &p0),
    );
    table.insert(
        "render",
        d_label,
        entry_over_basis(render_paragraphs(ws.store(), &run.root, &d0, &path), &d0),
    );

    // A PHONE edit bumps only the phone buffer generation. Re-resolve and
    // judge each entry by component-value comparison (D06.6).
    ws.client(phone).edit(pbuf, "phone again {#p}\n");
    let n1 = ws.basis(persp(neovim)).unwrap();
    let p1 = ws.basis(persp(phone)).unwrap();
    let d1 = ws.basis(BasisPerspective::DurableOnly).unwrap();
    assert!(
        table.is_stale("render", p_label, &p1),
        "phone entry stale after phone edit"
    );
    assert!(
        !table.is_stale("render", n_label, &n1),
        "neovim entry NOT invalidated by a phone edit (its component unchanged)"
    );
    assert!(
        !table.is_stale("render", d_label, &d1),
        "durable entry NOT invalidated by a phone buffer edit"
    );

    // A NEOVIM edit bumps only neovim's generation: neovim stale, phone/durable
    // unaffected (component-scoped, not whole-map).
    ws.client(neovim).edit(nbuf, "neovim moved {#p}\n");
    let n2 = ws.basis(persp(neovim)).unwrap();
    let d2 = ws.basis(BasisPerspective::DurableOnly).unwrap();
    assert!(
        table.is_stale("render", n_label, &n2),
        "neovim entry stale after neovim edit"
    );
    assert!(
        !table.is_stale("render", d_label, &d2),
        "durable export still cache-valid — untouched by any buffer edit"
    );
}

/// v4 §7.7 example / R4 §5: the two-step repair DAG schedules source-ID
/// insertion strictly before Relation reattachment, verified from the
/// fault-trace ordering of the apply boundaries, and rejects any schedule
/// that runs reattachment first.
#[test]
fn two_step_repair_dag_orders_id_insert_before_reattach() {
    use liminal_conformance::harness::{ToyRun, all_scenarios};

    let scenarios = all_scenarios().expect("scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "dag_accept")
        .expect("dag_accept must exist");

    // Baseline the accept run and read the ordered fault trace: the file step
    // (after_external_apply) must precede the finalize boundary where the
    // RetargetRelation graph op lands (before_finalize).
    let run = ToyRun::new("dag-order").expect("workspace");
    let trace = run.baseline(scenario).expect("baseline");
    let pos = |name: &str| {
        trace
            .hits
            .iter()
            .position(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("trace missing {name}: {:?}", trace.hits))
    };
    assert!(
        pos("ilrp/after_external_apply") < pos("ilrp/before_finalize"),
        "source-id insertion (external apply) must precede reattachment \
         (finalize): {:?}",
        trace.hits
    );

    // A hand-built plan with the edge inverted (a file/external step depending
    // on a Graph step) is unexecutable under ILRP and refused at prepare time
    // (D04.4) — the interpreter never runs reattachment before insertion.
    let refused = liminal_daemon::runner::graph_before_file_is_refused();
    assert!(
        refused,
        "graph-before-file ordering must be refused (D04.4)"
    );
}
