//! AM-17.12 trusted assembly must not attach another workspace's root owner.

use liminal_daemon::ToyWorkspace;
use liminal_graph::StoreOwner;

#[test]
fn workspace_rejects_another_roots_store_owner() {
    let first = liminal_scratch::ScratchDir::new("workspace-owner-first").unwrap();
    let second = liminal_scratch::ScratchDir::new("workspace-owner-second").unwrap();
    let owner = StoreOwner::open(&first.join("state")).unwrap();
    let result = ToyWorkspace::open_with_owner(&second, owner);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("another workspace")
    );
    assert!(!second.join("state").exists());
    let owner = StoreOwner::open(&first.join("state")).unwrap();
    assert_eq!(
        owner.store().head().unwrap(),
        liminal_id::GraphRevisionId(0)
    );
}
