//! In-process ILRP driver tests (M02.3).
//!
//! These tests exercise `IlrpDriver` against a real `GraphStore` in a temp dir,
//! using a `MockExecutor` that records verify/apply calls. Crashed intermediate
//! states are hand-crafted by committing aux records at target states.

use std::collections::BTreeMap;
use std::sync::Mutex;

use camino::Utf8PathBuf;
use liminal_graph::ns::ILRP_INTENT;
use liminal_graph::{GraphStore, Origin, TxnMeta};
use liminal_id::{
    ContentHash, IdempotencyKey, JurisdictionSubject, NodeId, PathId, RepairId, RepairStepId,
    Timestamp, TransactionId,
};
use liminal_jurisdiction::{
    ExternalExecutor, IlrpDriver, IlrpError, IntentState, NoCrash, PrestateMatch, RepairIntent,
    StepAck,
};
use liminal_jurisdiction::{
    ProposedMutation, RepairOperation, RepairPlan, SafetyEvidence, StatePredicate,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

fn fresh_dir(name: &str) -> Utf8PathBuf {
    let dir = Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
        "liminal-ilrp-test-{name}-{}-{:x}",
        std::process::id(),
        Timestamp::now().0
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn empty_basis() -> WorkspaceBasis {
    WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    }
}

fn write_file_step(path: &str, contents: &[u8]) -> ProposedMutation {
    ProposedMutation {
        id: RepairStepId::new(),
        subject: JurisdictionSubject::Node(NodeId::new()),
        operation: RepairOperation::WriteFile {
            path: PathId(path.into()),
            contents: contents.to_vec(),
        },
        expected_prestate: StatePredicate::Any,
        expected_poststate: StatePredicate::FileContent {
            path: PathId(path.into()),
            hash: ContentHash::of(contents),
        },
        idempotency_key: IdempotencyKey::new(),
    }
}

fn single_step_plan(path: &str, contents: &[u8]) -> RepairPlan {
    let step = write_file_step(path, contents);
    RepairPlan {
        id: RepairId::new(),
        basis: empty_basis(),
        steps: BTreeMap::from([(step.id, step)]),
        dependencies: vec![],
        inverse: None,
    }
}

/// A mock executor that records verify/apply calls and returns configurable results.
#[derive(Debug)]
struct MockExecutor {
    /// What verify returns for each call.
    verify_results: Mutex<Vec<PrestateMatch>>,
    /// Recorded verify calls.
    verify_calls: Mutex<Vec<StatePredicate>>,
    /// What apply returns.
    apply_results: Mutex<Vec<Result<StepAck, String>>>,
    /// Recorded apply calls.
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
        let result = self.verify_results.lock().unwrap().remove(0);
        Ok(result)
    }

    fn apply(&self, mutation: &ProposedMutation) -> Result<StepAck, IlrpError> {
        self.apply_calls
            .lock()
            .unwrap()
            .push(mutation.expected_poststate.clone());
        let result = self.apply_results.lock().unwrap().remove(0);
        result.map_err(IlrpError::Executor)
    }
}

/// Helper: build an intent at a specific state and commit it to the store.
fn hand_craft_intent(store: &GraphStore, intent: &RepairIntent) {
    let key = intent.plan.id.to_string();
    let mut txn = store.begin().unwrap();
    txn.put_aux(ILRP_INTENT, &key, serde_json::to_value(intent).unwrap())
        .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: Some("test:hand-craft".into()),
        inverse: None,
    })
    .unwrap();
}

#[test]
fn ilrp_prepare_persists_prepared_intent() {
    let dir = fresh_dir("prepare");
    let store = GraphStore::open(&dir).unwrap();
    let executor = MockExecutor::new();
    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: NoCrash,
    };

    let plan = single_step_plan("note.md", b"hello");
    let id = plan.id;
    let evidence = SafetyEvidence::StructurallyDisjoint {
        description: "test".into(),
    };

    let result = driver.prepare(plan, evidence.clone());
    assert!(result.is_ok(), "prepare must succeed: {:?}", result.err());
    assert_eq!(result.unwrap(), id);

    // Verify the intent is persisted as Prepared.
    let value = store
        .get_aux(ILRP_INTENT, &id.to_string())
        .unwrap()
        .unwrap();
    let intent: RepairIntent = serde_json::from_value(value).unwrap();
    assert_eq!(intent.state, IntentState::Prepared);
    assert_eq!(intent.evidence, evidence);
    assert!(intent.acks.is_empty());
}

#[test]
fn ilrp_run_happy_path_reaches_committed() {
    let dir = fresh_dir("happy");
    let store = GraphStore::open(&dir).unwrap();
    let executor = MockExecutor::new();

    // Verify returns Prestate → apply is called.
    executor.push_verify(PrestateMatch::Prestate);
    executor.push_apply(Ok(StepAck {
        step: RepairStepId::new(), // will be overwritten
        observed_poststate: StatePredicate::Any,
        at: Timestamp::now(),
    }));

    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: NoCrash,
    };

    let plan = single_step_plan("note.md", b"hello");
    let evidence = SafetyEvidence::StructurallyDisjoint {
        description: "test".into(),
    };

    let head_before = store.head().unwrap();
    let id = driver.prepare(plan, evidence).unwrap();
    let state = driver.run(id).unwrap();

    assert_eq!(state, IntentState::Committed);

    // Head advanced: prepare (1) + applying (1) + ack (1) + external-applied (1)
    // + finalizing (1) + finalize (1) = 6 commits.
    let head_after = store.head().unwrap();
    assert_eq!(
        head_after.0 - head_before.0,
        6,
        "head must advance by exactly 6 commits post-prepare"
    );

    // Terminal intent persisted.
    let value = store
        .get_aux(ILRP_INTENT, &id.to_string())
        .unwrap()
        .unwrap();
    let intent: RepairIntent = serde_json::from_value(value).unwrap();
    assert_eq!(intent.state, IntentState::Committed);
}

#[test]
fn ilrp_contested_step_lands_needs_review_and_preserves_target() {
    let dir = fresh_dir("contested");
    let store = GraphStore::open(&dir).unwrap();
    let executor = MockExecutor::new();

    // Verify returns Neither → contested path, zero apply calls.
    executor.push_verify(PrestateMatch::Neither);

    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: NoCrash,
    };

    let plan = single_step_plan("note.md", b"hello");
    let evidence = SafetyEvidence::StructurallyDisjoint {
        description: "test".into(),
    };

    let id = driver.prepare(plan, evidence).unwrap();
    let state = driver.run(id).unwrap();

    assert_eq!(state, IntentState::NeedsReview);
    assert_eq!(
        executor.apply_calls.lock().unwrap().len(),
        0,
        "contested step must NOT call apply"
    );
}

#[test]
fn ilrp_recover_all_terminalizes_each_nonterminal_state() {
    let dir = fresh_dir("recover");
    let store = GraphStore::open(&dir).unwrap();

    // Hand-craft intents at each nonterminal state.
    let states_and_expected = [
        (IntentState::Prepared, IntentState::Committed),
        (IntentState::Applying, IntentState::Committed),
        (IntentState::ExternalApplied, IntentState::Committed),
        (IntentState::Finalizing, IntentState::Committed),
    ];

    for (initial, _) in &states_and_expected {
        let plan = single_step_plan("note.md", b"data");
        let intent = RepairIntent {
            plan,
            state: *initial,
            evidence: SafetyEvidence::StructurallyDisjoint {
                description: "test".into(),
            },
            acks: BTreeMap::new(),
        };
        hand_craft_intent(&store, &intent);
    }

    // For Prepared states, the executor will be called; set up verify → Prestate.
    let executor = MockExecutor::new();
    for _ in 0..states_and_expected.len() {
        executor.push_verify(PrestateMatch::Prestate);
        executor.push_apply(Ok(StepAck {
            step: RepairStepId::new(),
            observed_poststate: StatePredicate::Any,
            at: Timestamp::now(),
        }));
    }

    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: NoCrash,
    };

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
    let store = GraphStore::open(&dir).unwrap();
    let executor = MockExecutor::new();

    // Prepare + run one intent to committed.
    executor.push_verify(PrestateMatch::Prestate);
    executor.push_apply(Ok(StepAck {
        step: RepairStepId::new(),
        observed_poststate: StatePredicate::Any,
        at: Timestamp::now(),
    }));

    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: NoCrash,
    };

    let plan = single_step_plan("note.md", b"hello");
    let evidence = SafetyEvidence::StructurallyDisjoint {
        description: "test".into(),
    };

    let id = driver.prepare(plan, evidence).unwrap();
    driver.run(id).unwrap();

    let head_after_run = store.head().unwrap();

    // First recover: nothing nonterminal → empty.
    let r1 = driver.recover_all().unwrap();
    assert!(r1.is_empty(), "all intents already terminal");

    // Second recover: same result, head unchanged.
    let r2 = driver.recover_all().unwrap();
    assert!(r2.is_empty());
    assert_eq!(
        store.head().unwrap(),
        head_after_run,
        "head must not change"
    );
}
