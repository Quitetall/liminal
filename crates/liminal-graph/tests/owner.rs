//! AM-17.12: narrow epoch and working-buffer capabilities, before migration.

use liminal_graph::{StoreOwner, ns};
use liminal_id::{BufferId, ClientId, SessionEpoch};

#[test]
fn working_capture_is_volatile_and_bound_to_its_owner() {
    let first_dir = liminal_scratch::ScratchDir::new("owner-working-first").unwrap();
    let second_dir = liminal_scratch::ScratchDir::new("owner-working-second").unwrap();
    let first = StoreOwner::open(&first_dir).unwrap();
    let second = StoreOwner::open(&second_dir).unwrap();
    let client = ClientId::new();
    let buffer = BufferId::new();
    let key = format!("buf/{client}/{buffer}/7");
    let head = first.store().head().unwrap();
    first
        .working_capture()
        .put_buffer(client, buffer, 7, "draft")
        .unwrap();
    assert_eq!(
        first.store().get_aux(ns::SYS_BLOB, &key).unwrap(),
        Some(serde_json::json!("draft"))
    );
    assert_eq!(first.store().head().unwrap(), head);
    assert_eq!(second.store().get_aux(ns::SYS_BLOB, &key).unwrap(), None);
    assert!(first.store().scan_aux(ns::ILRP_INTENT).unwrap().is_empty());
    drop(first);
    let reopened = StoreOwner::open(&first_dir).unwrap();
    assert_eq!(reopened.store().get_aux(ns::SYS_BLOB, &key).unwrap(), None);
    assert_eq!(reopened.store().head().unwrap(), head);
}

#[test]
fn epoch_capability_commits_only_its_owners_epoch() {
    let first_dir = liminal_scratch::ScratchDir::new("owner-epoch-first").unwrap();
    let second_dir = liminal_scratch::ScratchDir::new("owner-epoch-second").unwrap();
    let first = StoreOwner::open(&first_dir).unwrap();
    let second = StoreOwner::open(&second_dir).unwrap();
    let meta = liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: liminal_id::Timestamp::now(),
        provenance: Some("epoch-test".into()),
        inverse: None,
    };
    first
        .epoch_writer()
        .persist(SessionEpoch(17), meta)
        .unwrap();
    assert_eq!(
        second.store().get_aux(ns::SYS_EPOCH, "epoch").unwrap(),
        None
    );
    assert!(first.store().scan_aux(ns::ILRP_INTENT).unwrap().is_empty());
    assert!(first.store().nodes().unwrap().is_empty());
    drop(first);
    let reopened = StoreOwner::open(&first_dir).unwrap();
    assert_eq!(
        reopened.store().get_aux(ns::SYS_EPOCH, "epoch").unwrap(),
        Some(serde_json::json!(17))
    );
}
