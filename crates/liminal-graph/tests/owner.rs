//! AM-17.12: narrow epoch and working-buffer capabilities, before migration.

use liminal_graph::{Node, NodeFlags, Operation, PayloadRef, StoreOwner, kind, ns};
use liminal_id::{
    BufferId, ClientId, ContentHash, GraphRevisionId, KindId, NodeId, RevisionId, SessionEpoch,
    SourceId, Timestamp,
};

fn meta() -> liminal_graph::TxnMeta {
    liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: Timestamp(1),
        provenance: None,
        inverse: None,
    }
}

fn node(id: NodeId, kind: KindId) -> Node {
    Node {
        id,
        kind,
        payload: PayloadRef::None,
        revision: RevisionId(0),
        flags: NodeFlags::default(),
    }
}

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
fn a_captured_generation_cannot_change_under_an_admitted_basis() {
    let dir = liminal_scratch::ScratchDir::new("immutable-working-generation").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let client = ClientId::new();
    let buffer = BufferId::new();
    let writer = owner.working_capture();
    let head = owner.head().unwrap();
    writer.put_buffer(client, buffer, 1, "selected").unwrap();
    writer.put_buffer(client, buffer, 1, "selected").unwrap();
    assert!(writer.put_buffer(client, buffer, 1, "substituted").is_err());
    writer
        .put_buffer(client, buffer, 2, "new generation")
        .unwrap();
    assert_eq!(
        owner
            .get_aux(ns::SYS_BLOB, &format!("buf/{client}/{buffer}/1"))
            .unwrap(),
        Some(serde_json::json!("selected"))
    );
    assert_eq!(owner.head().unwrap(), head);
}

#[test]
fn reactor_cannot_retarget_its_guard_after_target_kind_changes() {
    let dir = liminal_scratch::ScratchDir::new("reactor-scope-race").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let id = NodeId::new();
    let mut setup = owner.begin().unwrap();
    setup
        .apply(Operation::CreateNode {
            node: node(id, kind::EXTERNAL_VALUE),
        })
        .unwrap();
    setup.commit(meta()).unwrap();
    let source = SourceId::from_name("scope-race");
    let mut pending = owner.reactor_writer(source).begin().unwrap();
    pending
        .apply(Operation::MaterializeExternal {
            node: id,
            source,
            observed: ContentHash::of(b"observation"),
            at: Timestamp(2),
        })
        .unwrap();
    let mut changed = owner.begin().unwrap();
    changed.apply(Operation::DeleteNode { id }).unwrap();
    changed
        .apply(Operation::CreateNode {
            node: node(id, kind::COMMENT),
        })
        .unwrap();
    changed.commit(meta()).unwrap();
    let head = owner.head().unwrap();
    assert!(pending.expect_head(head).commit(meta()).is_err());
    assert_eq!(owner.head().unwrap(), head);
    assert_eq!(
        owner.node_at(head, id).unwrap().unwrap().payload,
        PayloadRef::None
    );
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
        at: Timestamp::now(),
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

#[test]
fn fault_grant_keeps_the_store_locked_until_it_is_dropped() {
    let dir = liminal_scratch::ScratchDir::new("owner-fault-lifetime").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let faults = owner.blob_fault_injector();
    drop(owner);
    assert!(matches!(
        StoreOwner::open(&dir),
        Err(liminal_graph::StoreError::Locked(_))
    ));
    drop(faults);
    let reopened = StoreOwner::open(&dir).unwrap();
    assert_eq!(reopened.store().head().unwrap(), GraphRevisionId(0));
}
#[test]
fn reconciliation_writer_refuses_other_namespaces_and_taints_transaction() {
    let dir = liminal_scratch::ScratchDir::new("reconciliation-authority").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let writer = owner.reconciliation_writer();
    let before = owner.store().head().unwrap();
    let mut txn = writer.begin().unwrap();
    txn.put_aux(
        ns::JUR_RECONCILE,
        "item",
        serde_json::json!({"status":"pending"}),
    )
    .unwrap();
    assert!(
        txn.put_aux(
            ns::ILRP_INTENT,
            "forged",
            serde_json::json!({"state":"committed"})
        )
        .is_err()
    );
    assert!(
        txn.commit(liminal_graph::TxnMeta {
            actor: None,
            origin: liminal_graph::Origin::Human,
            at: Timestamp(1),
            provenance: None,
            inverse: None,
        })
        .is_err()
    );
    assert_eq!(owner.store().head().unwrap(), before);
    assert!(
        owner
            .store()
            .scan_aux(ns::JUR_RECONCILE)
            .unwrap()
            .is_empty()
    );
    assert!(owner.store().scan_aux(ns::ILRP_INTENT).unwrap().is_empty());
}

#[test]
fn reconciliation_writer_is_durable_owner_bound_and_aux_only() {
    let first_dir = liminal_scratch::ScratchDir::new("debt-first").unwrap();
    let second_dir = liminal_scratch::ScratchDir::new("debt-second").unwrap();
    let first = StoreOwner::open(&first_dir).unwrap();
    let second = StoreOwner::open(&second_dir).unwrap();
    let meta = || liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: Timestamp(1),
        provenance: None,
        inverse: None,
    };
    let writer = first.reconciliation_writer();
    let mut txn = writer.begin().unwrap();
    txn.put_aux(ns::JUR_RECONCILE, "item", serde_json::json!("pending"))
        .unwrap();
    txn.commit(meta()).unwrap();
    assert_eq!(
        second.store().get_aux(ns::JUR_RECONCILE, "item").unwrap(),
        None
    );
    let before = first.store().head().unwrap();
    for namespace in [
        ns::ILRP_INTENT,
        ns::JUR_PLAN,
        ns::JUR_DECISION,
        ns::JUR_REPAIR,
        ns::JUR_ALIAS,
        ns::JUR_OVERLAY,
        ns::JUR_OVERLAY_LOG,
        ns::SYS_BLOB,
        ns::SYS_EPOCH,
        ns::SYS_CLOCK,
        ns::SYS_UNAVAILABLE,
    ] {
        let mut txn = writer.begin().unwrap();
        assert!(txn.delete_aux(namespace, "item").is_err());
        assert!(txn.commit(meta()).is_err());
    }
    let mut txn = writer.begin().unwrap();
    assert!(
        txn.apply(Operation::DeleteNode { id: NodeId::new() })
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());
    assert_eq!(first.store().head().unwrap(), before);
    drop(first);
    let reopened = StoreOwner::open(&first_dir).unwrap();
    assert_eq!(
        reopened.store().get_aux(ns::JUR_RECONCILE, "item").unwrap(),
        Some(serde_json::json!("pending"))
    );
    let writer = reopened.reconciliation_writer();
    let mut txn = writer.begin().unwrap();
    txn.delete_aux(ns::JUR_RECONCILE, "item").unwrap();
    txn.commit(meta()).unwrap();
    assert_eq!(
        reopened.store().get_aux(ns::JUR_RECONCILE, "item").unwrap(),
        None
    );
}

#[test]
fn scoped_writers_accept_only_their_named_surfaces() {
    let dir = liminal_scratch::ScratchDir::new("all-scoped-writers").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();

    let bootstrap_node = NodeId::new();
    let mut txn = owner.bootstrap_writer().begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: node(bootstrap_node, kind::FILE),
    })
    .unwrap();
    txn.put_aux(
        ns::JUR_ALIAS,
        "file",
        serde_json::json!({"node": bootstrap_node}),
    )
    .unwrap();
    txn.put_aux(ns::SYS_BLOB, "file/notes.md", serde_json::json!("notes"))
        .unwrap();
    txn.commit(meta()).unwrap();

    let content_key = ContentHash::of(b"draft").to_hex();
    let mut txn = owner.capture_writer().begin().unwrap();
    for namespace in [
        ns::JUR_PLAN,
        ns::JUR_DECISION,
        ns::JUR_OVERLAY,
        ns::JUR_OVERLAY_LOG,
        ns::JUR_RECONCILE,
    ] {
        txn.put_aux(namespace, "capture", serde_json::json!(true))
            .unwrap();
    }
    txn.put_aux(ns::SYS_BLOB, &content_key, serde_json::json!("draft"))
        .unwrap();
    txn.commit(meta()).unwrap();

    let mut txn = owner.bookkeeping_writer().begin().unwrap();
    for namespace in [ns::JUR_REPAIR, ns::JUR_OVERLAY, ns::JUR_RECONCILE] {
        txn.put_aux(namespace, "bookkeeping", serde_json::json!(true))
            .unwrap();
    }
    txn.put_aux(ns::SYS_BLOB, "file/notes.md", serde_json::json!("updated"))
        .unwrap();
    txn.put_aux(ns::SYS_BLOB, "query/export", serde_json::json!("result"))
        .unwrap();
    txn.commit(meta()).unwrap();

    let mut txn = owner.host_control_writer().begin().unwrap();
    txn.put_aux(ns::SYS_CLOCK, "offset_ms", serde_json::json!(1))
        .unwrap();
    txn.put_aux(
        ns::SYS_UNAVAILABLE,
        "notes.md",
        serde_json::json!({"mode":"read-only"}),
    )
    .unwrap();
    txn.commit(meta()).unwrap();

    let coordinator_node = NodeId::new();
    let mut txn = owner.coordinator_writer().begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: node(coordinator_node, KindId(99)),
    })
    .unwrap();
    txn.put_aux(
        ns::ILRP_INTENT,
        "repair",
        serde_json::json!({"state":"prepared"}),
    )
    .unwrap();
    txn.commit(meta()).unwrap();

    let source = SourceId::from_name("prices");
    let observed = ContentHash::of(b"42");
    let external = NodeId::new();
    let writer = owner.reactor_writer(source);
    let mut txn = writer.begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: node(external, kind::EXTERNAL_VALUE),
    })
    .unwrap();
    txn.put_aux(
        ns::SYS_BLOB,
        &format!("obs/{source}/{observed}"),
        serde_json::json!(42),
    )
    .unwrap();
    txn.put_aux(
        ns::SYS_BLOB,
        &format!("obs-current/{source}"),
        serde_json::json!({"source":source}),
    )
    .unwrap();
    txn.apply(Operation::MaterializeExternal {
        node: external,
        source,
        observed,
        at: Timestamp(2),
    })
    .unwrap();
    txn.commit(meta()).unwrap();
}

#[test]
fn every_scoped_denial_taints_the_transaction() {
    let dir = liminal_scratch::ScratchDir::new("scoped-writer-denials").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let before = owner.head().unwrap();

    let mut txn = owner.bootstrap_writer().begin().unwrap();
    assert!(
        txn.apply(Operation::DeleteNode { id: NodeId::new() })
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let mut txn = owner.capture_writer().begin().unwrap();
    assert!(
        txn.put_aux(ns::SYS_BLOB, "file/escape", serde_json::json!(1))
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let mut txn = owner.bookkeeping_writer().begin().unwrap();
    assert!(
        txn.put_aux(ns::JUR_DECISION, "escape", serde_json::json!(1))
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let mut txn = owner.host_control_writer().begin().unwrap();
    assert!(
        txn.put_aux(ns::SYS_BLOB, "query/escape", serde_json::json!(1))
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let mut txn = owner.coordinator_writer().begin().unwrap();
    assert!(
        txn.put_aux(ns::JUR_PLAN, "escape", serde_json::json!(1))
            .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let source = SourceId::from_name("prices");
    let wrong_source = SourceId::from_name("weather");
    let mut txn = owner.reactor_writer(source).begin().unwrap();
    assert!(
        txn.apply(Operation::MaterializeExternal {
            node: NodeId::new(),
            source: wrong_source,
            observed: ContentHash::of(b"wrong"),
            at: Timestamp(2),
        })
        .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    let mut txn = owner.reactor_writer(source).begin().unwrap();
    assert!(
        txn.put_aux(
            ns::SYS_BLOB,
            &format!("obs/{source}/not-a-hash"),
            serde_json::json!(1),
        )
        .is_err()
    );
    assert!(txn.commit(meta()).is_err());

    assert_eq!(owner.head().unwrap(), before);
}

#[test]
fn preview_ops_has_no_effects_and_head_guard_refuses_drift() {
    let dir = liminal_scratch::ScratchDir::new("preview-and-head-guard").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    let previewed = NodeId::new();
    let head = owner.head().unwrap();
    let preview = owner
        .preview_ops(&[Operation::CreateNode {
            node: node(previewed, KindId(77)),
        }])
        .unwrap();
    assert!(preview.node(previewed).is_some());
    assert_eq!(preview.head(), head);
    assert!(owner.nodes().unwrap().is_empty());
    assert_eq!(owner.head().unwrap(), head);

    let mut guarded = owner
        .coordinator_writer()
        .begin()
        .unwrap()
        .expect_head(head);
    guarded
        .put_aux(ns::ILRP_INTENT, "guarded", serde_json::json!(true))
        .unwrap();

    let mut intervening = owner.begin().unwrap();
    intervening
        .put_aux(ns::JUR_ALIAS, "intervening", serde_json::json!(true))
        .unwrap();
    intervening.commit(meta()).unwrap();

    assert!(guarded.commit(meta()).is_err());
    assert_eq!(owner.head().unwrap(), GraphRevisionId(head.0 + 1));
    assert_eq!(owner.get_aux(ns::ILRP_INTENT, "guarded").unwrap(), None);
}

#[test]
fn committed_aux_history_retains_overwrites_and_deletes() {
    let dir = liminal_scratch::ScratchDir::new("committed-aux-history").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    for value in [Some(serde_json::json!(1)), Some(serde_json::json!(2)), None] {
        let mut txn = owner.begin().unwrap();
        match value {
            Some(value) => txn.put_aux(ns::JUR_ALIAS, "same", value).unwrap(),
            None => txn.delete_aux(ns::JUR_ALIAS, "same").unwrap(),
        }
        txn.commit(meta()).unwrap();
    }

    let history = owner.committed_aux_history(ns::JUR_ALIAS, "same").unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].revision, GraphRevisionId(1));
    assert_eq!(history[0].value, Some(serde_json::json!(1)));
    assert_eq!(history[1].revision, GraphRevisionId(2));
    assert_eq!(history[1].value, Some(serde_json::json!(2)));
    assert_eq!(history[2].revision, GraphRevisionId(3));
    assert_eq!(history[2].value, None);
    assert_eq!(history[2].transaction.parent, GraphRevisionId(2));
    assert!(owner.get_aux(ns::JUR_ALIAS, "same").unwrap().is_none());
}

#[test]
fn committed_history_ignores_unrelated_working_capture_but_refuses_target_tampering() {
    let dir = liminal_scratch::ScratchDir::new("history-working-capture").unwrap();
    let owner = StoreOwner::open(&dir).unwrap();
    owner
        .working_capture()
        .put_buffer(ClientId::new(), BufferId::new(), 1, "working")
        .unwrap();

    let mut txn = owner.coordinator_writer().begin().unwrap();
    txn.put_aux(
        ns::ILRP_INTENT,
        "repair",
        serde_json::json!({"state":"prepared"}),
    )
    .unwrap();
    txn.commit(meta()).unwrap();
    assert_eq!(
        owner
            .committed_aux_history(ns::ILRP_INTENT, "repair")
            .unwrap()
            .len(),
        1
    );

    let source = SourceId::from_name("tamper-target");
    let hash = ContentHash::of(b"original");
    let external = NodeId::new();
    let mut txn = owner.reactor_writer(source).begin().unwrap();
    txn.apply(Operation::CreateNode {
        node: node(external, kind::EXTERNAL_VALUE),
    })
    .unwrap();
    txn.put_aux(
        ns::SYS_BLOB,
        &format!("obs/{source}/{hash}"),
        serde_json::json!("original"),
    )
    .unwrap();
    txn.apply(Operation::MaterializeExternal {
        node: external,
        source,
        observed: hash,
        at: Timestamp(2),
    })
    .unwrap();
    txn.commit(meta()).unwrap();
    owner
        .blob_fault_injector()
        .corrupt_observation(source, hash, serde_json::json!("tampered"))
        .unwrap();
    assert!(
        owner
            .committed_aux_history(ns::SYS_BLOB, &format!("obs/{source}/{hash}"))
            .is_err()
    );
}
