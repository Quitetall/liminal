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

#[cfg(unix)]
#[test]
fn workspace_rejects_owner_after_store_directory_replacement() {
    let root = liminal_scratch::ScratchDir::new("workspace-owner-replaced").unwrap();
    let path = root.join("state");
    let original = StoreOwner::open(&path).unwrap();
    std::fs::rename(&path, root.join("previous-state")).unwrap();
    let replacement = StoreOwner::open(&path).unwrap();
    assert!(
        ToyWorkspace::open_with_owner(&root, original).is_err(),
        "equal path text must not attach the old store to its replacement"
    );
    assert_eq!(
        replacement.store().head().unwrap(),
        liminal_id::GraphRevisionId(0)
    );
}

#[cfg(unix)]
#[test]
fn missing_named_lock_does_not_match_the_owner() {
    let root = liminal_scratch::ScratchDir::new("workspace-owner-missing-lock").unwrap();
    let path = root.join("state");
    let owner = StoreOwner::open(&path).unwrap();
    std::fs::rename(&path, root.join("previous-state")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(!owner.matches_directory(&path).unwrap());
}
