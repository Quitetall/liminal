//! Human-signed batch authentication, not patch admission or an apply capability.

use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Explicit CLI inputs for the authorization-only check. No signing operation.
#[derive(Debug, clap::Args)]
pub struct AuthorizationRequest {
    /// Required until full patch/review admission is implemented.
    #[arg(long)]
    authorization_only: bool,
    /// Human-managed absolute directory outside every repository worktree.
    #[arg(long)]
    trust_root: Utf8PathBuf,
    /// Exact human-signed standing policy bytes.
    #[arg(long)]
    policy: Utf8PathBuf,
    /// Detached OpenSSH standing-policy signature.
    #[arg(long)]
    policy_signature: Utf8PathBuf,
    /// Exact separately human-signed batch bytes.
    #[arg(long)]
    batch: Utf8PathBuf,
    /// Detached OpenSSH batch signature.
    #[arg(long)]
    batch_signature: Utf8PathBuf,
    /// Exact requested base commit, not a branch or mutable revision name.
    #[arg(long)]
    base: String,
    /// Exact requested mutant identifier; wildcards are not supported.
    #[arg(long)]
    target: String,
}

// Strict shapes: omitted, duplicated and unknown fields are not authority.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trust {
    schema_version: u32,
    enabled: bool,
    principal: String,
    repository_git_dir: String,
    policy_sha256: String,
    batch_sha256: String,
    tool_revision: String,
    tool_sha256: String,
    allowed_signers_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u32,
    repository_git_dir: String,
    change_class: String,
    tool_revision: String,
    tool_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    schema_version: u32,
    id: String,
    policy_sha256: String,
    base_commit: String,
    change_class: String,
    tool_revision: String,
    tool_sha256: String,
    targets: Vec<String>,
}

/// Authenticate the selected human-authorized scope, without inspecting or
/// accepting a patch. JSON output is informational, never a reusable capability.
pub fn check_authorization(
    root: &Utf8Path,
    request: &AuthorizationRequest,
) -> Result<serde_json::Value> {
    ensure!(
        request.authorization_only,
        "full amendment admission unavailable; --authorization-only required"
    );
    let repository = auth_repository_paths(root)?;
    ensure!(
        request.trust_root.is_absolute(),
        "absolute external trust root required"
    );
    let trust_root = request
        .trust_root
        .canonicalize_utf8()
        .context("external trust root unavailable")?;
    auth_require_external(&trust_root, &repository)?;
    let trust: Trust = serde_json::from_slice(&auth_read(&trust_root.join("trust.json"))?)
        .context("invalid trust configuration")?;
    ensure!(
        trust.schema_version == 1 && trust.enabled,
        "trust disabled or unsupported"
    );
    ensure!(
        auth_identifier(&trust.principal),
        "invalid signer principal"
    );
    ensure!(
        trust.repository_git_dir == repository[0].as_str(),
        "repository scope mismatch"
    );
    ensure!(
        auth_hex(&trust.tool_revision, 40),
        "invalid tool revision pin"
    );
    for digest in [
        &trust.policy_sha256,
        &trust.batch_sha256,
        &trust.tool_sha256,
        &trust.allowed_signers_sha256,
    ] {
        ensure!(auth_hex(digest, 64), "invalid external digest pin");
    }
    auth_check_executable(&trust.tool_sha256)?;
    let signers = auth_read(&trust_root.join("allowed_signers"))?;
    ensure!(
        auth_digest(&signers) == trust.allowed_signers_sha256,
        "signer pin mismatch"
    );
    let policy_bytes = auth_read(&request.policy)?;
    let batch_bytes = auth_read(&request.batch)?;
    ensure!(
        auth_digest(&policy_bytes) == trust.policy_sha256,
        "inactive policy digest"
    );
    ensure!(
        auth_digest(&batch_bytes) == trust.batch_sha256,
        "inactive batch digest"
    );
    let policy: Policy = serde_json::from_slice(&policy_bytes).context("invalid signed policy")?;
    let batch: Batch = serde_json::from_slice(&batch_bytes).context("invalid signed batch")?;
    auth_check_scope(root, request, &trust, &policy, &batch)?;
    // Snapshot the public verification inputs so the process verifies the exact
    // bytes whose digests were checked, not a second read of mutable paths.
    let temp = Utf8PathBuf::from_path_buf(std::env::temp_dir())
        .map_err(|_| anyhow::anyhow!("temporary path is not UTF-8"))?
        .canonicalize_utf8()?;
    auth_require_external(&temp, &repository)?;
    let scratch = liminal_scratch::ScratchDir::new("assurance-signature")?;
    let signer_path = scratch.join("allowed_signers");
    fs::write(&signer_path, signers)?;
    auth_verify_signature(
        &scratch.join("policy.sig"),
        &auth_read(&request.policy_signature)?,
        &signer_path,
        &trust.principal,
        "liminal.assurance.policy.v1",
        &policy_bytes,
    )?;
    auth_verify_signature(
        &scratch.join("batch.sig"),
        &auth_read(&request.batch_signature)?,
        &signer_path,
        &trust.principal,
        "liminal.assurance.batch.v1",
        &batch_bytes,
    )?;
    Ok(serde_json::json!({
        "schema_version": 1, "batch_authenticated": true, "apply_authorized": false,
        "independent_review": "not-established", "batch_id": batch.id,
        "base_commit": batch.base_commit, "target": request.target,
        "policy_sha256": trust.policy_sha256, "batch_sha256": trust.batch_sha256,
        "tool_revision": trust.tool_revision, "tool_sha256": trust.tool_sha256
    }))
}

fn auth_check_executable(expected: &str) -> Result<()> {
    let executable = std::env::current_exe().context("locate maintenance executable")?;
    let mut file = fs::File::open(executable).context("read maintenance executable")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    ensure!(
        format!("{:x}", hasher.finalize()) == expected,
        "maintenance executable pin mismatch"
    );
    Ok(())
}

fn auth_check_scope(
    root: &Utf8Path,
    request: &AuthorizationRequest,
    trust: &Trust,
    policy: &Policy,
    batch: &Batch,
) -> Result<()> {
    ensure!(
        policy.schema_version == 1 && batch.schema_version == 1,
        "unsupported authorization schema"
    );
    ensure!(
        policy.repository_git_dir == trust.repository_git_dir,
        "policy repository mismatch"
    );
    ensure!(
        batch.policy_sha256 == trust.policy_sha256,
        "batch policy binding mismatch"
    );
    ensure!(
        policy.tool_revision == trust.tool_revision
            && batch.tool_revision == trust.tool_revision
            && policy.tool_sha256 == trust.tool_sha256
            && batch.tool_sha256 == trust.tool_sha256,
        "signed tool pin mismatch"
    );
    ensure!(
        policy.change_class == "coordinate-only" && batch.change_class == "coordinate-only",
        "non-coordinate authority refused"
    );
    ensure!(
        auth_identifier(&batch.id) && auth_hex(&batch.base_commit, 40),
        "invalid batch identity or base"
    );
    ensure!(request.base == batch.base_commit, "batch base mismatch");
    ensure!(
        auth_git(root, &["cat-file", "-t", &request.base])? == b"commit\n",
        "batch base is not a commit"
    );
    let unique: BTreeSet<_> = batch.targets.iter().collect();
    ensure!(
        !unique.is_empty()
            && unique.len() == batch.targets.len()
            && unique.iter().all(|id| auth_identifier(id)),
        "invalid or duplicate batch targets"
    );
    ensure!(
        unique.contains(&request.target),
        "target outside authorized batch"
    );
    Ok(())
}

fn auth_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn auth_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn auth_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn auth_read(path: &Utf8Path) -> Result<Vec<u8>> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "authorization input must be a regular file"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1_048_576,
        "authorization input exceeds 1 MiB"
    );
    Ok(bytes)
}

fn auth_git(root: &Utf8Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .current_dir(root)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "authorization repository lookup failed"
    );
    Ok(output.stdout)
}

fn auth_repository_paths(root: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let common = String::from_utf8(auth_git(root, &["rev-parse", "--git-common-dir"])?)?;
    // Remove output framing only. Whitespace can be part of repository identity.
    let common = common
        .strip_suffix('\n')
        .context("invalid Git common-directory framing")?;
    #[cfg(windows)]
    let common = common.strip_suffix('\r').unwrap_or(common);
    ensure!(!common.is_empty(), "empty Git common-directory path");
    let mut paths = vec![root.join(common).canonicalize_utf8()?];
    let worktrees = auth_git(root, &["worktree", "list", "--porcelain", "-z"])?;
    for field in worktrees.split(|byte| *byte == 0) {
        if let Some(path) = field.strip_prefix(b"worktree ") {
            paths.push(Utf8Path::new(std::str::from_utf8(path)?).canonicalize_utf8()?);
        }
    }
    ensure!(paths.len() > 1, "repository worktree scope unavailable");
    Ok(paths)
}

fn auth_require_external(path: &Utf8Path, repository: &[Utf8PathBuf]) -> Result<()> {
    ensure!(
        !repository.iter().any(|root| path.starts_with(root)),
        "trust or verification scratch is inside repository scope"
    );
    Ok(())
}

fn auth_verify_signature(
    signature_path: &Utf8Path,
    signature: &[u8],
    signers: &Utf8Path,
    principal: &str,
    namespace: &str,
    payload: &[u8],
) -> Result<()> {
    fs::write(signature_path, signature)?;
    let mut child = Command::new("ssh-keygen")
        .args(["-Y", "verify", "-f"])
        .arg(signers)
        .args(["-I", principal, "-n", namespace, "-s"])
        .arg(signature_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("OpenSSH signature verifier unavailable")?;
    let mut stdin = child
        .stdin
        .take()
        .context("signature verifier stdin missing")?;
    let payload = payload.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(&payload));
    let status = child.wait().context("wait for signature verification")?;
    let write = writer
        .join()
        .map_err(|_| anyhow::anyhow!("signature verifier writer panicked"))?;
    ensure!(
        write.is_ok() && status.success(),
        "signature verification failed for {namespace}"
    );
    Ok(())
}
