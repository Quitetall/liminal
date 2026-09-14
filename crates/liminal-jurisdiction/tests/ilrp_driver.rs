//! In-process ILRP driver tests (M02.3; AM-17.12 checked admission).
//!
//! Tests use real durable stores. Positive histories begin with a Checker-issued
//! capability. Malformed histories use explicit trusted-root writes and must be
//! rejected without effects.

use std::collections::BTreeMap;
use std::sync::Mutex;

use liminal_graph::ns::{ILRP_INTENT, SYS_BLOB};
use liminal_graph::{Node, NodeFlags, Origin, PayloadRef, StoreOwner, TxnMeta, kind};
use liminal_id::{
    ActorId, ContentHash, IdempotencyKey, JurisdictionSubject, NodeId, PathId, RepairId,
    RepairStepId, RevisionId, Timestamp, TransactionId,
};
use liminal_jurisdiction::{
    AuthorizedRepair, Checker, CrashInjector, CrashPoint, ExternalExecutor, IlrpDriver, IlrpError,
    IntentState, NoCrash, PrestateMatch, ProfileSet, ProposedMutation, RepairIntent,
    RepairOperation, RepairPlan, SafetyEvidence, StatePredicate, StepAck,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

fn fresh_dir(name: &str) -> liminal_scratch::ScratchDir {
    liminal_scratch::ScratchDir::new(&format!("ilrp-test-{name}")).expect("scratch dir")
}

fn empty_basis() -> WorkspaceBasis {
    WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    }
}

fn setup_file(owner: &StoreOwner, path: &str) -> NodeId {
    let node = NodeId::new();
    let writer = owner.bootstrap_writer();
    let mut txn = writer.begin().unwrap();
    txn.apply(liminal_graph::Operation::CreateNode {
        node: Node {
            id: node,
            kind: kind::FILE,
            payload: PayloadRef::Text(String::new()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.put_aux(
        SYS_BLOB,
        &format!("file/{path}"),
        serde_json::Value::String(String::new()),
    )
    .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp(1),
        provenance: Some("test:trusted-file-setup".into()),
        inverse: None,
    })
    .unwrap();
    node
}

fn write_file_step(subject: NodeId, path: &str, contents: &[u8]) -> ProposedMutation {
    ProposedMutation {
        id: RepairStepId::new(),
        subject: JurisdictionSubject::Node(subject),
        operation: RepairOperation::WriteFile {
            path: PathId(path.into()),
            contents: contents.to_vec(),
        },
        expected_prestate: StatePredicate::FileAbsent {
            path: PathId(path.into()),
        },
        expected_poststate: StatePredicate::FileContent {
            path: PathId(path.into()),
            hash: ContentHash::of(contents),
        },
        idempotency_key: IdempotencyKey::new(),
    }
}

fn single_step_plan(subject: NodeId, path: &str, contents: &[u8]) -> RepairPlan {
    let step = write_file_step(subject, path, contents);
    RepairPlan {
        id: RepairId::new(),
        basis: empty_basis(),
        steps: BTreeMap::from([(step.id, step)]),
        dependencies: vec![],
        inverse: None,
    }
}

fn accept<'s>(
    owner: &'s StoreOwner,
    profiles: &'s ProfileSet,
    plan: RepairPlan,
) -> AuthorizedRepair<'s> {
    Checker {
        store: owner.store(),
        profiles,
    }
    .accept_repair(plan, ActorId::new())
    .unwrap()
}

#[derive(Debug)]
struct MockExecutor {
    verify_results: Mutex<Vec<PrestateMatch>>,
    verify_calls: Mutex<Vec<StatePredicate>>,
    apply_results: Mutex<Vec<Result<StepAck, String>>>,
    apply_calls: Mutex<Vec<StatePredicate>>,
}

impl MockExecutor {
    fn new() -> Self {
        Self {
            verify_results: Mutex::new(Vec::new()),
            verify_calls: Mutex::new(Vec::new()),
            apply_results: Mutex::new(Vec::new()),
            apply_calls: Mutex::new(Vec::new()),
        }
    }

    fn push_verify(&self, result: PrestateMatch) {
        self.verify_results.lock().unwrap().push(result);
    }

    fn push_apply(&self, result: Result<StepAck, String>) {
        self.apply_results.lock().unwrap().push(result);
    }
}

impl ExternalExecutor for &MockExecutor {
    fn verify(&self, mutation: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        self.verify_calls
            .lock()
            .unwrap()
            .push(mutation.expected_prestate.clone());
        Ok(self.verify_results.lock().unwrap().remove(0))
    }

    fn apply(&self, mutation: &ProposedMutation) -> Result<StepAck, IlrpError> {
        self.apply_calls
            .lock()
            .unwrap()
            .push(mutation.expected_poststate.clone());
        self.apply_results
            .lock()
            .unwrap()
            .remove(0)
            .map_err(IlrpError::Executor)
    }
}

fn append_intent(owner: &StoreOwner, intent: &RepairIntent, provenance: &str) {
    let key = intent.plan.id.to_string();
    let mut txn = owner.begin().unwrap();
    txn.put_aux(ILRP_INTENT, &key, serde_json::to_value(intent).unwrap())
        .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: Some(provenance.into()),
        inverse: None,
    })
    .unwrap();
}

fn current_intent(owner: &StoreOwner, id: RepairId) -> RepairIntent {
    serde_json::from_value(
        owner
            .get_aux(ILRP_INTENT, &id.to_string())
            .unwrap()
            .unwrap(),
    )
    .unwrap()
}

#[derive(Debug, Clone, Copy)]
struct PanicAt(CrashPoint);

impl CrashInjector for PanicAt {
    fn crash_if_armed(&self, at: CrashPoint) {
        assert!(at != self.0, "controlled fixture stop at {at:?}");
    }
}

/// Synthesize only the durable state with no runtime crash hook. Production
/// commits `ExternalApplied` and immediately commits `Finalizing`; a real
/// driver run reaches Applying-with-ack first, then trusted test assembly adds
/// exactly the missing legal transition. This is not a crash witness and does
/// not claim root authority cannot forge.
fn synthesize_external_applied(owner: &StoreOwner, id: RepairId) {
    let mut intent = current_intent(owner, id);
    assert_eq!(intent.state, IntentState::Applying);
    assert_eq!(intent.acks.len(), intent.plan.steps.len());
    intent.state = IntentState::ExternalApplied;
    append_intent(owner, &intent, "state:external-applied");
}

/// Root fixture corruption. Unlike `synthesize_legal_progress`, this creates no
/// checked prepare record and must always be rejected by run and recovery.
fn hand_craft_intent(owner: &StoreOwner, intent: &RepairIntent) {
    append_intent(owner, intent, "test:hand-craft");
}

#[test]
fn ilrp_rejects_mismatched_ack_before_persistence() {
    for wrong_identity in [true, false] {
        let dir = fresh_dir("mismatched-ack");
        let owner = StoreOwner::open(&dir).unwrap();
        let subject = setup_file(&owner, "note.md");
        let plan = single_step_plan(subject, "note.md", b"hello");
        let step = plan.steps.values().next().unwrap();
        let executor = MockExecutor::new();
        executor.push_verify(PrestateMatch::Prestate);
        executor.push_apply(Ok(StepAck {
            step: if wrong_identity {
                RepairStepId::new()
            } else {
                step.id
            },
            observed_poststate: if wrong_identity {
                step.expected_poststate.clone()
            } else {
                StatePredicate::Any
            },
            at: Timestamp::now(),
        }));
        let profiles = ProfileSet::phase_minus_1();
        let authorized = accept(&owner, &profiles, plan);
        let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);
        let id = driver.prepare(authorized).unwrap();

        assert!(
            driver.run(id).is_err(),
            "invalid acknowledgement must refuse"
        );
        let intent = current_intent(&owner, id);
        assert_eq!(intent.state, IntentState::Applying);
        assert!(
            intent.acks.is_empty(),
            "invalid acknowledgement must not persist"
        );
    }
}

#[test]
fn ilrp_missing_acks_cannot_authorize_finalization_or_terminal_success() {
    for state in [
        IntentState::ExternalApplied,
        IntentState::Finalizing,
        IntentState::Committed,
    ] {
        let dir = fresh_dir("missing-acks");
        let owner = StoreOwner::open(&dir).unwrap();
        let subject = setup_file(&owner, "note.md");
        let intent = RepairIntent {
            plan: single_step_plan(subject, "note.md", b"hello"),
            state,
            evidence: SafetyEvidence::StructurallyDisjoint {
                description: "fault input".into(),
            },
            acks: BTreeMap::new(),
        };
        let id = intent.plan.id;
        hand_craft_intent(&owner, &intent);
        let head = owner.head().unwrap();
        let executor = MockExecutor::new();
        let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

        assert!(
            driver.run(id).is_err(),
            "state label cannot replace acknowledgements"
        );
        assert!(
            driver.recover_all().is_err(),
            "recovery must validate terminal rows too"
        );
        assert_eq!(
            owner.head().unwrap(),
            head,
            "invalid record must remain preserved"
        );
        assert!(executor.verify_calls.lock().unwrap().is_empty());
        assert!(executor.apply_calls.lock().unwrap().is_empty());
    }
}

#[test]
fn ilrp_prepare_persists_prepared_intent() {
    let dir = fresh_dir("prepare");
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = setup_file(&owner, "note.md");
    let executor = MockExecutor::new();
    let profiles = ProfileSet::phase_minus_1();
    let plan = single_step_plan(subject, "note.md", b"hello");
    let id = plan.id;
    let authorized = accept(&owner, &profiles, plan);
    let evidence = authorized.evidence().clone();
    let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

    let result = driver.prepare(authorized);
    assert!(result.is_ok(), "prepare must succeed: {:?}", result.err());
    assert_eq!(result.unwrap(), id);

    let intent = current_intent(&owner, id);
    assert_eq!(intent.state, IntentState::Prepared);
    assert_eq!(intent.evidence, evidence);
    assert!(intent.acks.is_empty());
}

#[test]
fn ilrp_run_happy_path_reaches_committed() {
    let dir = fresh_dir("happy");
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = setup_file(&owner, "note.md");
    let executor = MockExecutor::new();
    let plan = single_step_plan(subject, "note.md", b"hello");
    let step = plan.steps.values().next().unwrap();
    executor.push_verify(PrestateMatch::Prestate);
    executor.push_apply(Ok(StepAck {
        step: step.id,
        observed_poststate: step.expected_poststate.clone(),
        at: Timestamp::now(),
    }));
    let profiles = ProfileSet::phase_minus_1();
    let authorized = accept(&owner, &profiles, plan);
    let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

    let head_before = owner.head().unwrap();
    let id = driver.prepare(authorized).unwrap();
    let state = driver.run(id).unwrap();

    assert_eq!(state, IntentState::Committed);
    let head_after = owner.head().unwrap();
    assert_eq!(
        head_after.0 - head_before.0,
        6,
        "head must advance by exactly 6 commits post-prepare"
    );
    assert_eq!(current_intent(&owner, id).state, IntentState::Committed);
}

#[test]
fn ilrp_contested_step_lands_needs_review_and_preserves_target() {
    let dir = fresh_dir("contested");
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = setup_file(&owner, "note.md");
    let executor = MockExecutor::new();
    executor.push_verify(PrestateMatch::Neither);
    let profiles = ProfileSet::phase_minus_1();
    let plan = single_step_plan(subject, "note.md", b"hello");
    let authorized = accept(&owner, &profiles, plan);
    let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

    let id = driver.prepare(authorized).unwrap();
    let outcome = driver.run_checked(id).unwrap();

    assert_eq!(outcome.state(), IntentState::NeedsReview);
    assert!(
        outcome.committed().is_none(),
        "contested result must not carry a committed receipt"
    );
    assert_eq!(
        executor.apply_calls.lock().unwrap().len(),
        0,
        "contested step must NOT call apply"
    );
}

#[test]
fn ilrp_recover_all_terminalizes_each_nonterminal_state() {
    let dir = fresh_dir("recover");
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = setup_file(&owner, "note.md");
    let profiles = ProfileSet::phase_minus_1();
    let states_and_expected = [
        (IntentState::Prepared, IntentState::Committed),
        (IntentState::Applying, IntentState::Committed),
        (IntentState::ExternalApplied, IntentState::Committed),
        (IntentState::Finalizing, IntentState::Committed),
    ];

    let mut expected_acks = BTreeMap::new();
    for (initial, _) in &states_and_expected {
        let plan = single_step_plan(subject, "note.md", b"data");
        let step = plan.steps.values().next().unwrap();
        let ack = StepAck {
            step: step.id,
            observed_poststate: step.expected_poststate.clone(),
            at: Timestamp::now(),
        };
        let id = plan.id;
        let authorized = accept(&owner, &profiles, plan);
        let prepare_executor = MockExecutor::new();
        IlrpDriver::new(owner.coordinator_writer(), &prepare_executor, NoCrash)
            .prepare(authorized)
            .unwrap();

        match initial {
            IntentState::Prepared => {
                expected_acks.insert(id, ack);
            }
            IntentState::Applying => {
                let progress = MockExecutor::new();
                progress.push_verify(PrestateMatch::Prestate);
                let driver = IlrpDriver::new(
                    owner.coordinator_writer(),
                    &progress,
                    PanicAt(CrashPoint::BeforeExternalApply),
                );
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = driver.run(id);
                    }))
                    .is_err()
                );
                expected_acks.insert(id, ack);
            }
            IntentState::ExternalApplied => {
                let progress = MockExecutor::new();
                progress.push_verify(PrestateMatch::Prestate);
                progress.push_apply(Ok(ack));
                let driver = IlrpDriver::new(
                    owner.coordinator_writer(),
                    &progress,
                    PanicAt(CrashPoint::AfterAcknowledge),
                );
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = driver.run(id);
                    }))
                    .is_err()
                );
                synthesize_external_applied(&owner, id);
            }
            IntentState::Finalizing => {
                let progress = MockExecutor::new();
                progress.push_verify(PrestateMatch::Prestate);
                progress.push_apply(Ok(ack));
                let driver = IlrpDriver::new(
                    owner.coordinator_writer(),
                    &progress,
                    PanicAt(CrashPoint::BeforeFinalize),
                );
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = driver.run(id);
                    }))
                    .is_err()
                );
            }
            _ => unreachable!("nonterminal fixture table"),
        }
        assert_eq!(current_intent(&owner, id).state, *initial);
    }

    let executor = MockExecutor::new();
    for ack in expected_acks.into_values() {
        executor.push_verify(PrestateMatch::Prestate);
        executor.push_apply(Ok(ack));
    }
    let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

    let results = driver.recover_all().unwrap();
    assert_eq!(results.len(), states_and_expected.len());
    for (_, state) in &results {
        assert!(
            state.is_terminal(),
            "recovery must reach terminal state, got {state:?}"
        );
    }
}

#[test]
fn ilrp_recover_twice_is_noop() {
    let dir = fresh_dir("recover-twice");
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = setup_file(&owner, "note.md");
    let executor = MockExecutor::new();
    let plan = single_step_plan(subject, "note.md", b"hello");
    let step = plan.steps.values().next().unwrap();
    executor.push_verify(PrestateMatch::Prestate);
    executor.push_apply(Ok(StepAck {
        step: step.id,
        observed_poststate: step.expected_poststate.clone(),
        at: Timestamp::now(),
    }));
    let profiles = ProfileSet::phase_minus_1();
    let authorized = accept(&owner, &profiles, plan);
    let driver = IlrpDriver::new(owner.coordinator_writer(), &executor, NoCrash);

    let id = driver.prepare(authorized).unwrap();
    driver.run(id).unwrap();
    let head_after_run = owner.head().unwrap();

    let r1 = driver.recover_all().unwrap();
    assert!(r1.is_empty(), "all intents already terminal");
    let r2 = driver.recover_all().unwrap();
    assert!(r2.is_empty());
    assert_eq!(
        owner.head().unwrap(),
        head_after_run,
        "head must not change"
    );
}
