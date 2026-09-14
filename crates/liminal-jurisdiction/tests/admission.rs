//! AM-17.12 admission and receipt bypass controls.

use std::collections::BTreeMap;

use liminal_graph::ns::{ILRP_INTENT, SYS_BLOB};
use liminal_graph::{Node, NodeFlags, Operation, Origin, PayloadRef, StoreOwner, TxnMeta, kind};
use liminal_id::{
    ActorId, ContentHash, IdempotencyKey, JurisdictionKey, JurisdictionSubject, NodeId, PathId,
    RepairId, RepairStepId, RevisionId, Timestamp, TransactionId,
};
use liminal_jurisdiction::{
    Checker, ExternalExecutor, IlrpDriver, IlrpError, IntentState, NoCrash, PrestateMatch,
    ProfileSet, ProposedMutation, RepairIntent, RepairOperation, RepairPlan, StatePredicate,
    StepAck,
};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};

fn fresh_owner(name: &str) -> (liminal_scratch::ScratchDir, StoreOwner) {
    let dir = liminal_scratch::ScratchDir::new(&format!("admission-{name}")).unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    (dir, owner)
}

fn empty_basis() -> WorkspaceBasis {
    WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    }
}

fn add_comment(owner: &StoreOwner, payload: &str) -> NodeId {
    let id = NodeId::new();
    let mut txn = owner.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id,
            kind: kind::COMMENT,
            payload: PayloadRef::Text(payload.into()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp(1),
        provenance: Some("test:trusted-setup".into()),
        inverse: None,
    })
    .unwrap();
    id
}

fn add_file(owner: &StoreOwner, path: &str, mirrored: &[u8]) -> NodeId {
    let id = NodeId::new();
    let writer = owner.bootstrap_writer();
    let mut txn = writer.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id,
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
        serde_json::Value::String(String::from_utf8(mirrored.to_vec()).unwrap()),
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
    id
}

fn file_plan(subject: NodeId, path: &str, old: &[u8], new: &[u8]) -> RepairPlan {
    let path = PathId(path.into());
    let old_hash = ContentHash::of(old);
    let step = RepairStepId::new();
    RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                JurisdictionKey::Path(path.clone()),
                BasisComponent::FileContent {
                    path: path.clone(),
                    hash: old_hash,
                },
            )]),
        },
        steps: BTreeMap::from([(
            step,
            ProposedMutation {
                id: step,
                subject: JurisdictionSubject::Node(subject),
                operation: RepairOperation::WriteFile {
                    path: path.clone(),
                    contents: new.to_vec(),
                },
                expected_prestate: StatePredicate::FileContent {
                    path: path.clone(),
                    hash: old_hash,
                },
                expected_poststate: StatePredicate::FileContent {
                    path,
                    hash: ContentHash::of(new),
                },
                idempotency_key: IdempotencyKey::new(),
            },
        )]),
        dependencies: vec![],
        inverse: None,
    }
}

fn graph_plan(subject: NodeId, operation_target: NodeId) -> RepairPlan {
    let step = RepairStepId::new();
    RepairPlan {
        id: RepairId::new(),
        basis: empty_basis(),
        steps: BTreeMap::from([(
            step,
            ProposedMutation {
                id: step,
                subject: JurisdictionSubject::Node(subject),
                operation: RepairOperation::Graph(Operation::SetPayload {
                    id: operation_target,
                    payload: PayloadRef::Text("new".into()),
                }),
                expected_prestate: StatePredicate::NodeAt {
                    node: operation_target,
                    revision: RevisionId(0),
                },
                expected_poststate: StatePredicate::NodeAt {
                    node: operation_target,
                    revision: RevisionId(1),
                },
                idempotency_key: IdempotencyKey::new(),
            },
        )]),
        dependencies: vec![],
        inverse: None,
    }
}

#[derive(Debug)]
struct NoExternalEffects;

impl ExternalExecutor for NoExternalEffects {
    fn verify(&self, _: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        panic!("graph-only repair must not verify an external effect")
    }

    fn apply(&self, _: &ProposedMutation) -> Result<StepAck, IlrpError> {
        panic!("graph-only repair must not apply an external effect")
    }
}

#[test]
fn admission_refuses_an_operation_labeled_as_another_subject() {
    let (_dir, owner) = fresh_owner("subject");
    let subject = add_comment(&owner, "old");
    let target = add_comment(&owner, "old");
    let plan = graph_plan(subject, target);
    let profiles = ProfileSet::phase_minus_1();
    let checker = Checker {
        store: owner.store(),
        profiles: &profiles,
    };
    let before = owner.head().unwrap();

    assert!(checker.authorize_repair(plan).is_err());
    assert_eq!(owner.head().unwrap(), before);
    assert_eq!(
        owner.node_at(before, target).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
}

#[test]
fn authority_from_another_store_cannot_prepare_or_mutate() {
    let (_first_dir, first) = fresh_owner("cross-store-first");
    let (_second_dir, second) = fresh_owner("cross-store-second");
    let node = add_comment(&first, "old");
    let profiles = ProfileSet::phase_minus_1();
    let authorized = Checker {
        store: first.store(),
        profiles: &profiles,
    }
    .authorize_repair(graph_plan(node, node))
    .unwrap();
    let before = second.head().unwrap();
    let driver = IlrpDriver::new(second.coordinator_writer(), NoExternalEffects, NoCrash);

    assert!(driver.prepare(authorized).is_err());
    assert_eq!(second.head().unwrap(), before);
    assert!(second.scan_aux(ILRP_INTENT).unwrap().is_empty());
    assert_eq!(
        first
            .node_at(first.head().unwrap(), node)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );
}

#[test]
fn authority_becomes_stale_without_persisting_an_intent() {
    let (_dir, owner) = fresh_owner("stale");
    let node = add_comment(&owner, "old");
    let profiles = ProfileSet::phase_minus_1();
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .authorize_repair(graph_plan(node, node))
    .unwrap();

    let unrelated = add_comment(&owner, "concurrent");
    let after_concurrent_write = owner.head().unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);

    assert!(driver.prepare(authorized).is_err());
    assert_eq!(owner.head().unwrap(), after_concurrent_write);
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());
    assert!(
        owner
            .node_at(after_concurrent_write, unrelated)
            .unwrap()
            .is_some()
    );
}

#[test]
fn handcrafted_terminal_intent_cannot_mint_a_receipt() {
    let (_dir, owner) = fresh_owner("forged-terminal");
    let node = add_comment(&owner, "old");
    let plan = graph_plan(node, node);
    let id = plan.id;
    let step = plan.steps.values().next().unwrap();
    let forged = RepairIntent {
        plan: plan.clone(),
        state: IntentState::Committed,
        evidence: liminal_jurisdiction::SafetyEvidence::HumanApproval {
            actor: ActorId::new(),
            at: Timestamp(2),
        },
        acks: BTreeMap::from([(
            step.id,
            StepAck {
                step: step.id,
                observed_poststate: step.expected_poststate.clone(),
                at: Timestamp(3),
            },
        )]),
    };
    let mut txn = owner.begin().unwrap();
    txn.put_aux(
        ILRP_INTENT,
        &id.to_string(),
        serde_json::to_value(forged).unwrap(),
    )
    .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp(4),
        provenance: Some("ilrp:finalize".into()),
        inverse: None,
    })
    .unwrap();
    let before = owner.head().unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);

    assert!(driver.run_checked(id).is_err());
    assert_eq!(owner.head().unwrap(), before);
    assert_eq!(
        owner.node_at(before, node).unwrap().unwrap().payload,
        PayloadRef::Text("old".into())
    );
}

#[test]
fn checked_receipt_is_durable_and_idempotent() {
    let (_dir, owner) = fresh_owner("receipt");
    let node = add_comment(&owner, "old");
    let profiles = ProfileSet::phase_minus_1();
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .authorize_repair(graph_plan(node, node))
    .unwrap();
    let id = authorized.plan().id;
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    driver.prepare(authorized).unwrap();

    let first = driver.run_checked(id).unwrap();
    assert_eq!(first.state(), IntentState::Committed);
    let first_receipt = first.committed().expect("committed receipt");
    assert_eq!(first_receipt.repair(), id);
    assert!(first_receipt.belongs_to(owner.store()));
    assert_eq!(
        first_receipt.resulting_basis().transaction,
        first_receipt.transaction()
    );
    assert_eq!(
        first_receipt
            .resulting_basis()
            .components
            .get(&liminal_revision::graph_key()),
        Some(&BasisComponent::GraphSnapshot {
            revision: first_receipt.revision()
        })
    );
    assert_eq!(
        owner
            .node_at(first_receipt.revision(), node)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("new".into())
    );
    let first_revision = first_receipt.revision();
    let first_transaction = first_receipt.transaction();
    let head_after_first = owner.head().unwrap();

    let second = driver.run_checked(id).unwrap();
    let second_receipt = second.committed().expect("same durable receipt");
    assert_eq!(second.state(), IntentState::Committed);
    assert_eq!(second_receipt.revision(), first_revision);
    assert_eq!(second_receipt.transaction(), first_transaction);
    assert_eq!(owner.head().unwrap(), head_after_first);
}

#[test]
fn automatic_admission_accepts_hash_matching_durable_file_mirror() {
    let (_dir, owner) = fresh_owner("mirror-fallback");
    let old = b"old durable bytes";
    let node = add_file(&owner, "note.md", old);
    let plan = file_plan(node, "note.md", old, b"new durable bytes");
    let hash_key = ContentHash::of(old).to_hex();
    assert!(
        owner.get_aux(SYS_BLOB, &hash_key).unwrap().is_none(),
        "fixture must exercise file/path mirror fallback, not content-address storage"
    );
    let profiles = ProfileSet::phase_minus_1();
    let before = owner.head().unwrap();

    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .authorize_repair(plan);

    assert!(
        authorized.is_ok(),
        "matching durable mirror is captured prestate evidence: {authorized:?}"
    );
    assert_eq!(owner.head().unwrap(), before);
}

#[test]
fn automatic_admission_refuses_missing_prestate_bytes_even_with_inverse() {
    let (_dir, owner) = fresh_owner("missing-prestate");
    let expected = b"expected durable bytes";
    let node = add_file(&owner, "note.md", b"different durable bytes");
    let mut plan = file_plan(node, "note.md", expected, b"new durable bytes");
    plan.inverse = Some(liminal_jurisdiction::InverseRepairPlan(Box::new(
        RepairPlan {
            id: RepairId::new(),
            basis: empty_basis(),
            steps: BTreeMap::new(),
            dependencies: vec![],
            inverse: None,
        },
    )));
    let profiles = ProfileSet::phase_minus_1();
    let before = owner.head().unwrap();

    assert!(
        Checker {
            store: owner.store(),
            profiles: &profiles,
        }
        .authorize_repair(plan)
        .is_err(),
        "inverse existence cannot replace captured prestate bytes"
    );
    assert_eq!(owner.head().unwrap(), before);
    assert!(owner.scan_aux(ILRP_INTENT).unwrap().is_empty());
}

#[test]
fn impossible_graph_poststate_cannot_produce_a_committed_receipt() {
    let (_dir, owner) = fresh_owner("graph-poststate");
    let node = add_comment(&owner, "old");
    let mut plan = graph_plan(node, node);
    plan.steps.values_mut().next().unwrap().expected_poststate = StatePredicate::NodeAt {
        node,
        revision: RevisionId(99),
    };
    let profiles = ProfileSet::phase_minus_1();
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .authorize_repair(plan)
    .unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    let id = driver.prepare(authorized).unwrap();
    let result = driver.run_checked(id).unwrap();
    assert_eq!(result.state(), IntentState::NeedsReview);
    assert!(result.committed().is_none());
    assert_eq!(
        owner
            .node_at(owner.head().unwrap(), node)
            .unwrap()
            .unwrap()
            .payload,
        PayloadRef::Text("old".into())
    );
}

#[test]
fn graph_drift_after_prepare_preserves_other_writers_change() {
    let (_dir, owner) = fresh_owner("graph-drift");
    let node = add_comment(&owner, "old");
    let profiles = ProfileSet::phase_minus_1();
    let authorized = Checker {
        store: owner.store(),
        profiles: &profiles,
    }
    .authorize_repair(graph_plan(node, node))
    .unwrap();
    let driver = IlrpDriver::new(owner.coordinator_writer(), NoExternalEffects, NoCrash);
    let id = driver.prepare(authorized).unwrap();
    let mut other = owner.begin().unwrap();
    other
        .apply(Operation::SetPayload {
            id: node,
            payload: PayloadRef::Text("other writer".into()),
        })
        .unwrap();
    other
        .commit(TxnMeta {
            actor: None,
            origin: Origin::Human,
            at: Timestamp(10),
            provenance: None,
            inverse: None,
        })
        .unwrap();
    for _ in 0..2 {
        let result = driver.run_checked(id).unwrap();
        assert_eq!(result.state(), IntentState::NeedsReview);
        assert!(result.committed().is_none());
        assert_eq!(
            owner
                .node_at(owner.head().unwrap(), node)
                .unwrap()
                .unwrap()
                .payload,
            PayloadRef::Text("other writer".into())
        );
    }
}
