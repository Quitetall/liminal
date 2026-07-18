//! M08.1 as-of read tests: `state_at(head)` equals the in-memory head state,
//! and `state_at(r)` is stable across a snapshot rotation (segments are never
//! GC'd, so genesis replay always reconstructs the same historical state).

use camino::Utf8PathBuf;
use liminal_graph::{GraphStore, Node, NodeFlags, Operation, Origin, PayloadRef, TxnMeta};
use liminal_id::{GraphRevisionId, KindId, NodeId, RevisionId, Timestamp};
use proptest::prelude::*;

fn meta() -> TxnMeta {
    TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: None,
        inverse: None,
    }
}

fn fresh_dir(name: &str) -> Utf8PathBuf {
    let dir = Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
        "liminal-asof-{name}-{}-{:x}",
        std::process::id(),
        Timestamp::now().0
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Append one text node, returning its id and the revision it was created at.
fn commit_node(store: &GraphStore, text: &str) -> (NodeId, GraphRevisionId) {
    let id = NodeId::new();
    let mut txn = store.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id,
            kind: KindId(2),
            payload: PayloadRef::Text(text.to_owned()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.commit(meta()).unwrap();
    (id, store.head().unwrap())
}

#[test]
fn asof_head_equals_in_memory_state() {
    let dir = fresh_dir("head-eq");
    let store = GraphStore::open(&dir).unwrap();
    let (a, _) = commit_node(&store, "alpha");
    let (b, _) = commit_node(&store, "beta");

    let head = store.head().unwrap();
    let view = store.state_at(head).unwrap();
    assert_eq!(view.head(), head);
    // Same nodes, same payloads as the head fast path.
    assert_eq!(view.node(a), store.node_at(head, a).unwrap().as_ref());
    assert_eq!(view.node(b), store.node_at(head, b).unwrap().as_ref());
}

#[test]
fn asof_past_revision_reconstructs_history() {
    let dir = fresh_dir("past");
    let store = GraphStore::open(&dir).unwrap();
    let (a, rev_a) = commit_node(&store, "alpha");
    let (b, rev_b) = commit_node(&store, "beta");

    // At rev_a only `a` exists; `b` was created later.
    let at_a = store.state_at(rev_a).unwrap();
    assert!(at_a.node(a).is_some(), "a exists at its own revision");
    assert!(at_a.node(b).is_none(), "b does not exist yet at rev_a");
    assert_eq!(at_a.head(), rev_a);

    // At rev_b both exist.
    let at_b = store.state_at(rev_b).unwrap();
    assert!(at_b.node(a).is_some());
    assert!(at_b.node(b).is_some());
}

#[test]
fn asof_future_revision_errors() {
    let dir = fresh_dir("future");
    let store = GraphStore::open(&dir).unwrap();
    commit_node(&store, "alpha");
    let head = store.head().unwrap();
    let err = store.state_at(GraphRevisionId(head.0 + 1)).unwrap_err();
    assert!(
        format!("{err}").contains("ahead of head"),
        "unexpected error: {err}"
    );
}

#[test]
fn asof_stable_across_snapshot_rotation() {
    let dir = fresh_dir("rotate");
    let store = GraphStore::open(&dir).unwrap();
    let (a, rev_a) = commit_node(&store, "alpha");
    let (_b, rev_b) = commit_node(&store, "beta");

    // Capture the historical view before rotation.
    let before = store.state_at(rev_a).unwrap();
    let a_before = before.node(a).cloned();

    // Rotate: write a snapshot at head and start a fresh segment. Old segments
    // are retained, so genesis replay must still reconstruct rev_a identically.
    store.snapshot().unwrap();
    let (_c, _rev_c) = commit_node(&store, "gamma");

    let after = store.state_at(rev_a).unwrap();
    assert_eq!(
        after.node(a).cloned(),
        a_before,
        "rev_a drifted after rotation"
    );
    assert!(after.node(a).is_some());
    // rev_b still reconstructs (spans the pre-rotation segment).
    let at_b = store.state_at(rev_b).unwrap();
    assert_eq!(at_b.head(), rev_b);
}

proptest! {
    /// `state_at(head)` always agrees with the head fast path for every created
    /// node, regardless of how many commits and rotations happened.
    #[test]
    fn prop_asof_head_matches_fast_path(ops in 1usize..12, rotate_at in 0usize..12) {
        let dir = fresh_dir("prop");
        let store = GraphStore::open(&dir).unwrap();
        let mut ids = Vec::new();
        for i in 0..ops {
            let (id, _) = commit_node(&store, &format!("n{i}"));
            ids.push(id);
            if i == rotate_at {
                store.snapshot().unwrap();
            }
        }
        let head = store.head().unwrap();
        let view = store.state_at(head).unwrap();
        for id in ids {
            let fast = store.node_at(head, id).unwrap();
            prop_assert_eq!(view.node(id), fast.as_ref());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
