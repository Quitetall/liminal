//! `claims_never_exceed_evidence` (Algorithm E): the corpus caps every declared
//! identity claim. A profile's declared `required_identity` may never exceed the
//! worst outcome the matrix demonstrates for its strategies, and every inline-id
//! degradation is either bounded by the declared floor or loudly surfaced by the
//! M03 checker (DG-7.1 resolution, v4 §19.2).

use std::collections::BTreeMap;

use liminal_graph::{
    Node, NodeFlags, Operation as GraphOp, Origin, PayloadRef, StoreOwner, TxnMeta, kind,
    ns::{JUR_ALIAS, SYS_BLOB},
};
use liminal_id::{
    EntityId, IdentityGrade, JurisdictionSubject, NodeId, RevisionId, Timestamp, TransactionId,
};
use liminal_jurisdiction::Checker;
use liminal_jurisdiction::profile::{
    ExternalFileProfile, GraphNativeProfile, JurisdictionProfile, ProfileSet,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

use crate::identity::Strategy;
use crate::identity::matrix::Matrix;
use crate::identity::ops::ManagedObservation;

/// The declared identity floor of the external-file profile (its
/// `ContinuityPolicy::required_identity`), read from the live profile — the
/// value the corpus must not let the profile over-claim.
#[must_use]
pub fn external_file_floor() -> IdentityGrade {
    profile_floor(
        &ExternalFileProfile,
        JurisdictionSubject::Node(NodeId::new()),
    )
}

/// The declared identity floor of the graph-native profile.
#[must_use]
pub fn graph_native_floor() -> IdentityGrade {
    profile_floor(
        &GraphNativeProfile,
        JurisdictionSubject::Node(NodeId::new()),
    )
}

/// Read a profile's `required_identity` from its contract. Uses a throwaway
/// empty store (the floor is a profile constant, independent of store state).
fn profile_floor(profile: &dyn JurisdictionProfile, subject: JurisdictionSubject) -> IdentityGrade {
    let dir = tempdir_for("floor");
    let owner = StoreOwner::open(&dir).expect("open floor store");
    profile
        .contract_for(subject, owner.store())
        .continuity
        .required_identity
}

/// The column cap (Algorithm E): the minimum `cap(cell)` over the rows counted
/// for this column. For `managed-graph` only **observed** rows count (graph-
/// native subjects have no file for git to mangle — v4 §7.6); every other
/// column counts all rows.
#[must_use]
pub fn column_cap(matrix: &Matrix, strat_idx: usize) -> IdentityGrade {
    let strat = matrix.strategies[strat_idx];
    let mut cap: Option<IdentityGrade> = None;
    for (op_idx, op) in matrix.operations.iter().enumerate() {
        if strat == Strategy::ManagedGraph
            && op.managed_observation() == ManagedObservation::Foreign
        {
            continue;
        }
        let cell_cap = matrix.cells[&(op_idx, strat_idx)].outcome.cap();
        cap = Some(match cap {
            Some(c) if c.strength() <= cell_cap.strength() => c,
            _ => cell_cap,
        });
    }
    cap.expect("every column has at least one counted row")
}

/// The index of a strategy column, or `None` if absent.
#[must_use]
pub fn strategy_index(matrix: &Matrix, strat: Strategy) -> Option<usize> {
    matrix.strategies.iter().position(|&s| s == strat)
}

/// Replay a post-op file through the M03 checker and return the finding count.
/// The file is ingested exactly as the runner does — paragraph nodes with
/// first-wins `{#id}` aliases (D03.3) — so a duplicated id surfaces as JUR042.
#[must_use]
pub fn checker_findings(text: &str, label: &str) -> usize {
    let dir = tempdir_for(label);
    let owner = StoreOwner::open(&dir).expect("open replay store");
    ingest_text(&owner, "notes.md", text);

    let profiles = ProfileSet::default();
    let checker = Checker {
        store: owner.store(),
        profiles: &profiles,
    };
    let basis = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    checker
        .check_workspace(&basis)
        .expect("check_workspace")
        .findings
        .len()
}

/// Minimal ingest matching `runner::ingest_files`: a FILE node, its raw bytes in
/// `SYS_BLOB`, and one PARAGRAPH node per block with a first-wins `{#id}` alias
/// (later duplicates get NO alias → the checker surfaces the orphaned durable-id
/// marker as JUR042).
fn ingest_text(owner: &StoreOwner, path: &str, text: &str) {
    let meta = || TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: Some("identity:replay".into()),
        inverse: None,
    };

    let file_node = NodeId::new();
    {
        let mut txn = owner.begin().expect("begin");
        txn.apply(GraphOp::CreateNode {
            node: Node {
                id: file_node,
                kind: kind::FILE,
                payload: PayloadRef::None,
                revision: RevisionId(0),
                flags: NodeFlags::default(),
            },
        })
        .expect("create file node");
        txn.put_aux(
            SYS_BLOB,
            &format!("file/{path}"),
            serde_json::Value::String(text.to_owned()),
        )
        .expect("put file blob");
        txn.commit(meta()).expect("commit file");
    }

    for (i, block) in liminal_source::paragraph::parse(text).iter().enumerate() {
        let flags = if block.id.is_some() {
            NodeFlags::HAS_DURABLE_ID
        } else {
            NodeFlags::default()
        };
        let para_node = NodeId::new();
        let mut txn = owner.begin().expect("begin");
        txn.apply(GraphOp::CreateNode {
            node: Node {
                id: para_node,
                kind: kind::PARAGRAPH,
                payload: PayloadRef::Text(block.text.clone()),
                revision: RevisionId(0),
                flags,
            },
        })
        .expect("create paragraph node");
        txn.apply(GraphOp::InsertChild {
            parent: file_node,
            child: para_node,
            index: i as u64,
        })
        .expect("insert child");
        if let Some(ref id) = block.id
            && owner
                .store()
                .get_aux(JUR_ALIAS, id)
                .expect("get alias")
                .is_none()
        {
            let entity = EntityId::new();
            txn.put_aux(
                JUR_ALIAS,
                id,
                serde_json::json!({
                    "node": para_node.to_string(),
                    "entity": entity.to_string(),
                }),
            )
            .expect("put alias");
        }
        txn.commit(meta()).expect("commit paragraph");
    }
}

/// A fresh scratch directory for an in-process store (deterministic-ish label,
/// removed if present so reruns start clean).
fn tempdir_for(label: &str) -> liminal_scratch::ScratchDir {
    crate::identity::gitenv::case_tmpdir(&format!("claims-{label}"))
        .expect("scratch dir for claims store")
}
