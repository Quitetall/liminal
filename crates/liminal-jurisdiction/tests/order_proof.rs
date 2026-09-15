//! Regression coverage for lossless repair-order proof projection.

use std::collections::BTreeMap;
use std::str::FromStr;

use liminal_graph::{Operation, PayloadRef};
use liminal_id::{NodeId, PathId, RepairId, RepairStepId, RevisionId};
use liminal_jurisdiction::order_proof::OrderProofInput;
use liminal_jurisdiction::{
    ProposedMutation, RepairDependency, RepairOperation, RepairPlan, StatePredicate,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

fn step_id(value: &str) -> RepairStepId {
    RepairStepId::from_str(value).expect("full-width repair step id")
}

fn empty_plan() -> RepairPlan {
    RepairPlan {
        id: RepairId::from_str("repair:00000000-0000-7000-8000-000000000010").unwrap(),
        basis: WorkspaceBasis {
            transaction: liminal_id::TransactionId::from_str(
                "txn:00000000-0000-7000-8000-000000000011",
            )
            .unwrap(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps: BTreeMap::new(),
        dependencies: vec![],
        inverse: None,
    }
}

#[test]
fn order_proof_empty_inputs_have_empty_views() {
    let plan = empty_plan();
    let proof = OrderProofInput::try_new(&plan, &[], &[]).unwrap();
    let view = proof.view();
    assert!(view.steps.is_empty());
    assert!(view.dependencies.is_empty());
    assert!(view.schedule.is_empty());
    assert!(view.applied.is_empty());
}

#[test]
fn order_proof_preserves_declared_map_graph_and_passed_orderings() {
    let map_id = step_id("step:ffffffff-ffff-7fff-8fff-ffffffffffff");
    let declared_id = step_id("step:00000000-0000-7000-8000-000000000001");
    let outside_id = step_id("step:00000000-0000-7000-8000-000000000002");
    let plan = RepairPlan {
        id: RepairId::from_str("repair:00000000-0000-7000-8000-000000000003").unwrap(),
        basis: WorkspaceBasis {
            transaction: liminal_id::TransactionId::from_str(
                "txn:00000000-0000-7000-8000-000000000004",
            )
            .unwrap(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps: [(
            map_id,
            ProposedMutation {
                id: declared_id,
                subject: liminal_id::JurisdictionSubject::Node(
                    NodeId::from_str("node:00000000-0000-7000-8000-000000000005").unwrap(),
                ),
                operation: RepairOperation::WriteFile {
                    path: PathId("proof.txt".into()),
                    contents: b"proof".to_vec(),
                },
                expected_prestate: StatePredicate::FileAbsent {
                    path: PathId("proof.txt".into()),
                },
                expected_poststate: StatePredicate::FileAbsent {
                    path: PathId("proof.txt".into()),
                },
                idempotency_key: liminal_id::IdempotencyKey::from_str(
                    "idem:00000000-0000-7000-8000-000000000006",
                )
                .unwrap(),
            },
        )]
        .into_iter()
        .collect(),
        dependencies: vec![
            RepairDependency {
                before: map_id,
                after: declared_id,
            },
            RepairDependency {
                before: map_id,
                after: declared_id,
            },
        ],
        inverse: None,
    };
    let schedule = vec![outside_id, map_id];
    let applied = vec![declared_id, outside_id];

    let proof = OrderProofInput::try_new(&plan, &schedule, &applied).unwrap();
    let view = proof.view();
    assert_eq!(
        view.steps,
        &[(
            0xffff_ffff_ffff_7fff_8fff_ffff_ffff_ffff,
            0x7000_8000_0000_0000_0001,
            false
        )]
    );
    assert_eq!(
        view.dependencies,
        &[
            (
                0xffff_ffff_ffff_7fff_8fff_ffff_ffff_ffff,
                0x7000_8000_0000_0000_0001
            ),
            (
                0xffff_ffff_ffff_7fff_8fff_ffff_ffff_ffff,
                0x7000_8000_0000_0000_0001
            ),
        ]
    );
    assert_eq!(
        view.schedule,
        &[
            0x7000_8000_0000_0000_0002,
            0xffff_ffff_ffff_7fff_8fff_ffff_ffff_ffff
        ]
    );
    assert_eq!(
        view.applied,
        &[0x7000_8000_0000_0000_0001, 0x7000_8000_0000_0000_0002]
    );
}

#[test]
fn order_proof_marks_graph_steps_and_sorts_map_keys_without_reordering_inputs() {
    let high = step_id("step:00000000-0000-7000-8000-000000000021");
    let low = step_id("step:00000000-0000-7000-8000-000000000020");
    let node = NodeId::from_str("node:00000000-0000-7000-8000-000000000022").unwrap();
    let mutation = |id, operation| ProposedMutation {
        id,
        subject: liminal_id::JurisdictionSubject::Node(node),
        operation,
        expected_prestate: StatePredicate::NodeAt {
            node,
            revision: RevisionId(0),
        },
        expected_poststate: StatePredicate::NodeAt {
            node,
            revision: RevisionId(1),
        },
        idempotency_key: liminal_id::IdempotencyKey::from_str(
            "idem:00000000-0000-7000-8000-000000000023",
        )
        .unwrap(),
    };
    let mut plan = empty_plan();
    plan.steps.insert(
        high,
        mutation(
            high,
            RepairOperation::WriteFile {
                path: PathId("proof.txt".into()),
                contents: b"proof".to_vec(),
            },
        ),
    );
    plan.steps.insert(
        low,
        mutation(
            low,
            RepairOperation::Graph(Operation::SetPayload {
                id: node,
                payload: PayloadRef::Text("graph".into()),
            }),
        ),
    );
    let schedule = vec![high, low];
    let applied = vec![low, high];

    let proof = OrderProofInput::try_new(&plan, &schedule, &applied).unwrap();
    let view = proof.view();
    assert_eq!(
        view.steps,
        &[
            (0x7000_8000_0000_0000_0020, 0x7000_8000_0000_0000_0020, true),
            (
                0x7000_8000_0000_0000_0021,
                0x7000_8000_0000_0000_0021,
                false
            ),
        ]
    );
    assert!(view.dependencies.is_empty());
    assert_eq!(
        view.schedule,
        &[0x7000_8000_0000_0000_0021, 0x7000_8000_0000_0000_0020]
    );
    assert_eq!(
        view.applied,
        &[0x7000_8000_0000_0000_0020, 0x7000_8000_0000_0000_0021]
    );
}
