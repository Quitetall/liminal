//! CLI-level regressions for the HAQP unresolved-review ruling gate.

use std::{fs, process::Command};

use camino::Utf8PathBuf;
use liminal_scratch::ScratchDir;

const RULING_PATH: &str = "docs/execution/rulings/R-001.md";
const TARGET: &str = "crates/liminal-xtask/src/haq.rs";

fn fixture(label: &str, target: &str) -> (ScratchDir, Utf8PathBuf) {
    let scratch = ScratchDir::new(label).expect("scratch dir");
    let root = scratch.path().to_owned();
    fs::create_dir_all(root.join("docs/execution/rulings")).expect("rulings dir");
    fs::create_dir_all(root.join("conformance/haqp")).expect("haqp dir");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("workspace marker");
    fs::write(root.join("docs/execution/00-protocol.md"), "fixture\n").expect("protocol marker");
    fs::write(
        root.join("conformance/haqp/ruling-registry.json"),
        format!(
            "{{\"schema_version\":1,\"rulings\":[{{\"id\":\"R-001\",\"file\":\"{RULING_PATH}\",\"target\":\"{target}\"}}]}}\n"
        ),
    )
    .expect("registry");
    fs::write(
        root.join(RULING_PATH),
        format!(
            "---\nid: R-001\nattack_class: corpus leakage\ntarget: {target}\nclaim_requires: read access\nclaim_excludes: chdir\nstatus: ruled\nruled_by: test\ndate: 2026-09-30\n---\n\nFixture ruling.\n"
        ),
    )
    .expect("ruling");
    (scratch, root)
}

fn signed_commit(root: &Utf8PathBuf) {
    let key = root.join("test-signing-key");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&key)
        .output()
        .expect("ssh-keygen available");
    assert!(generated.status.success(), "ssh-keygen failed");
    let public_key = fs::read_to_string(format!("{key}.pub")).expect("public test key");
    let mut public_fields = public_key.split_whitespace();
    let key_type = public_fields.next().expect("key type");
    let key_data = public_fields.next().expect("key data");
    fs::write(
        root.join("conformance/haqp/ruling-signers"),
        format!("test@example.invalid {key_type} {key_data}\n"),
    )
    .expect("allowed test signer");
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "fixture signer"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["add", "-A"]);
    let signed = Command::new("git")
        .current_dir(root)
        .args(["-c", "gpg.format=ssh", "-c"])
        .arg(format!("user.signingkey={key}"))
        .args(["commit", "--quiet", "-S", "-m", "fixture ruling"])
        .output()
        .expect("git commit");
    assert!(signed.status.success(), "fixture signing commit failed");
}

fn git(root: &Utf8PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git available");
    assert!(output.status.success(), "git {args:?} failed");
}

fn run_review_unresolved(root: &Utf8PathBuf, record: &Utf8PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
        .current_dir(root)
        .args(["haq", "review-unresolved"])
        .arg(record)
        .output()
        .expect("run haq review-unresolved")
}

fn record(root: &Utf8PathBuf, target: &str) -> Utf8PathBuf {
    let path = root.join("attempts.json");
    fs::write(
        &path,
        format!(
            r#"{{"attempts":[{{"id":"A1","attack_class":"corpus leakage","target":"{target}","attempt":"review read access","observed_result":"unauthorized read access remains accepted","classification":"verified_defect","independently_reproduced":true,"resolved":false}}],"findings":[],"independently_reproduced":[]}}"#
        ),
    )
    .expect("review record");
    path
}

#[test]
fn cli_clears_only_the_exact_signed_coordinate() {
    let (_scratch, root) = fixture("review-exact-coordinate", &format!("{TARGET}:10"));
    signed_commit(&root);

    let exact = record(&root, &format!("{TARGET}:10"));
    let output = run_review_unresolved(&root, &exact);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");

    let neighboring = record(&root, &format!("{TARGET}:11"));
    let output = run_review_unresolved(&root, &neighboring);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
}

#[test]
fn cli_rejects_file_scoped_rulings() {
    let (_scratch, root) = fixture("review-file-scope", TARGET);
    signed_commit(&root);
    let attempt = record(&root, &format!("{TARGET}:10"));
    let output = run_review_unresolved(&root, &attempt);
    assert!(!output.status.success(), "file-only target was accepted");
}

#[test]
fn cli_rejects_rulings_outside_the_closed_registry() {
    let (_scratch, root) = fixture("review-closed-registry", &format!("{TARGET}:10"));
    signed_commit(&root);
    fs::write(
        root.join("docs/execution/rulings/R-999-extra.md"),
        "---\nid: R-999\nattack_class: corpus leakage\ntarget: src/lib.rs:1\nclaim_requires: read access\nstatus: draft\n---\n\nExtra ruling.\n",
    )
    .expect("unregistered ruling");
    let attempt = record(&root, &format!("{TARGET}:11"));
    let output = run_review_unresolved(&root, &attempt);
    assert!(!output.status.success(), "unregistered ruling was accepted");
}
