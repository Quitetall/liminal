//! Public assurance CLI controls (ADR-0022).

use std::process::Command;

fn auth_fixture_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

struct SignedBatchFixture {
    root: std::path::PathBuf,
    trust: std::path::PathBuf,
    base: String,
    target: String,
    source: String,
}

impl SignedBatchFixture {
    fn create() -> Self {
        Self::create_with_packet(false)
    }

    fn create_with_packet(use_real_packet: bool) -> Self {
        let root = fixture();
        let source = "crates/liminal-source/src/view.rs".to_owned();
        let source_path = root.join(&source);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let mut source_lines = vec!["pub fn fixture_logic() {".to_owned()];
        while source_lines.len() < 58 {
            source_lines.push(String::new());
        }
        source_lines.extend(["    &self.basis".to_owned(), "}".to_owned()]);
        std::fs::write(&source_path, format!("{}\n", source_lines.join("\n"))).unwrap();
        let packet_path = root.join("conformance/haqp/packet.json");
        std::fs::create_dir_all(packet_path.parent().unwrap()).unwrap();
        if use_real_packet {
            let packet_template = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../conformance/haqp/packet.json");
            std::fs::copy(packet_template, &packet_path).unwrap();
        } else {
            std::fs::write(
                &packet_path,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "mutants": [{
                        "id": "P1-M008",
                        "source": "crates/liminal-source/src/view.rs:59"
                    }]
                }))
                .unwrap(),
            )
            .unwrap();
        }
        fixture_git(&root, &["init", "--quiet"]);
        fixture_git(&root, &["add", "."]);
        fixture_git(&root, &["commit", "--quiet", "-m", "authorization fixture"]);
        let base = fixture_git(&root, &["rev-parse", "HEAD"]);
        let trust =
            std::env::temp_dir().join(format!("liminal-test-trust-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&trust).unwrap();
        let generated = Command::new("ssh-keygen")
            .args([
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-C",
                "disposable-assurance-test",
                "-f",
            ])
            .arg(trust.join("test-key"))
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "disposable key generation failed"
        );
        let public = std::fs::read_to_string(trust.join("test-key.pub")).unwrap();
        let signers = format!("fixture {public}");
        std::fs::write(trust.join("allowed_signers"), &signers).unwrap();
        let executable = std::fs::read(env!("CARGO_BIN_EXE_liminal-xtask")).unwrap();
        let tool_hash = auth_fixture_hash(&executable);
        let tool_revision = "1".repeat(40); // External fixture attestation, not this binary's Git HEAD.
        let git_dir = root.join(".git").canonicalize().unwrap();
        let policy = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "repository_git_dir": git_dir,
            "change_class": "coordinate-only", "tool_revision": tool_revision,
            "tool_sha256": tool_hash
        }))
        .unwrap();
        let policy_hash = auth_fixture_hash(&policy);
        std::fs::write(trust.join("policy.json"), &policy).unwrap();
        let batch = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "id": "fixture-batch", "policy_sha256": policy_hash,
            "base_commit": base, "change_class": "coordinate-only",
            "tool_revision": tool_revision, "tool_sha256": tool_hash,
            "targets": ["P1-M008"]
        }))
        .unwrap();
        std::fs::write(trust.join("batch.json"), &batch).unwrap();
        std::fs::write(trust.join("trust.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "enabled": true, "principal": "fixture",
            "repository_git_dir": git_dir, "policy_sha256": policy_hash,
            "batch_sha256": auth_fixture_hash(&batch), "tool_revision": tool_revision,
            "tool_sha256": tool_hash, "allowed_signers_sha256": auth_fixture_hash(signers.as_bytes())
        })).unwrap()).unwrap();
        let fixture = Self {
            root,
            trust,
            base,
            target: "P1-M008".to_owned(),
            source,
        };
        fixture.sign("policy.json", "liminal.assurance.policy.v1");
        fixture.sign("batch.json", "liminal.assurance.batch.v1");
        fixture
    }

    fn sign(&self, filename: &str, namespace: &str) {
        let signature = self.trust.join(format!("{filename}.sig"));
        if signature.exists() {
            std::fs::remove_file(&signature).unwrap();
        }
        let output = Command::new("ssh-keygen")
            .args(["-Y", "sign", "-n", namespace, "-f"])
            .arg(self.trust.join("test-key"))
            .arg(self.trust.join(filename))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "disposable signature generation failed"
        );
    }

    fn command(&self) -> Command {
        self.command_with_trust(&self.trust)
    }

    fn command_with_trust(&self, trust: &std::path::Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"));
        command
            .current_dir(&self.root)
            .args([
                "assurance",
                "amend",
                "check",
                "--authorization-only",
                "--trust-root",
            ])
            .arg(trust)
            .arg("--policy")
            .arg(self.trust.join("policy.json"))
            .arg("--policy-signature")
            .arg(self.trust.join("policy.json.sig"))
            .arg("--batch")
            .arg(self.trust.join("batch.json"))
            .arg("--batch-signature")
            .arg(self.trust.join("batch.json.sig"))
            .args(["--base", &self.base, "--target", &self.target]);
        command
    }

    fn candidate_command(&self, candidate: &str) -> Command {
        let mut command = self.command();
        command
            .args(["--candidate", candidate, "--source"])
            .arg(&self.source)
            .args(["--line", "59", "--anchor", "    &self.basis"]);
        command
    }

    fn candidate_command_with_review(&self, candidate: &str, receipt: &std::path::Path) -> Command {
        let mut command = self.candidate_command(candidate);
        command.args(["--review-receipt"]).arg(receipt);
        command
    }

    fn apply_command(
        &self,
        candidate: &str,
        receipt: Option<&std::path::Path>,
        output: &std::path::Path,
    ) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"));
        command
            .current_dir(&self.root)
            .args([
                "assurance",
                "amend",
                "apply",
                "--authorization-only",
                "--trust-root",
            ])
            .arg(&self.trust)
            .args(["--policy"])
            .arg(self.trust.join("policy.json"))
            .args(["--policy-signature"])
            .arg(self.trust.join("policy.json.sig"))
            .args(["--batch"])
            .arg(self.trust.join("batch.json"))
            .args(["--batch-signature"])
            .arg(self.trust.join("batch.json.sig"))
            .args([
                "--base",
                &self.base,
                "--target",
                &self.target,
                "--candidate",
                candidate,
            ])
            .args(["--source"])
            .arg(&self.source)
            .args(["--line", "59", "--anchor", "    &self.basis"]);
        if let Some(receipt) = receipt {
            command.args(["--review-receipt"]).arg(receipt);
        }
        command.args(["--output"]).arg(output);
        command
    }

    fn make_coordinate_candidate(&self) -> String {
        let source_path = self.root.join(&self.source);
        let mut source_lines: Vec<_> = std::fs::read_to_string(&source_path)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        // Packet coordinate mirrors this two-line relocation: 59 becomes 61.
        source_lines.insert(0, String::new());
        source_lines.insert(0, String::new());
        std::fs::write(&source_path, format!("{}\n", source_lines.join("\n"))).unwrap();
        let packet_path = self.root.join("conformance/haqp/packet.json");
        let packet = std::fs::read_to_string(&packet_path).unwrap().replace(
            "crates/liminal-source/src/view.rs:59",
            "crates/liminal-source/src/view.rs:61",
        );
        std::fs::write(packet_path, packet).unwrap();
        fixture_git(&self.root, &["add", "."]);
        fixture_git(&self.root, &["commit", "--quiet", "-m", "coordinate move"]);
        fixture_git(&self.root, &["rev-parse", "HEAD"])
    }

    fn write_review(&self, candidate: &str) -> std::path::PathBuf {
        let output = Command::new("git")
            .current_dir(&self.root)
            .args([
                "diff",
                "--no-ext-diff",
                "--binary",
                "--full-index",
                &self.base,
                candidate,
                "--",
                &self.source,
                "conformance/haqp/packet.json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture patch lookup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let path = self.trust.join("review.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "reviewer": "fixture-reviewer",
                "backend": "lamu",
                "base_commit": self.base,
                "candidate_commit": candidate,
                "target": self.target,
                "patch_sha256": auth_fixture_hash(&output.stdout),
                "verdict": "pass",
                "unresolved_verified_findings": 0,
                "findings": []
            }))
            .unwrap(),
        )
        .unwrap();
        path
    }

    fn rewrite(&self, name: &str, change: impl FnOnce(&mut serde_json::Value)) {
        let path = self.trust.join(name);
        let mut value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        change(&mut value);
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }

    fn repin(&self, filename: &str, field: &str) {
        let hash = auth_fixture_hash(&std::fs::read(self.trust.join(filename)).unwrap());
        self.rewrite("trust.json", |value| value[field] = hash.into());
    }

    fn finish(self) {
        std::fs::remove_dir_all(self.root).unwrap();
        std::fs::remove_dir_all(self.trust).unwrap();
    }
}

struct FixtureWorktreeCleanup {
    root: std::path::PathBuf,
    output: std::path::PathBuf,
}

impl Drop for FixtureWorktreeCleanup {
    fn drop(&mut self) {
        if self.output.exists() {
            let _ = Command::new("git")
                .current_dir(&self.root)
                .args(["worktree", "remove", "--force"])
                .arg(&self.output)
                .output();
        }
    }
}

#[test]
fn signed_batch_authenticates_scope_without_authorizing_apply() {
    let fixture = SignedBatchFixture::create();
    let output = fixture.command().output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["batch_authenticated"], true);
    assert_eq!(value["apply_authorized"], false);
    assert_eq!(value["independent_review"], "not-established");
    assert_eq!(value["batch_id"], "fixture-batch");
    assert_eq!(fixture_git(&fixture.root, &["status", "--porcelain"]), "");
    fixture.finish();
}

fn assert_auth_refusal(mut command: Command, expected: &str) {
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stderr.contains(expected),
        "expected {expected}: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "refusal must not emit authentication success"
    );
}

#[cfg(unix)]
fn assert_auth_success_with_timeout(mut command: Command) {
    use std::time::{Duration, Instant};

    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "large signed batch failed: {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("large signed batch verification timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn signed_batch_rejects_wrong_signature_namespaces() {
    for (file, wrong, expected) in [
        (
            "policy.json",
            "liminal.assurance.batch.v1",
            "liminal.assurance.policy.v1",
        ),
        (
            "batch.json",
            "liminal.assurance.policy.v1",
            "liminal.assurance.batch.v1",
        ),
    ] {
        let fixture = SignedBatchFixture::create();
        fixture.sign(file, wrong);
        assert_auth_refusal(
            fixture.command(),
            &format!("signature verification failed for {expected}"),
        );
        fixture.finish();
    }
}

#[test]
fn signed_batch_verifies_exact_bytes_even_after_external_digest_repin() {
    let fixture = SignedBatchFixture::create();
    let path = fixture.trust.join("batch.json");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b'\n');
    std::fs::write(path, bytes).unwrap();
    fixture.repin("batch.json", "batch_sha256");
    assert_auth_refusal(
        fixture.command(),
        "signature verification failed for liminal.assurance.batch.v1",
    );
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn signed_batch_verifies_payload_larger_than_pipe_buffer() {
    let fixture = SignedBatchFixture::create();
    fixture.rewrite("batch.json", |value| {
        value["id"] = "x".repeat(131_072).into();
    });
    fixture.repin("batch.json", "batch_sha256");
    fixture.sign("batch.json", "liminal.assurance.batch.v1");
    assert_auth_success_with_timeout(fixture.command());
    fixture.finish();
}

#[test]
fn signed_batch_binds_exact_closed_registry_and_packet_move() {
    let fixture = SignedBatchFixture::create();
    let candidate = fixture.make_coordinate_candidate();
    let output = fixture.candidate_command(&candidate).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["registry_binding"], "verified");
    assert_eq!(value["apply_authorized"], false);
    fixture.finish();
}

#[test]
fn signed_batch_binds_independent_review_receipt_to_exact_patch() {
    let fixture = SignedBatchFixture::create();
    let candidate = fixture.make_coordinate_candidate();
    let receipt = fixture.write_review(&candidate);
    let output = fixture
        .candidate_command_with_review(&candidate, &receipt)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["registry_binding"], "verified");
    assert_eq!(value["independent_review"], "receipt-verified");
    assert_eq!(value["apply_authorized"], false);
    assert_eq!(value["review_receipt_sha256"].as_str().unwrap().len(), 64);
    fixture.finish();
}

#[test]
fn signed_batch_apply_uses_fresh_worktree_and_leaves_source_checkout_untouched() {
    let fixture = SignedBatchFixture::create_with_packet(true);
    let candidate = fixture.make_coordinate_candidate();
    let receipt = fixture.write_review(&candidate);
    let output =
        std::env::temp_dir().join(format!("liminal-assurance-apply-{}", uuid::Uuid::now_v7()));
    let cleanup = FixtureWorktreeCleanup {
        root: fixture.root.clone(),
        output: output.clone(),
    };
    let result = fixture
        .apply_command(&candidate, Some(&receipt), &output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["batch_authenticated"], true);
    assert_eq!(value["apply_authorized"], true);
    assert_eq!(value["independent_review"], "receipt-verified");
    assert_eq!(value["committed"], false);
    assert_eq!(value["pushed"], false);
    assert_eq!(value["signed"], false);
    assert_eq!(value["isolated_worktree"], output.to_str().unwrap());
    assert_eq!(value["packet_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        fixture_git(&fixture.root, &["rev-parse", "HEAD"]),
        candidate
    );
    assert_eq!(fixture_git(&fixture.root, &["status", "--porcelain"]), "");
    let candidate_diff = Command::new("git")
        .current_dir(&output)
        .args([
            "diff",
            "--no-ext-diff",
            "--binary",
            "--full-index",
            "--cached",
            &candidate,
            "--",
        ])
        .output()
        .unwrap();
    assert!(candidate_diff.status.success());
    assert!(candidate_diff.stdout.is_empty());
    drop(cleanup);
    fixture.finish();
}

#[test]
fn signed_batch_apply_requires_review_and_fresh_destination() {
    let fixture = SignedBatchFixture::create();
    let candidate = fixture.make_coordinate_candidate();
    let missing_review_output = std::env::temp_dir().join(format!(
        "liminal-assurance-apply-missing-{}",
        uuid::Uuid::now_v7()
    ));
    assert_auth_refusal(
        fixture.apply_command(&candidate, None, &missing_review_output),
        "isolated apply requires a verified review receipt",
    );
    assert!(!missing_review_output.exists());
    let receipt = fixture.write_review(&candidate);
    let existing = std::env::temp_dir().join(format!(
        "liminal-assurance-apply-existing-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir(&existing).unwrap();
    assert_auth_refusal(
        fixture.apply_command(&candidate, Some(&receipt), &existing),
        "isolated apply output already exists",
    );
    std::fs::remove_dir_all(existing).unwrap();
    let inside = fixture.root.join("isolated-apply");
    assert_auth_refusal(
        fixture.apply_command(&candidate, Some(&receipt), &inside),
        "trust or verification scratch is inside repository scope",
    );
    assert!(!inside.exists());
    fixture.finish();
}

#[test]
fn signed_batch_rejects_substituted_review_receipts() {
    for (field, value, expected) in [
        (
            "patch_sha256",
            serde_json::json!("0".repeat(64)),
            "review receipt patch digest mismatch",
        ),
        (
            "candidate_commit",
            serde_json::json!("0".repeat(40)),
            "review receipt scope mismatch",
        ),
        (
            "verdict",
            serde_json::json!("fail"),
            "review receipt is not a pass",
        ),
        (
            "unresolved_verified_findings",
            serde_json::json!(1),
            "review receipt has unresolved verified findings",
        ),
        (
            "extra",
            serde_json::json!(true),
            "invalid external review receipt",
        ),
    ] {
        let fixture = SignedBatchFixture::create();
        let candidate = fixture.make_coordinate_candidate();
        let receipt = fixture.write_review(&candidate);
        fixture.rewrite("review.json", |review| review[field] = value);
        assert_auth_refusal(
            fixture.candidate_command_with_review(&candidate, &receipt),
            expected,
        );
        fixture.finish();
    }
}

#[test]
fn signed_batch_rejects_unresolved_or_unreproduced_review_findings() {
    for (finding, expected) in [
        (
            serde_json::json!({
                "id": "R-1",
                "classification": "caught_violation",
                "independently_reproduced": false,
                "resolved": false
            }),
            "review receipt has unresolved finding",
        ),
        (
            serde_json::json!({
                "id": "R-1",
                "classification": "verified_defect",
                "independently_reproduced": false,
                "resolved": true
            }),
            "verified review finding lacks independent reproduction",
        ),
    ] {
        let fixture = SignedBatchFixture::create();
        let candidate = fixture.make_coordinate_candidate();
        let receipt = fixture.write_review(&candidate);
        fixture.rewrite("review.json", |review| {
            review["findings"] = vec![finding].into();
        });
        assert_auth_refusal(
            fixture.candidate_command_with_review(&candidate, &receipt),
            expected,
        );
        fixture.finish();
    }
}

#[test]
fn signed_batch_rejects_review_receipt_outside_trust_root() {
    let fixture = SignedBatchFixture::create();
    let candidate = fixture.make_coordinate_candidate();
    let receipt = fixture.write_review(&candidate);
    let outside = fixture
        .trust
        .parent()
        .unwrap()
        .join(format!("liminal-review-outside-{}", uuid::Uuid::now_v7()));
    std::fs::copy(&receipt, &outside).unwrap();
    assert_auth_refusal(
        fixture.candidate_command_with_review(&candidate, &outside),
        "review receipt must be under external trust root",
    );
    std::fs::remove_file(outside).unwrap();
    fixture.finish();
}

#[test]
fn signed_batch_rejects_review_receipt_under_trust_root_prefix_sibling() {
    let fixture = SignedBatchFixture::create();
    let candidate = fixture.make_coordinate_candidate();
    let receipt = fixture.write_review(&candidate);
    let sibling = std::path::PathBuf::from(format!("{}-sibling", fixture.trust.to_string_lossy()));
    std::fs::create_dir(&sibling).unwrap();
    let outside = sibling.join("review.json");
    std::fs::copy(&receipt, &outside).unwrap();
    assert_auth_refusal(
        fixture.candidate_command_with_review(&candidate, &outside),
        "review receipt must be under external trust root",
    );
    std::fs::remove_dir_all(sibling).unwrap();
    fixture.finish();
}

#[test]
fn signed_batch_rejects_extra_packet_or_tree_changes() {
    for extra in ["packet", "tree"] {
        let fixture = SignedBatchFixture::create();
        let source_path = fixture.root.join(&fixture.source);
        let mut source_lines: Vec<_> = std::fs::read_to_string(&source_path)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        source_lines.insert(0, String::new());
        source_lines.insert(0, String::new());
        std::fs::write(&source_path, format!("{}\n", source_lines.join("\n"))).unwrap();
        let packet_path = fixture.root.join("conformance/haqp/packet.json");
        let mut packet = std::fs::read_to_string(&packet_path).unwrap().replace(
            "crates/liminal-source/src/view.rs:59",
            "crates/liminal-source/src/view.rs:61",
        );
        if extra == "packet" {
            packet = packet.replacen("{\n", "{\n  \"extra\": true,\n", 1);
            std::fs::write(&packet_path, packet).unwrap();
        } else {
            std::fs::write(&packet_path, packet).unwrap();
            std::fs::write(fixture.root.join("unrelated.txt"), "unrelated\n").unwrap();
        }
        fixture_git(&fixture.root, &["add", "."]);
        fixture_git(
            &fixture.root,
            &["commit", "--quiet", "-m", "invalid coordinate move"],
        );
        let candidate = fixture_git(&fixture.root, &["rev-parse", "HEAD"]);
        assert_auth_refusal(
            fixture.candidate_command(&candidate),
            if extra == "packet" {
                "candidate packet diff is not one exact source-coordinate replacement"
            } else {
                "candidate diff contains non-modification status"
            },
        );
        fixture.finish();
    }
}

#[test]
fn signed_batch_rejects_partial_registry_binding_group() {
    let fixture = SignedBatchFixture::create();
    let mut command = fixture.command();
    command.args(["--candidate", &fixture.base]);
    assert_auth_refusal(
        command,
        "candidate, source, line and anchor must be supplied together",
    );
    fixture.finish();
}

#[test]
fn signed_batch_rejects_review_receipt_without_registry_binding() {
    let fixture = SignedBatchFixture::create();
    let mut command = fixture.command();
    command
        .args(["--review-receipt"])
        .arg(fixture.trust.join("missing-review.json"));
    assert_auth_refusal(
        command,
        "review receipt requires complete registry binding inputs",
    );
    fixture.finish();
}

#[test]
fn signed_batch_rejects_source_outside_closed_registry() {
    let fixture = SignedBatchFixture::create();
    let mut command = fixture.command();
    command
        .args(["--candidate", &fixture.base, "--source"])
        .arg("src/lib.rs")
        .args(["--line", "1", "--anchor", "pub fn example() {}"]);
    assert_auth_refusal(command, "source path does not match closed registry");
    fixture.finish();
}

#[test]
fn signed_batch_refuses_revoked_pins_wrong_tool_and_wrong_scope() {
    for (field, value, expected) in [
        ("enabled", serde_json::json!(false), "trust disabled"),
        (
            "batch_sha256",
            serde_json::json!("0".repeat(64)),
            "inactive batch digest",
        ),
        (
            "policy_sha256",
            serde_json::json!("0".repeat(64)),
            "inactive policy digest",
        ),
        (
            "tool_sha256",
            serde_json::json!("0".repeat(64)),
            "maintenance executable pin mismatch",
        ),
        (
            "tool_revision",
            serde_json::json!("2".repeat(40)),
            "signed tool pin mismatch",
        ),
        (
            "repository_git_dir",
            serde_json::json!("unrelated-repository"),
            "repository scope mismatch",
        ),
        (
            "allowed_signers_sha256",
            serde_json::json!("0".repeat(64)),
            "signer pin mismatch",
        ),
    ] {
        let fixture = SignedBatchFixture::create();
        fixture.rewrite("trust.json", |v| v[field] = value);
        assert_auth_refusal(fixture.command(), expected);
        fixture.finish();
    }
}

#[test]
fn signed_batch_refuses_out_of_scope_base_and_target() {
    let mut fixture = SignedBatchFixture::create();
    let base = fixture.base.clone();
    fixture.base = "0".repeat(40);
    assert_auth_refusal(fixture.command(), "batch base mismatch");
    fixture.base = base;
    fixture.target = "P1-M002".to_owned();
    assert_auth_refusal(fixture.command(), "target outside authorized batch");
    fixture.finish();
}

#[test]
fn signed_batch_refuses_semantic_scope_unknown_fields_and_duplicate_targets() {
    for (field, value, expected) in [
        (
            "change_class",
            serde_json::json!("semantic"),
            "non-coordinate authority refused",
        ),
        (
            "targets",
            serde_json::json!(["P1-M008", "P1-M008"]),
            "invalid or duplicate batch targets",
        ),
        (
            "targets",
            serde_json::json!(["*"]),
            "invalid or duplicate batch targets",
        ),
        ("reviewed", serde_json::json!(true), "invalid signed batch"),
    ] {
        let fixture = SignedBatchFixture::create();
        fixture.rewrite("batch.json", |v| v[field] = value);
        fixture.repin("batch.json", "batch_sha256");
        fixture.sign("batch.json", "liminal.assurance.batch.v1");
        assert_auth_refusal(fixture.command(), expected);
        fixture.finish();
    }
}

#[test]
fn signed_batch_refuses_trust_inside_any_worktree() {
    let fixture = SignedBatchFixture::create();
    let inside = fixture.root.join("candidate-trust");
    std::fs::create_dir(&inside).unwrap();
    assert_auth_refusal(
        fixture.command_with_trust(&inside),
        "inside repository scope",
    );
    let linked =
        std::env::temp_dir().join(format!("liminal-linked-trust-{}", uuid::Uuid::now_v7()));
    fixture_git(
        &fixture.root,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            &fixture.base,
        ],
    );
    assert_auth_refusal(
        fixture.command_with_trust(&linked),
        "inside repository scope",
    );
    fixture_git(
        &fixture.root,
        &["worktree", "remove", "--force", linked.to_str().unwrap()],
    );
    fixture.finish();
}

#[test]
fn signed_batch_refuses_a_signature_from_an_unenrolled_key() {
    let fixture = SignedBatchFixture::create();
    let foreign = fixture.trust.join("unenrolled-test-key");
    let generated = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&foreign)
        .output()
        .unwrap();
    assert!(generated.status.success());
    std::fs::remove_file(fixture.trust.join("batch.json.sig")).unwrap();
    let signed = Command::new("ssh-keygen")
        .args(["-Y", "sign", "-n", "liminal.assurance.batch.v1", "-f"])
        .arg(foreign)
        .arg(fixture.trust.join("batch.json"))
        .output()
        .unwrap();
    assert!(signed.status.success());
    assert_auth_refusal(
        fixture.command(),
        "signature verification failed for liminal.assurance.batch.v1",
    );
    fixture.finish();
}

#[test]
fn signed_batch_refuses_an_unenrolled_principal_and_duplicate_trust_fields() {
    let fixture = SignedBatchFixture::create();
    fixture.rewrite("trust.json", |v| v["principal"] = "unenrolled".into());
    assert_auth_refusal(
        fixture.command(),
        "signature verification failed for liminal.assurance.policy.v1",
    );
    let path = fixture.trust.join("trust.json");
    let original = std::fs::read_to_string(&path).unwrap();
    let duplicate = original.replacen('{', "{\"enabled\":true,", 1);
    std::fs::write(path, duplicate).unwrap();
    assert_auth_refusal(fixture.command(), "duplicate field");
    fixture.finish();
}

#[test]
fn signed_batch_check_without_partial_mode_cannot_claim_full_admission() {
    let fixture = SignedBatchFixture::create();
    let partial = fixture.command();
    let mut full = Command::new(env!("CARGO_BIN_EXE_liminal-xtask"));
    full.current_dir(&fixture.root)
        .args(partial.get_args().filter(|a| *a != "--authorization-only"));
    assert_auth_refusal(full, "full amendment admission unavailable");
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn signed_batch_preserves_trailing_space_in_repository_identity() {
    let fixture = SignedBatchFixture::create();
    let metadata = fixture.root.join("metadata ");
    fixture_git(
        &fixture.root,
        &["init", "--separate-git-dir", metadata.to_str().unwrap()],
    );
    assert_eq!(
        fixture_git(&fixture.root, &["cat-file", "-t", &fixture.base]),
        "commit"
    );
    let canonical = metadata.canonicalize().unwrap();
    fixture.rewrite("policy.json", |v| {
        v["repository_git_dir"] = serde_json::json!(canonical);
    });
    fixture.repin("policy.json", "policy_sha256");
    let policy_hash = auth_fixture_hash(&std::fs::read(fixture.trust.join("policy.json")).unwrap());
    fixture.rewrite("batch.json", |v| v["policy_sha256"] = policy_hash.into());
    fixture.repin("batch.json", "batch_sha256");
    fixture.rewrite("trust.json", |v| {
        v["repository_git_dir"] = serde_json::json!(canonical);
    });
    fixture.sign("policy.json", "liminal.assurance.policy.v1");
    fixture.sign("batch.json", "liminal.assurance.batch.v1");
    let output = fixture.command().output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.finish();
}

fn fixture_git(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(["-c", "init.templateDir="])
        .arg("-c")
        .arg(format!(
            "core.hooksPath={}",
            root.join("disabled-fixture-hooks").display()
        ))
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
fn coordinate_proposal_refuses_blank_change_inside_raw_string_outside_target() {
    let root = fixture();
    fixture_git(&root, &["init", "--quiet"]);
    let old = concat!(
        "const DATA: &str = r#\"first\n",
        "\n",
        "last\"#;\n\n",
        "pub fn example() -> bool {\n    false\n}\n",
    );
    std::fs::write(root.join("src/lib.rs"), old).unwrap();
    fixture_git(&root, &["add", "."]);
    fixture_git(&root, &["commit", "--quiet", "-m", "base"]);
    let base = fixture_git(&root, &["rev-parse", "HEAD"]);
    let new = concat!(
        "const DATA: &str = r#\"first\n",
        "\n",
        "\n",
        "last\"#;\n\n",
        "pub fn example() -> bool {\n    false\n}\n",
    );
    std::fs::write(root.join("src/lib.rs"), new).unwrap();
    fixture_git(&root, &["add", "src/lib.rs"]);
    fixture_git(&root, &["commit", "--quiet", "-m", "raw-data-change"]);
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
            "6",
            "--anchor",
            "    false",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stderr.contains("source token stream changed"),
        "raw-string data change must refuse: {stderr}"
    );
    assert!(output.stdout.is_empty());
    std::fs::remove_dir_all(root).unwrap();
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
