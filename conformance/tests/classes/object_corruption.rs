//! §92/§113 object-corruption class: damage to durable bytes is DETECTED,
//! never silently served. Partially REAL today against the toy store; the
//! content-addressed object-store variant arrives with `liminal-resource`.

use liminal_graph::{GraphStore, Node, NodeFlags, Operation, Origin, PayloadRef, TxnMeta};
use liminal_id::{KindId, NodeId, RevisionId, Timestamp};

fn fresh_dir(name: &str) -> liminal_scratch::ScratchDir {
    liminal_scratch::ScratchDir::new(&format!("conf-corrupt-{name}")).expect("scratch dir")
}

fn seed_store(dir: &camino::Utf8Path) {
    let store = GraphStore::open(dir).unwrap();
    let mut txn = store.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: Node {
            id: NodeId::new(),
            kind: KindId(0),
            payload: PayloadRef::Text("guarded".into()),
            revision: RevisionId(0),
            flags: NodeFlags::default(),
        },
    })
    .unwrap();
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: None,
        inverse: None,
    })
    .unwrap();
    store.snapshot().unwrap();
}

/// REAL today: a bit-flip in the committed snapshot fails loudly on open —
/// the store never guesses past a corrupt snapshot (v4 §92, §7.8 step 5).
#[test]
fn corrupt_snapshot_is_detected_never_served() {
    let dir = fresh_dir("snapshot");
    seed_store(&dir);
    let snap = dir.join("snapshot.json");
    let mut bytes = std::fs::read(&snap).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] = 0x00; // guaranteed-invalid JSON byte inside the document
    std::fs::write(&snap, &bytes).unwrap();
    let err = GraphStore::open(&dir).unwrap_err();
    assert!(
        err.to_string().contains("corrupt"),
        "expected loud corruption error, got: {err}"
    );
}

/// Content-addressed objects verify their hash on read (v4 §47, §92).
#[test]
#[ignore = "Phase 4: content-addressed object store (liminal-resource)"]
fn corrupt_object_bytes_fail_hash_verification() {
    unimplemented!("store object, flip a byte, read → integrity error naming the hash")
}
