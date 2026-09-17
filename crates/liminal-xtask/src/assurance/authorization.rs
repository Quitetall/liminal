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
    /// Required for authorization-only checking and isolated apply.
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
    /// Candidate commit for optional registry/patch binding.
    #[arg(long)]
    candidate: Option<String>,
    /// Relative Rust source path for optional coordinate binding.
    #[arg(long)]
    source: Option<Utf8PathBuf>,
    /// Existing source line for optional coordinate binding.
    #[arg(long)]
    line: Option<usize>,
    /// Exact existing source line anchor for optional coordinate binding.
    #[arg(long)]
    anchor: Option<String>,
    /// External trusted-adapter review receipt for optional review binding.
    #[arg(long)]
    review_receipt: Option<Utf8PathBuf>,
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

#[derive(Deserialize)]
struct RegistryPacket {
    mutants: Vec<RegistryMutant>,
}

#[derive(Deserialize)]
struct RegistryMutant {
    id: String,
    source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewReceipt {
    schema_version: u32,
    reviewer: String,
    backend: String,
    base_commit: String,
    candidate_commit: String,
    target: String,
    patch_sha256: String,
    verdict: String,
    unresolved_verified_findings: u32,
    findings: Vec<ReviewFinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewFinding {
    id: String,
    classification: String,
    independently_reproduced: bool,
    resolved: bool,
}

struct ReviewVerification {
    receipt_sha256: String,
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
    let registry = auth_check_optional_registry(root, request)?;
    let review = auth_check_optional_review(root, request, &trust_root)?;
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
        "independent_review": if review.is_some() { "receipt-verified" } else { "not-established" }, "batch_id": batch.id,
        "base_commit": batch.base_commit, "target": request.target,
        "registry_binding": registry.unwrap_or("not-requested"),
        "review_receipt_sha256": review.as_ref().map(|item| item.receipt_sha256.as_str()),
        "policy_sha256": trust.policy_sha256, "batch_sha256": trust.batch_sha256,
        "tool_revision": trust.tool_revision, "tool_sha256": trust.tool_sha256
    }))
}

/// Apply an already authenticated coordinate patch in a fresh external
/// worktree. The source checkout is never modified and no commit, push or
/// signature operation is performed.
pub fn apply_isolated(
    root: &Utf8Path,
    request: &AuthorizationRequest,
    output: &Utf8Path,
) -> Result<serde_json::Value> {
    ensure!(
        request.review_receipt.is_some(),
        "isolated apply requires a verified review receipt"
    );
    let summary = check_authorization(root, request)?;
    let candidate = request
        .candidate
        .as_deref()
        .context("isolated apply requires candidate binding")?;
    let source = request
        .source
        .as_ref()
        .context("isolated apply requires source binding")?;
    let repository = auth_repository_paths(root)?;
    auth_prepare_apply_destination(output, &repository)?;
    let patch = auth_git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--binary",
            "--full-index",
            &request.base,
            candidate,
            "--",
            source.as_str(),
            "conformance/haqp/packet.json",
        ],
    )?;
    auth_git_add_worktree(root, output, &request.base)?;
    let mut guard = ApplyWorktreeGuard::new(root, output);
    auth_git_apply(output, &patch)?;
    let remaining = auth_git(
        output,
        &[
            "diff",
            "--no-ext-diff",
            "--binary",
            "--full-index",
            "--cached",
            candidate,
            "--",
        ],
    )?;
    ensure!(
        remaining.is_empty(),
        "isolated apply tree differs from candidate commit"
    );
    ensure!(
        auth_git(output, &["ls-files", "--others", "--exclude-standard"])?.is_empty(),
        "isolated apply produced untracked files"
    );
    let packet_digest = crate::haq::packet_digest_repo(output)
        .context("derive packet digest in isolated worktree")?;
    guard.keep();
    let mut applied = summary;
    applied["apply_authorized"] = serde_json::json!(true);
    applied["isolated_worktree"] = serde_json::json!(output);
    applied["candidate_commit"] = serde_json::json!(candidate);
    applied["packet_digest"] = serde_json::json!(packet_digest);
    applied["committed"] = serde_json::json!(false);
    applied["pushed"] = serde_json::json!(false);
    applied["signed"] = serde_json::json!(false);
    Ok(applied)
}

fn auth_prepare_apply_destination(output: &Utf8Path, repository: &[Utf8PathBuf]) -> Result<()> {
    ensure!(
        output.is_absolute(),
        "absolute isolated apply output required"
    );
    ensure!(
        fs::symlink_metadata(output).is_err(),
        "isolated apply output already exists"
    );
    let parent = output
        .parent()
        .context("isolated apply output has no parent")?
        .canonicalize_utf8()
        .context("isolated apply output parent unavailable")?;
    ensure!(
        parent.is_dir(),
        "isolated apply output parent is not a directory"
    );
    auth_require_external(&parent, repository)
}

fn auth_git_add_worktree(root: &Utf8Path, output: &Utf8Path, base: &str) -> Result<()> {
    auth_git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            output.as_str(),
            base,
        ],
    )?;
    Ok(())
}

fn auth_git_apply(root: &Utf8Path, patch: &[u8]) -> Result<()> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let mut child = command
        .current_dir(root)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(["apply", "--index", "--whitespace=nowarn"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("git apply unavailable")?;
    let mut stdin = child.stdin.take().context("git apply stdin missing")?;
    let payload = patch.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(&payload));
    let status = child.wait().context("wait for git apply")?;
    let write = writer
        .join()
        .map_err(|_| anyhow::anyhow!("git apply writer panicked"))?;
    ensure!(
        write.is_ok() && status.success(),
        "isolated apply rejected candidate patch"
    );
    Ok(())
}

struct ApplyWorktreeGuard<'a> {
    root: &'a Utf8Path,
    output: &'a Utf8Path,
    active: bool,
}

impl<'a> ApplyWorktreeGuard<'a> {
    fn new(root: &'a Utf8Path, output: &'a Utf8Path) -> Self {
        Self {
            root,
            output,
            active: true,
        }
    }

    fn keep(&mut self) {
        self.active = false;
    }
}

impl Drop for ApplyWorktreeGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = auth_git(
                self.root,
                &["worktree", "remove", "--force", self.output.as_str()],
            );
        }
    }
}

fn auth_check_optional_review(
    root: &Utf8Path,
    request: &AuthorizationRequest,
    trust_root: &Utf8Path,
) -> Result<Option<ReviewVerification>> {
    let Some(receipt) = &request.review_receipt else {
        return Ok(None);
    };
    // Review is deliberately coupled to the complete coordinate binding. The
    // registry check runs first and establishes candidate/source validity.
    let (Some(candidate), Some(source), Some(_line), Some(_anchor)) = (
        &request.candidate,
        &request.source,
        request.line,
        &request.anchor,
    ) else {
        anyhow::bail!("review receipt requires complete registry binding inputs");
    };
    ensure!(
        receipt.is_absolute(),
        "absolute external review receipt required"
    );
    let canonical = receipt
        .canonicalize_utf8()
        .context("external review receipt unavailable")?;
    ensure!(
        canonical.starts_with(trust_root),
        "review receipt must be under external trust root"
    );
    let bytes = auth_read(receipt)?;
    let review: ReviewReceipt =
        serde_json::from_slice(&bytes).context("invalid external review receipt")?;
    ensure!(
        review.schema_version == 1,
        "unsupported review receipt schema"
    );
    ensure!(
        auth_label(&review.reviewer) && auth_label(&review.backend),
        "invalid review identity"
    );
    ensure!(
        review.base_commit == request.base
            && review.candidate_commit == *candidate
            && review.target == request.target,
        "review receipt scope mismatch"
    );
    ensure!(
        auth_hex(&review.patch_sha256, 64),
        "invalid review patch digest"
    );
    let patch = auth_git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--binary",
            "--full-index",
            &request.base,
            candidate,
            "--",
            source.as_str(),
            "conformance/haqp/packet.json",
        ],
    )?;
    ensure!(
        auth_digest(&patch) == review.patch_sha256,
        "review receipt patch digest mismatch"
    );
    ensure!(review.verdict == "pass", "review receipt is not a pass");
    ensure!(
        review.unresolved_verified_findings == 0,
        "review receipt has unresolved verified findings"
    );
    let mut finding_ids = BTreeSet::new();
    for finding in &review.findings {
        ensure!(
            auth_identifier(&finding.id) && finding_ids.insert(&finding.id),
            "review receipt has invalid or duplicate finding"
        );
        ensure!(
            matches!(
                finding.classification.as_str(),
                "false_positive" | "caught_violation" | "verified_defect"
            ),
            "review receipt has unknown finding classification"
        );
        ensure!(finding.resolved, "review receipt has unresolved finding");
        if finding.classification == "verified_defect" {
            ensure!(
                finding.independently_reproduced,
                "verified review finding lacks independent reproduction"
            );
        }
    }
    Ok(Some(ReviewVerification {
        receipt_sha256: auth_digest(&bytes),
    }))
}

fn auth_check_optional_registry(
    root: &Utf8Path,
    request: &AuthorizationRequest,
) -> Result<Option<&'static str>> {
    match (
        &request.candidate,
        &request.source,
        request.line,
        &request.anchor,
    ) {
        (Some(candidate), Some(source), Some(line), Some(anchor)) => Ok(Some(
            auth_check_registry_binding(root, request, candidate, source, line, anchor)?,
        )),
        (None, None, None, None) => Ok(None),
        _ => anyhow::bail!(
            "candidate, source, line and anchor must be supplied together for registry binding"
        ),
    }
}

fn auth_check_registry_binding(
    root: &Utf8Path,
    request: &AuthorizationRequest,
    candidate: &str,
    source: &Utf8Path,
    line: usize,
    anchor: &str,
) -> Result<&'static str> {
    ensure!(auth_hex(candidate, 40), "invalid candidate commit pin");
    ensure!(
        auth_identifier(&request.target),
        "invalid target identifier"
    );
    ensure!(
        source.extension() == Some("rs")
            && !source.is_absolute()
            && source
                .components()
                .all(|part| matches!(part, camino::Utf8Component::Normal(_)))
            && !source.as_str().contains("conformance/corpora/heldout"),
        "unsafe registry source path"
    );
    ensure!(line > 0, "invalid registry source line");
    let expected = auth_closed_registry_entry(&request.target)?;
    ensure!(
        source.as_str() == expected.0,
        "source path does not match closed registry"
    );
    let proposal = crate::assurance::amendment::propose_coordinate(
        root,
        &request.base,
        candidate,
        source,
        line,
        anchor,
    )?;
    ensure!(
        proposal.source == expected.0 && proposal.old_line == expected.1,
        "base coordinate does not match closed registry"
    );
    let candidate_source = format!("{}:{}", expected.0, proposal.new_line);
    ensure!(
        auth_packet_source(root, &request.base, &request.target)? == expected.2,
        "base packet source does not match closed registry"
    );
    ensure!(
        auth_packet_source(root, candidate, &request.target)? == candidate_source,
        "candidate packet source does not match moved coordinate"
    );
    auth_check_candidate_diff(
        root,
        &request.base,
        candidate,
        source,
        &expected.2,
        &candidate_source,
    )?;
    Ok("verified")
}

fn auth_closed_registry_entry(target: &str) -> Result<(String, usize, String)> {
    let packet: RegistryPacket =
        serde_json::from_slice(include_bytes!("../../../../conformance/haqp/packet.json"))
            .context("compiled closed registry is malformed")?;
    ensure!(
        packet.mutants.len() == 65,
        "compiled closed registry count drift"
    );
    let mut ids = BTreeSet::new();
    for mutant in &packet.mutants {
        ensure!(
            ids.insert(&mutant.id),
            "compiled closed registry has duplicate target"
        );
        auth_registry_coordinate(&mutant.source)?;
    }
    let mut matches = packet.mutants.iter().filter(|mutant| mutant.id == target);
    let mutant = matches
        .next()
        .context("target absent from closed registry")?;
    ensure!(
        matches.next().is_none(),
        "target duplicated in closed registry"
    );
    let (file, line) = auth_registry_coordinate(&mutant.source)?;
    Ok((file.to_owned(), line, mutant.source.clone()))
}

fn auth_registry_coordinate(source: &str) -> Result<(&str, usize)> {
    let (file, line) = source
        .rsplit_once(':')
        .context("closed registry source is not file:line")?;
    let line: usize = line
        .parse()
        .context("closed registry line is not numeric")?;
    ensure!(
        line > 0
            && !file.is_empty()
            && !file.starts_with('/')
            && file
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
            && Utf8Path::new(file).extension() == Some("rs"),
        "unsafe closed registry coordinate"
    );
    Ok((file, line))
}

fn auth_packet_source(root: &Utf8Path, revision: &str, target: &str) -> Result<String> {
    ensure!(auth_hex(revision, 40), "invalid packet revision");
    let spec = format!("{revision}:conformance/haqp/packet.json");
    let bytes = auth_git(root, &["cat-file", "blob", &spec])?;
    let packet: RegistryPacket = serde_json::from_slice(&bytes).context("invalid packet JSON")?;
    let mut matches = packet.mutants.iter().filter(|mutant| mutant.id == target);
    let mutant = matches.next().context("target absent from packet")?;
    ensure!(matches.next().is_none(), "target duplicated in packet");
    Ok(mutant.source.clone())
}

fn auth_check_candidate_diff(
    root: &Utf8Path,
    base: &str,
    candidate: &str,
    source: &Utf8Path,
    old_packet_source: &str,
    new_packet_source: &str,
) -> Result<()> {
    let names = String::from_utf8(auth_git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--name-status",
            base,
            candidate,
            "--",
        ],
    )?)?;
    let mut paths = Vec::new();
    for row in names.lines() {
        let (status, path) = row
            .split_once('\t')
            .context("candidate diff has malformed name-status row")?;
        ensure!(
            status == "M",
            "candidate diff contains non-modification status"
        );
        paths.push(path);
    }
    let packet_path = "conformance/haqp/packet.json";
    let mut expected_paths = vec![source.as_str(), packet_path];
    expected_paths.sort_unstable();
    paths.sort_unstable();
    ensure!(
        paths == expected_paths,
        "candidate changes files outside coordinate scope"
    );
    ensure!(
        auth_git(
            root,
            &["diff", "--no-ext-diff", "--summary", base, candidate, "--"]
        )?
        .is_empty(),
        "candidate changes file metadata"
    );

    let diff = String::from_utf8(auth_git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--unified=0",
            base,
            candidate,
            "--",
            packet_path,
        ],
    )?)?;
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for row in diff.lines() {
        if row.starts_with("---") || row.starts_with("+++") {
            continue;
        }
        if let Some(value) = row.strip_prefix('-') {
            removed.push(value);
        } else if let Some(value) = row.strip_prefix('+') {
            added.push(value);
        }
    }
    let packet_line = |line: &str, expected: &str| {
        let line = line.strip_suffix(',').unwrap_or(line);
        let leading = line.len() - line.trim_start().len();
        let content = line.trim_start();
        (leading, content == format!("\"source\": \"{expected}\""))
    };
    let removed_shape = removed
        .first()
        .map(|line| packet_line(line, old_packet_source));
    let added_shape = added
        .first()
        .map(|line| packet_line(line, new_packet_source));
    let valid_change = matches!(
        (removed_shape, added_shape),
        (Some((removed_indent, true)), Some((added_indent, true)))
            if removed_indent == added_indent
    );
    ensure!(
        removed.len() == 1 && added.len() == 1 && valid_change,
        "candidate packet diff is not one exact source-coordinate replacement"
    );
    Ok(())
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

fn auth_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
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
