//! Public assurance CLI controls (ADR-0022).

use std::process::Command;

fn fixture_git(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "-c",
            "user.name=Assurance Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn coordinate_proposal_binds_committed_versions_without_authorizing_apply() {
    let root = fixture();
    fixture_git(&root, &["init", "--quiet"]);
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn example() -> bool {\n    false\n}\n",
    )
    .unwrap();
    fixture_git(&root, &["add", "."]);
    fixture_git(&root, &["commit", "--quiet", "-m", "base"]);
    let base = fixture_git(&root, &["rev-parse", "HEAD"]);
    std::fs::write(
        root.join("src/lib.rs"),
        "\n\npub fn example() -> bool {\n    false\n}\n",
    )
    .unwrap();
    fixture_git(&root, &["add", "src/lib.rs"]);
    fixture_git(&root, &["commit", "--quiet", "-m", "move"]);
    let candidate = fixture_git(&root, &["rev-parse", "HEAD"]);
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args([
            "assurance",
            "amend",
            "propose",
            "--base",
            &base,
            "--candidate",
            &candidate,
            "--source",
            "src/lib.rs",
            "--line",
            "2",
            "--anchor",
            "    false",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let proposal: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(proposal["base_commit"], base);
    assert_eq!(proposal["candidate_commit"], candidate);
    assert_eq!(proposal["old_line"], 2);
    assert_eq!(proposal["new_line"], 4);
    assert_eq!(proposal["authority"], "none");
    assert_eq!(proposal["status"], "proposal-only");
    assert_eq!(fixture_git(&root, &["status", "--porcelain"]), "");
    std::fs::remove_dir_all(root).unwrap();
}

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
    let generated = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "generate-workflows"])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    root
}

#[test]
fn coordinate_proposal_refuses_git_repository_redirection() {
    let root = fixture();
    let foreign = fixture();
    fixture_git(&root, &["init", "--quiet"]);
    fixture_git(&foreign, &["init", "--quiet"]);
    std::fs::write(
        foreign.join("src/lib.rs"),
        "pub fn example() -> bool {\n    false\n}\n",
    )
    .unwrap();
    fixture_git(&foreign, &["add", "."]);
    fixture_git(&foreign, &["commit", "--quiet", "-m", "foreign"]);
    let revision = fixture_git(&foreign, &["rev-parse", "HEAD"]);
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .env("GIT_DIR", foreign.join(".git"))
        .args([
            "assurance",
            "amend",
            "propose",
            "--base",
            &revision,
            "--candidate",
            &revision,
            "--source",
            "src/lib.rs",
            "--line",
            "2",
            "--anchor",
            "    false",
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "foreign repository silently supplied source: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("committed source lookup failed"));
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(foreign).unwrap();
}

#[test]
fn coordinate_proposal_refuses_changed_ambiguous_and_nonexpression_targets() {
    for (old, new, anchor, expected) in [
        (
            "pub fn example() -> bool {\n    false\n}\n",
            "pub fn renamed() -> bool {\n    false\n}\n",
            "    false",
            "enclosing implementation changed",
        ),
        (
            "pub fn example() -> bool {\n    false\n}\n",
            "pub fn example() -> bool {\n    false\n}\npub fn other() -> bool {\n    false\n}\n",
            "    false",
            "ambiguous candidate target",
        ),
        (
            "pub fn example() -> bool {\n    // false\n    true\n}\n",
            "\npub fn example() -> bool {\n    // false\n    true\n}\n",
            "    // false",
            "whole-line expression",
        ),
        (
            "pub fn example() -> bool {\n    false\n}\n",
            "pub fn example() -> bool {\n    true\n}\n",
            "    false",
            "missing or ambiguous candidate target",
        ),
        (
            "pub fn example() -> bool {\n    false\n}\n",
            "\nnot valid rust",
            "    false",
            "missing or ambiguous candidate target",
        ),
    ] {
        let root = fixture();
        fixture_git(&root, &["init", "--quiet"]);
        std::fs::write(root.join("src/lib.rs"), old).unwrap();
        fixture_git(&root, &["add", "."]);
        fixture_git(&root, &["commit", "--quiet", "-m", "base"]);
        let base = fixture_git(&root, &["rev-parse", "HEAD"]);
        std::fs::write(root.join("src/lib.rs"), new).unwrap();
        fixture_git(&root, &["add", "src/lib.rs"]);
        fixture_git(&root, &["commit", "--quiet", "-m", "candidate"]);
        let candidate = fixture_git(&root, &["rev-parse", "HEAD"]);
        let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
            .current_dir(&root)
            .args([
                "assurance",
                "amend",
                "propose",
                "--base",
                &base,
                "--candidate",
                &candidate,
                "--source",
                "src/lib.rs",
                "--line",
                "2",
                "--anchor",
                anchor,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success() && stderr.contains(expected),
            "expected {expected}: {stderr}"
        );
        assert!(output.stdout.is_empty());
        assert_eq!(fixture_git(&root, &["status", "--porcelain"]), "");
        std::fs::remove_dir_all(root).unwrap();
    }
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

#[test]
fn profile_failure_leaves_remaining_commands_unexecuted() {
    let root = fixture();
    // No justfile: the real style subprocess must fail, not be retried or
    // interpreted as evidence that later classification ran.
    let output_dir = root.with_extension("receipt");
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "run", "development", "--output"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output_dir.join("receipt.json")).expect("failure receipt retained"),
    )
    .unwrap();
    assert_eq!(receipt["state"], "failed");
    assert_eq!(receipt["commands"][1]["state"], "unexecuted");
    assert_eq!(receipt["qualification_established"], false);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(output_dir).unwrap();
}

#[cfg(unix)]
#[test]
fn python_discovery_refuses_symlinked_directories() {
    let root = fixture();
    std::fs::create_dir_all(root.join("verification/proof")).unwrap();
    std::os::unix::fs::symlink(root.join("src"), root.join("verification/proof/linked")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "symlink directory silently omitted"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn completed_profile_is_reportable_but_incomplete_receipt_cannot_claim_pass() {
    let root = fixture();
    std::fs::write(
        root.join("justfile"),
        "fmt-check:\n    @echo fixture-style-control\n",
    )
    .unwrap();
    let output_dir = root.with_extension("receipt");
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "run", "development", "--output"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = output_dir.join("receipt.json");
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .args(["assurance", "report"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["state"], "passed");
    assert_eq!(receipt["qualification_established"], false);
    receipt["commands"][1]["state"] = "unexecuted".into();
    std::fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .args(["assurance", "report"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("incomplete receipt claims pass"));
    let prior = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "run", "development", "--output"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read(path).unwrap(),
        prior,
        "existing output overwritten"
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn disabling_cargo_test_does_not_hide_unclassified_code() {
    let root = fixture();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    let manifest = root.join("Cargo.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("\n[[bin]]\nname = \"untested\"\npath = \"src/main.rs\"\ntest = false\n");
    std::fs::write(manifest, text).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "test=false hid an unclassified target"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("unclassified target"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workflow_drift_is_detected_without_repair() {
    let root = fixture();
    let workflow = root.join(".github/workflows/ci.yml");
    let changed = std::fs::read_to_string(&workflow)
        .unwrap()
        .replace("'haq-canaries'", "'haq-inventory'");
    std::fs::write(&workflow, &changed).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .args(["assurance", "check"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "omitted CI canaries accepted");
    assert!(String::from_utf8_lossy(&output.stderr).contains("workflow drift"));
    assert_eq!(std::fs::read_to_string(workflow).unwrap(), changed);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn merge_requires_independent_checkout_targets_before_execution() {
    let root = fixture();
    let output_dir = root.with_extension("receipt");
    let output = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(["assurance", "run", "merge", "--output"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unset CARGO_TARGET_DIR"));
    assert!(
        !output_dir.exists(),
        "execution started with incompatible global target override"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn profile_survives_atomic_replacement_of_running_tool() {
    let root = fixture();
    std::fs::create_dir(root.join("tools")).unwrap();
    let executable = root.join("tools/xtask");
    std::fs::copy(env!("CARGO_BIN_EXE_liminal-xtask"), &executable).unwrap();
    std::fs::write(root.join("justfile"), "fmt-check:\n    @cp tools/xtask tools/replacement\n    @mv -f tools/replacement tools/xtask\n").unwrap();
    let output_dir = root.with_extension("receipt");
    let output = Command::new(executable)
        .current_dir(&root)
        .args(["assurance", "run", "development", "--output"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output_dir.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["commands"][1]["state"], "passed");
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(output_dir).unwrap();
}
