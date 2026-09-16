//! Development-only consolidated resource controls; not qualification evidence.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use liminal_graph::ns::ILRP_INTENT;
use liminal_graph::{Node, NodeFlags, Operation, Origin, PayloadRef, StoreOwner, TxnMeta, kind};
use liminal_id::{
    ActorId, IdempotencyKey, JurisdictionSubject, NodeId, RepairId, RepairStepId, RevisionId,
    Timestamp, TransactionId,
};
use liminal_jurisdiction::{
    AdmissionError, AuthorizedRepair, Checker, CrashInjector, CrashPoint, ExternalExecutor,
    IlrpDriver, IlrpError, IntentState, NoCrash, PrestateMatch, ProfileSet, ProposedMutation,
    RepairOperation, RepairPlan, StatePredicate, StepAck,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

struct FailOneProofBuffer;

#[derive(Debug)]
struct PanicAtRecoveryBoundary {
    point: CrashPoint,
    reached: Rc<Cell<bool>>,
}

impl CrashInjector for PanicAtRecoveryBoundary {
    fn crash_if_armed(&self, at: CrashPoint) {
        if at == self.point {
            self.reached.set(true);
            panic!("controlled recovery cutoff")
        }
    }
}

fn recovery_after_cutoff_control(
    point: CrashPoint,
    expected_state: &str,
    promote_external_applied: bool,
) {
    let (_dir, owner, subject, profiles, plan) = fixture("resource-recovery-cutoff");
    let repair_id = plan.id;
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .accept_repair(plan, ActorId::new())
    .unwrap();
    let reached = Rc::new(Cell::new(false));
    let crash = PanicAtRecoveryBoundary {
        point,
        reached: Rc::clone(&reached),
    };
    let progress = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, crash);
    assert_eq!(progress.prepare(authorized).unwrap(), repair_id);
    let cutoff = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        progress.run(repair_id).unwrap();
    }))
    .expect_err("fixture must stop at the armed cutoff");
    assert!(
        reached.get(),
        "fixture must reach requested recovery cutoff"
    );
    assert_eq!(
        cutoff.downcast_ref::<&str>(),
        Some(&"controlled recovery cutoff")
    );

    let persisted = owner
        .get_aux(ILRP_INTENT, &repair_id.to_string())
        .unwrap()
        .unwrap();
    if promote_external_applied {
        // Trusted fixture assembly, not a crash witness: no runtime hook leaves
        // ExternalApplied durable before the immediate Finalizing transition.
        assert_eq!(persisted["state"].as_str(), Some("applying"));
        assert_eq!(persisted["acks"].as_object().unwrap().len(), 1);
        let mut promoted = persisted.clone();
        promoted["state"] = "external-applied".into();
        let mut txn = owner.begin().unwrap();
        txn.put_aux(ILRP_INTENT, &repair_id.to_string(), promoted)
            .unwrap();
        txn.commit(TxnMeta {
            actor: None,
            origin: Origin::Human,
            at: Timestamp::now(),
            provenance: Some("state:external-applied".into()),
            inverse: None,
        })
        .unwrap();
    }
    let persisted = owner
        .get_aux(ILRP_INTENT, &repair_id.to_string())
        .unwrap()
        .unwrap();
    assert_eq!(persisted["state"].as_str(), Some(expected_state));
    assert_eq!(persisted["acks"].as_object().unwrap().len(), 1);
    assert_eq!(
        owner
            .node_at(owner.head().unwrap(), subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );

    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    let head_before = owner.head().unwrap();
    let aux_before = owner.scan_aux(ILRP_INTENT).unwrap();
    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = driver.recover_all();
    ARMED.store(false, Ordering::Release);

    assert!(matches!(&result, Err(IlrpError::ResourceExhaustion(_))));
    assert_eq!(
        result.as_ref().unwrap_err().to_string(),
        "ordering proof resources exhausted"
    );
    assert_eq!(MATCHED.load(Ordering::Acquire), 1);
    assert_eq!(owner.head().unwrap(), head_before);
    assert_eq!(owner.scan_aux(ILRP_INTENT).unwrap(), aux_before);
    assert_eq!(
        owner
            .node_at(head_before, subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );

    assert_eq!(
        driver.recover_all().unwrap(),
        vec![(repair_id, IntentState::Committed)]
    );
    let committed_head = owner.head().unwrap();
    assert_eq!(
        owner
            .node_at(committed_head, subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("new".into())
    );
}

fn recovery_control() {
    let (_dir, owner, subject, profiles, plan) = fixture("resource-recovery");
    let repair_id = plan.id;
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .accept_repair(plan, ActorId::new())
    .unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    let prepared_id = driver.prepare(authorized).unwrap();
    assert_eq!(prepared_id, repair_id);
    let head_before = owner.head().unwrap();
    let aux_before = owner.scan_aux(ILRP_INTENT).unwrap();

    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = driver.recover_all();
    ARMED.store(false, Ordering::Release);

    assert!(
        matches!(&result, Err(IlrpError::ResourceExhaustion(_))),
        "recovery must refuse before progress: {result:?}"
    );
    assert_eq!(
        result.as_ref().unwrap_err().to_string(),
        "ordering proof resources exhausted"
    );
    assert_eq!(MATCHED.load(Ordering::Acquire), 1);
    assert_eq!(owner.head().unwrap(), head_before);
    assert_eq!(owner.scan_aux(ILRP_INTENT).unwrap(), aux_before);
    assert_eq!(
        owner
            .node_at(head_before, subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );

    let recovered = driver.recover_all().unwrap();
    assert_eq!(recovered, vec![(repair_id, IntentState::Committed)]);
    let committed_head = owner.head().unwrap();
    assert_eq!(
        owner
            .node_at(committed_head, subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("new".into())
    );
    assert!(driver.recover_all().unwrap().is_empty());
}

#[derive(Debug)]
struct NoExternalEffects;

impl ExternalExecutor for NoExternalEffects {
    fn verify(&self, _: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        panic!("graph-only prepare must not verify an external effect")
    }

    fn apply(&self, _: &ProposedMutation) -> Result<StepAck, IlrpError> {
        panic!("graph-only prepare must not apply an external effect")
    }
}

fn prepare_control() {
    let (_dir, owner, subject, profiles, original_plan) = fixture("resource-prepare");
    let checker = Checker {
        store: owner.store(),
        profiles: &profiles,
    };
    let actor = ActorId::new();
    let authorized = checker.accept_repair(original_plan.clone(), actor).unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    let head_before = owner.head().unwrap();

    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = driver.prepare(authorized);
    ARMED.store(false, Ordering::Release);

    assert!(matches!(&result, Err(IlrpError::ResourceExhaustion(_))));
    assert_eq!(
        result.as_ref().unwrap_err().to_string(),
        "ordering proof resources exhausted"
    );
    assert_eq!(MATCHED.load(Ordering::Acquire), 1);
    assert_eq!(owner.head().unwrap(), head_before);
    assert_eq!(
        owner
            .node_at(head_before, subject)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());

    let fresh_authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .accept_repair(original_plan.clone(), ActorId::new())
    .unwrap();
    let expected_id = original_plan.id;
    let prepared_id = driver.prepare(fresh_authorized).unwrap();
    assert_eq!(prepared_id, expected_id);
    assert!(owner.head().unwrap() > head_before);
    assert_eq!(owner.scan_aux(ILRP_INTENT).unwrap().len(), 1);
}

static ARMED: AtomicBool = AtomicBool::new(false);
static MATCHED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for FailOneProofBuffer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 48 && layout.align() == 16 && ARMED.load(Ordering::Acquire) {
            let prior = MATCHED.fetch_add(1, Ordering::AcqRel);
            if prior == 0 {
                // SAFETY: null is the allocator contract for refusal.
                return std::ptr::null_mut();
            }
        }
        // SAFETY: the original layout is forwarded unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 48 && layout.align() == 16 && ARMED.load(Ordering::Acquire) {
            let prior = MATCHED.fetch_add(1, Ordering::AcqRel);
            if prior == 0 {
                // SAFETY: null is the allocator contract for refusal.
                return std::ptr::null_mut();
            }
        }
        // SAFETY: the original layout is forwarded unchanged to System.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the original pointer and layout are forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if size == 48 && layout.align() == 16 && ARMED.load(Ordering::Acquire) {
            let prior = MATCHED.fetch_add(1, Ordering::AcqRel);
            if prior == 0 {
                // SAFETY: null is the allocator contract for refusal.
                return std::ptr::null_mut();
            }
        }
        // SAFETY: the original pointer, layout, and requested size are forwarded unchanged.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: FailOneProofBuffer = FailOneProofBuffer;

fn add_comment(owner: &StoreOwner) -> NodeId {
    let id = NodeId::new();
    let mut txn = owner.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id,
            kind: kind::COMMENT,
            payload: PayloadRef::Text("old".into()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp(1),
        provenance: Some("probe:trusted-setup".into()),
        inverse: None,
    })
    .unwrap();
    id
}

fn graph_plan(subject: NodeId) -> RepairPlan {
    let step = RepairStepId::new();
    RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps: BTreeMap::from([(
            step,
            ProposedMutation {
                id: step,
                subject: JurisdictionSubject::Node(subject),
                operation: RepairOperation::Graph(Operation::SetPayload {
                    id: subject,
                    payload: PayloadRef::Text("new".into()),
                }),
                expected_prestate: StatePredicate::NodeAt {
                    node: subject,
                    revision: RevisionId(0),
                },
                expected_poststate: StatePredicate::NodeAt {
                    node: subject,
                    revision: RevisionId(1),
                },
                idempotency_key: IdempotencyKey::new(),
            },
        )]),
        dependencies: vec![],
        inverse: None,
    }
}

fn fixture(
    name: &str,
) -> (
    liminal_scratch::ScratchDir,
    StoreOwner,
    NodeId,
    ProfileSet,
    RepairPlan,
) {
    let dir = liminal_scratch::ScratchDir::new(name).unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let subject = add_comment(&owner);
    let profiles = ProfileSet::phase_minus_1();
    let plan = graph_plan(subject);
    (dir, owner, subject, profiles, plan)
}

fn assert_resource_refusal(result: Result<AuthorizedRepair<'_>, AdmissionError>) {
    assert!(result.is_err());
    assert_eq!(MATCHED.load(Ordering::Acquire), 1);
    assert_eq!(
        result.as_ref().unwrap_err().to_string(),
        "ordering proof resources exhausted"
    );
    assert!(matches!(result, Err(AdmissionError::ResourceExhaustion(_))));
}

fn assert_precedence_refusal(result: Result<AuthorizedRepair<'_>, AdmissionError>) {
    assert!(matches!(
        &result,
        Err(AdmissionError::Review(reasons))
            if reasons.len() == 1
                && reasons[0].0 == "plan key and step identity disagree"
    ));
    assert_eq!(MATCHED.load(Ordering::Acquire), 0);
}

fn malformed(mut plan: RepairPlan) -> RepairPlan {
    let key = *plan.steps.keys().next().unwrap();
    plan.steps.values_mut().next().unwrap().id = RepairStepId::new();
    assert_ne!(key, plan.steps.values().next().unwrap().id);
    plan
}

fn main() {
    assert_eq!(std::mem::size_of::<(u128, u128, bool)>(), 48);
    assert_eq!(std::mem::align_of::<(u128, u128, bool)>(), 16);

    let (_dir, owner, subject, profiles, valid) = fixture("order-resource-human");
    let checker = Checker {
        store: owner.store(),
        profiles: &profiles,
    };
    let head = owner.head().unwrap();
    assert!(checker.accept_repair(valid.clone(), ActorId::new()).is_ok());
    let actor = ActorId::new();
    let attempt = valid.clone();
    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = checker.accept_repair(attempt, actor);
    ARMED.store(false, Ordering::Release);
    assert_resource_refusal(result);
    assert_eq!(owner.head().unwrap(), head);
    assert_eq!(
        owner.node_at(head, subject).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());

    let malformed_human = malformed(valid);
    let actor = ActorId::new();
    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = checker.accept_repair(malformed_human, actor);
    ARMED.store(false, Ordering::Release);
    assert_precedence_refusal(result);
    assert_eq!(owner.head().unwrap(), head);
    assert_eq!(
        owner.node_at(head, subject).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());

    let (_dir, owner, subject, profiles, valid) = fixture("order-resource-automatic");
    let checker = Checker {
        store: owner.store(),
        profiles: &profiles,
    };
    let head = owner.head().unwrap();
    assert!(checker.authorize_repair(valid.clone()).is_ok());
    let attempt = valid.clone();
    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = checker.authorize_repair(attempt);
    ARMED.store(false, Ordering::Release);
    assert_resource_refusal(result);
    assert_eq!(owner.head().unwrap(), head);
    assert_eq!(
        owner.node_at(head, subject).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());

    let malformed_automatic = malformed(valid);
    MATCHED.store(0, Ordering::Release);
    ARMED.store(true, Ordering::Release);
    let result = checker.authorize_repair(malformed_automatic);
    ARMED.store(false, Ordering::Release);
    assert_precedence_refusal(result);
    assert_eq!(owner.head().unwrap(), head);
    assert_eq!(
        owner.node_at(head, subject).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());

    prepare_control();
    recovery_control();
    recovery_after_cutoff_control(CrashPoint::AfterAcknowledge, "applying", false);
    recovery_after_cutoff_control(CrashPoint::BeforeFinalize, "finalizing", false);
    recovery_after_cutoff_control(CrashPoint::AfterAcknowledge, "external-applied", true);
    println!(
        "liminal-resource-controls: admission resource/precedence, Prepare and all four nonterminal recovery resource controls passed; qualification=false"
    );
}
