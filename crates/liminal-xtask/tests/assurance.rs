//! Public assurance CLI controls (ADR-0022).

use std::process::Command;

fn fixture() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("liminal-assurance-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(root.join("docs/execution")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("verification/assurance")).unwrap();
    std::fs::write(root.join("docs/execution/00-protocol.md"), "fixture").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn example() {}\n").unwrap();
    std::fs::write(root.join("authority.md"), "# AUTH-1\n").unwrap();
    std::fs::write(root.join("verification/assurance/catalog.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1,
        "profiles": {
            "development": ["style", "assurance"],
            "merge": ["style", "lint", "bootstrap", "proof-support", "nextest", "threaded", "doctest", "docs", "deny", "inventory", "canaries", "resource-run", "resource-lint", "assurance"],
            "portability": ["nextest", "doctest"],
            "scheduled": ["advisories", "beta"],
            "qualification": ["haqp"]
        },
        "families": [{
            "id": "fixture", "owner": "maintainer", "source_areas": ["src"],
            "targets": ["Cargo.toml::lib::fixture"],
            "references": [{"path":"authority.md", "anchor":"AUTH-1"}],
            "review_triggers": ["interface", "phase-gate"],
            "profiles": ["merge", "portability"],
            "platforms": ["linux", "macos", "windows"]
        }]
    })).unwrap()).unwrap();
    root
}

#[test]
fn catalog_check_accepts_registered_target_without_claiming_qualification() {
    let root = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("qualification: not-established"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_python_target_requires_classification() {
    let root = fixture();
    std::fs::create_dir_all(root.join("verification/proof")).unwrap();
    std::fs::write(
        root.join("verification/proof/test_extra.py"),
        "# new test target\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "new Python test target was silently unclassified"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("unclassified target"));
    std::fs::remove_dir_all(root).unwrap();
}

fn reject_catalog_change(change: impl FnOnce(&mut serde_json::Value), expected: &str) {
    let root = fixture();
    let path = root.join("verification/assurance/catalog.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    change(&mut catalog);
    std::fs::write(path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stderr.contains(expected),
        "{stderr}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn omitted_canaries_refused_even_when_catalog_is_internally_consistent() {
    reject_catalog_change(
        |c| {
            c["profiles"]["merge"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v != "canaries");
        },
        "mandatory command sequence drift",
    );
}

#[test]
fn recursive_or_unknown_commands_refused() {
    reject_catalog_change(
        |c| c["profiles"]["development"][0] = "merge".into(),
        "mandatory command sequence drift",
    );
}

#[test]
fn target_cannot_have_two_owners() {
    reject_catalog_change(
        |c| {
            let mut duplicate = c["families"][0].clone();
            duplicate["id"] = "other".into();
            c["families"].as_array_mut().unwrap().push(duplicate);
        },
        "duplicate target",
    );
}

#[test]
fn missing_authority_anchor_refused() {
    reject_catalog_change(
        |c| c["families"][0]["references"][0]["anchor"] = "AUTH-10".into(),
        "missing authority anchor",
    );
}

#[test]
fn source_area_must_cover_target() {
    reject_catalog_change(
        |c| c["families"][0]["source_areas"] = serde_json::json!(["docs"]),
        "target outside family source areas",
    );
}

#[test]
fn stale_target_refused() {
    reject_catalog_change(
        |c| {
            c["families"][0]["targets"]
                .as_array_mut()
                .unwrap()
                .push("Cargo.toml::test::missing".into());
        },
        "stale catalog targets",
    );
}

#[test]
fn duplicate_profile_keys_refused_even_with_identical_values() {
    let root = fixture();
    let path = root.join("verification/assurance/catalog.json");
    let text = std::fs::read_to_string(&path).unwrap().replace(
        "\"profiles\": {",
        "\"profiles\": {\"development\":[\"style\",\"assurance\"],",
    );
    std::fs::write(path, text).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "duplicate profile key accepted");
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate"));
    std::fs::remove_dir_all(root).unwrap();
}
