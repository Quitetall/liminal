//! Day-one durability tests for the toy §92 store (M1 exit gate,
//! docs/implementation-plan.md).
//!
//! Every durability assertion re-opens the store fresh — nothing is trusted
//! from the writing handle's memory.

use camino::Utf8PathBuf;
use liminal_graph::{GraphStore, Node, NodeFlags, Operation, Origin, PayloadRef, TxnMeta};
use liminal_id::{KindId, NodeId, RevisionId, Timestamp};
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
        "liminal-store-test-{name}-{}-{:x}",
        std::process::id(),
        Timestamp::now().0
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn create_node(store: &GraphStore, text: &str) -> NodeId {
    let id = NodeId::new();
    let mut txn = store.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id,
            kind: KindId(0),
            payload: PayloadRef::Text(text.to_owned()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.commit(meta()).unwrap();
    id
}

fn log_segment_path(dir: &Utf8PathBuf) -> Utf8PathBuf {
    dir.join("log.000000.ndjson")
}

#[test]
fn commits_survive_reopen() {
    let dir = fresh_dir("reopen");
    let id = {
        let store = GraphStore::open(&dir).unwrap();
        create_node(&store, "hello")
    };
    let store = GraphStore::open(&dir).unwrap();
    let head = store.head().unwrap();
    let node = store.node_at(head, id).unwrap().unwrap();
    assert_eq!(node.payload, PayloadRef::Text("hello".to_owned()));
}

#[test]
fn aux_commits_atomically_with_ops() {
    let dir = fresh_dir("aux");
    let id = NodeId::new();
    {
        let store = GraphStore::open(&dir).unwrap();
        let mut txn = store.begin().unwrap();
        txn.apply(Operation::CreateNode {
            node: Node {
                id,
                kind: KindId(0),
                payload: PayloadRef::None,
                revision: RevisionId(0),
                flags: NodeFlags::default(),
            },
        })
        .unwrap();
        txn.put_aux(
            "ilrp.intent",
            "intent-1",
            serde_json::json!({"state": "prepared"}),
        )
        .unwrap();
        txn.commit(meta()).unwrap();
    }
    let store = GraphStore::open(&dir).unwrap();
    let value = store.get_aux("ilrp.intent", "intent-1").unwrap().unwrap();
    assert_eq!(value["state"], "prepared");
    assert!(store.node_at(store.head().unwrap(), id).unwrap().is_some());
}

#[test]
fn invalid_transaction_leaves_no_trace() {
    let dir = fresh_dir("invalid");
    let store = GraphStore::open(&dir).unwrap();
    let head_before = store.head().unwrap();
    let mut txn = store.begin().unwrap();
    txn.apply(Operation::DeleteNode { id: NodeId::new() })
        .unwrap();
    assert!(txn.commit(meta()).is_err());
    assert_eq!(store.head().unwrap(), head_before);
    // And durably: reopening sees the same head.
    drop(store);
    let store = GraphStore::open(&dir).unwrap();
    assert_eq!(store.head().unwrap(), head_before);
}

#[test]
fn snapshot_replace_atomic_under_abort() {
    let dir = fresh_dir("snapshot-abort");
    let id = {
        let store = GraphStore::open(&dir).unwrap();
        let id = create_node(&store, "survives");
        store.snapshot().unwrap();
        id
    };
    // Simulate a crash BETWEEN tmp-write and rename on a later snapshot
    // attempt: a garbage .tmp must be ignored, the committed snapshot rules.
    std::fs::write(dir.join("snapshot.json.tmp"), b"{ torn garbage").unwrap();
    let store = GraphStore::open(&dir).unwrap();
    let node = store.node_at(store.head().unwrap(), id).unwrap().unwrap();
    assert_eq!(node.payload, PayloadRef::Text("survives".to_owned()));
}

#[test]
fn mid_file_corruption_is_detected_never_served() {
    // §92: object/record corruption is DETECTED — a bit-flip before the tail
    // must fail loudly, not replay partially.
    let dir = fresh_dir("bitflip");
    {
        let store = GraphStore::open(&dir).unwrap();
        create_node(&store, "one");
        create_node(&store, "two");
        create_node(&store, "three");
    }
    let path = log_segment_path(&dir);
    let mut bytes = std::fs::read(&path).unwrap();
    // Flip a byte inside the FIRST record's JSON (well before the tail).
    let target = 20;
    bytes[target] ^= 0x40;
    std::fs::write(&path, &bytes).unwrap();
    let err = GraphStore::open(&dir).unwrap_err();
    assert!(
        err.to_string().contains("corrupt") || err.to_string().contains("invalid record"),
        "expected loud corruption error, got: {err}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// M1 exit gate `prop_torn_tail_truncates_cleanly`: write N records,
    /// truncate the log at an arbitrary byte, reopen — the store recovers
    /// exactly the longest checksummed prefix, never panics, never serves a
    /// partial record (v4 §92).
    #[test]
    fn prop_torn_tail_truncates_cleanly(
        payload_sizes in prop::collection::vec(1usize..64, 1..8),
        cut_fraction in 0.0f64..1.0,
    ) {
        let dir = fresh_dir("torn");
        let mut committed = Vec::new();
        {
            let store = GraphStore::open(&dir).unwrap();
            for (i, size) in payload_sizes.iter().enumerate() {
                let text = format!("record-{i}-{}", "x".repeat(*size));
                committed.push(create_node(&store, &text));
            }
        }

        // Tear the log at an arbitrary byte offset.
        let path = log_segment_path(&dir);
        let bytes = std::fs::read(&path).unwrap();
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test-only offset arithmetic on small files"
        )]
        let cut = ((bytes.len() as f64) * cut_fraction) as usize;
        std::fs::write(&path, &bytes[..cut]).unwrap();

        // Reopen: must not panic, must recover a clean prefix.
        let store = GraphStore::open(&dir).unwrap();
        let head = store.head().unwrap();
        let survivors = usize::try_from(head.0).unwrap();
        prop_assert!(survivors <= committed.len());

        // Every surviving record is fully intact (no partial payloads).
        for id in committed.iter().take(survivors) {
            let node = store.node_at(head, *id).unwrap();
            prop_assert!(node.is_some(), "surviving record must be readable");
            let payload = node.unwrap().payload;
            prop_assert!(
                matches!(&payload, PayloadRef::Text(t) if t.starts_with("record-")),
                "payload must be intact, got {payload:?}"
            );
        }
        // Every torn-off record is fully absent.
        for id in committed.iter().skip(survivors) {
            prop_assert!(store.node_at(head, *id).unwrap().is_none());
        }

        // Recovery is idempotent: a second reopen sees the identical world.
        drop(store);
        let store2 = GraphStore::open(&dir).unwrap();
        prop_assert_eq!(store2.head().unwrap(), head);
    }
}
