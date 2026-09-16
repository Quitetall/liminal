//! Fixed-command execution with explicit incomplete receipts.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    profile: String,
    state: String,
    qualification_established: bool,
    source_commit: Option<String>,
    source_dirty: Option<bool>,
    platform: String,
    architecture: String,
    cache_state_reported: String,
    sample_count: u32,
    impact_advisory: Vec<String>,
    commands: Vec<Observation>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    id: String,
    state: String,
    informational: bool,
    argv: Vec<String>,
    exit_code: Option<i32>,
    signal: Option<i32>,
    elapsed_ms: Option<u128>,
    peak_memory_bytes: Option<u64>,
    error: Option<String>,
}

/// Run an entire registered profile into a new external receipt directory.
///
/// # Errors
/// Refuses malformed catalogs, unknown profiles, existing/in-tree outputs and
/// failed mandatory commands. Receipts never establish qualification.
pub fn run_profile(root: &Utf8Path, profile: &str, output: &Utf8Path, cache: &str) -> Result<()> {
    let root = root.canonicalize_utf8()?;
    ensure!(
        profile != "merge" || std::env::var_os("CARGO_TARGET_DIR").is_none(),
        "merge requires unset CARGO_TARGET_DIR: crash and sanitizer replay need independent checkout-local targets"
    );
    let catalog = super::load_catalog(&root)?;
    super::check_catalog(&root, &catalog)?;
    super::workflows::check(&root, &catalog)?;
    let ids = catalog
        .profiles
        .get(profile)
        .context("unknown assurance profile")?;
    ensure!(
        ["cold", "warm", "unknown"].contains(&cache),
        "unknown cache classification"
    );
    let output = fresh_output(&root, output)?;
    let mut receipt = Receipt {
        schema_version: 1,
        profile: profile.into(),
        state: "incomplete".into(),
        qualification_established: false,
        source_commit: git_text(&root, &["rev-parse", "HEAD"]),
        source_dirty: git_text(&root, &["status", "--porcelain"]).map(|s| !s.is_empty()),
        platform: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        cache_state_reported: cache.into(),
        sample_count: 1,
        impact_advisory: impact(&root, &catalog),
        commands: ids
            .iter()
            .map(|id| Observation {
                id: id.clone(),
                state: "unexecuted".into(),
                informational: id == "beta",
                argv: Vec::new(),
                exit_code: None,
                signal: None,
                elapsed_ms: None,
                peak_memory_bytes: None,
                error: None,
            })
            .collect(),
    };
    save(&output, &receipt)?;
    // Cargo can atomically replace this executable while tests build. Resolve
    // command paths before starting any child, not from a later deleted inode.
    let commands = ids
        .iter()
        .map(|id| registered(id, &output))
        .collect::<Result<Vec<_>>>()?;
    for (index, command) in commands.into_iter().enumerate() {
        receipt.commands[index].state = "running".into();
        save(&output, &receipt)?;
        observe(&root, &output, &mut receipt.commands[index], command);
        let failed =
            receipt.commands[index].state != "passed" && !receipt.commands[index].informational;
        if failed {
            receipt.state.clone_from(&receipt.commands[index].state);
        }
        save(&output, &receipt)?;
        if failed {
            bail!(
                "profile {profile}: {}; receipt: {output}/receipt.json",
                receipt.state
            );
        }
    }
    receipt.state = "passed".into();
    save(&output, &receipt)?;
    println!(
        "profile {profile}: passed; qualification: not-established; receipt: {output}/receipt.json"
    );
    Ok(())
}

fn fresh_output(root: &Utf8Path, output: &Utf8Path) -> Result<Utf8PathBuf> {
    ensure!(output.is_absolute(), "receipt output must be absolute");
    let parent = output
        .parent()
        .context("receipt parent required")?
        .canonicalize_utf8()?;
    ensure!(
        !parent.starts_with(root),
        "receipt output must be outside repository"
    );
    let name = output
        .file_name()
        .context("receipt directory name required")?;
    ensure!(name != "." && name != "..", "invalid receipt directory");
    let output = parent.join(name);
    std::fs::create_dir(&output).context("receipt directory must be new")?;
    Ok(output)
}

fn git_text(root: &Utf8Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().into())
}

fn impact(root: &Utf8Path, catalog: &super::Catalog) -> Vec<String> {
    // Advisory only. Unknown/untracked change sets conservatively name all
    // families. Never use this list to omit commands from any profile.
    let changed = git_text(root, &["diff", "--name-only", "HEAD"]);
    let status = git_text(root, &["status", "--porcelain"]);
    let all = status
        .as_ref()
        .is_none_or(|s| s.lines().any(|line| line.starts_with("??")));
    catalog
        .families
        .iter()
        .filter(|family| {
            all || changed.as_ref().is_none_or(|paths| {
                paths.lines().any(|path| {
                    family
                        .source_areas
                        .iter()
                        .any(|area| Utf8Path::new(path).starts_with(area))
                        || [
                            "Cargo.lock",
                            "Cargo.toml",
                            "rust-toolchain.toml",
                            "justfile",
                        ]
                        .contains(&path)
                        || path.starts_with("verification/assurance/")
                        || path.starts_with(".github/")
                })
            })
        })
        .map(|family| family.id.clone())
        .collect()
}

fn save(output: &Utf8Path, receipt: &Receipt) -> Result<()> {
    let temporary = output.join("receipt.pending");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(receipt)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, output.join("receipt.json"))?;
    #[cfg(unix)]
    File::open(output)?.sync_all()?;
    Ok(())
}

fn observe(root: &Utf8Path, output: &Utf8Path, observation: &mut Observation, command: Command) {
    let start = Instant::now();
    match execute(root, output, observation, command) {
        Ok(status) => {
            observation.exit_code = status.code();
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                observation.signal = status.signal();
            }
            observation.state = if status.success() { "passed" } else { "failed" }.into();
            if observation.id.starts_with("resource-") && status.code() == Some(3) {
                observation.state = "infrastructure-unavailable".into();
            }
            if status.code().is_none() {
                observation.error =
                    Some("terminated without an exit code (signal or platform equivalent)".into());
            }
        }
        Err(error) => {
            observation.state = "infrastructure-unavailable".into();
            observation.error = Some(format!("{error:#}"));
        }
    }
    observation.elapsed_ms = Some(start.elapsed().as_millis());
    if observation.id.starts_with("resource-") {
        observation.peak_memory_bytes =
            std::fs::read(output.join(&observation.id).join("receipt.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|receipt| {
                    receipt["service"]["MemoryPeak"]
                        .as_str()
                        .and_then(|value| value.parse().ok())
                });
    }
}

fn execute(
    root: &Utf8Path,
    output: &Utf8Path,
    observation: &mut Observation,
    mut command: Command,
) -> Result<std::process::ExitStatus> {
    observation.argv = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let stdout = File::create(output.join(format!("{}.stdout", observation.id)))?;
    let stderr = File::create(output.join(format!("{}.stderr", observation.id)))?;
    command
        .current_dir(root)
        .env("CARGO_BUILD_JOBS", "2")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .with_context(|| format!("launch {}", observation.id))
}

pub(super) fn registered(id: &str, output: &Utf8Path) -> Result<Command> {
    let argv: &[&str] = match id {
        "style" => &["just", "fmt-check"],
        "lint" => &["just", "lint"],
        "bootstrap" => &["just", "formal-bootstrap-self-test"],
        "proof-support" => &["just", "formal-proof-self-test"],
        "nextest" => &[
            "cargo",
            "nextest",
            "run",
            "--workspace",
            "--all-features",
            "--profile",
            "ci",
        ],
        "threaded" => &["just", "test-threaded"],
        "doctest" => &["cargo", "test", "--workspace", "--doc"],
        "docs" => &["just", "doc"],
        "deny" => &["just", "deny"],
        "inventory" => &["just", "haq-inventory"],
        "canaries" => &["just", "haq-canaries"],
        "advisories" => &["cargo", "deny", "check", "advisories"],
        "beta" => &["cargo", "+beta", "check", "--workspace", "--all-targets"],
        "haqp" => &["just", "haq-lane"],
        "assurance" => {
            let mut command = Command::new(std::env::current_exe()?);
            command.args(["assurance", "check"]);
            return Ok(command);
        }
        "resource-run" | "resource-lint" => {
            let mut command = Command::new("python3");
            command
                .args(["-B", "verification/assurance/resource.py", id, "--output"])
                .arg(output.join(id));
            return Ok(command);
        }
        _ => bail!("unregistered command: {id}"),
    };
    let mut command = Command::new(argv[0]);
    command.args(&argv[1..]);
    Ok(command)
}

/// Read one execution receipt without rerunning or repairing it.
///
/// # Errors
/// Rejects malformed receipts, unknown profiles/commands or inconsistent success.
pub fn report(path: &Utf8Path) -> Result<()> {
    let receipt: Receipt = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        [
            "incomplete",
            "passed",
            "failed",
            "infrastructure-unavailable"
        ]
        .contains(&receipt.state.as_str()),
        "unknown profile state"
    );
    ensure!(
        receipt.sample_count == 1,
        "receipt is one invocation, not an aggregate"
    );
    ensure!(
        receipt.schema_version == 1 && !receipt.qualification_established,
        "invalid maintenance receipt authority"
    );
    let (_, expected) = super::PROFILES
        .iter()
        .find(|(name, _)| *name == receipt.profile)
        .context("unknown receipt profile")?;
    ensure!(
        receipt
            .commands
            .iter()
            .map(|c| c.id.as_str())
            .eq(expected.iter().copied()),
        "receipt command inventory drift"
    );
    for command in &receipt.commands {
        ensure!(
            [
                "unexecuted",
                "running",
                "passed",
                "failed",
                "infrastructure-unavailable",
                "deferred"
            ]
            .contains(&command.state.as_str()),
            "unknown command state"
        );
        ensure!(
            command.informational == (command.id == "beta"),
            "receipt authority drift"
        );
        ensure!(
            command.state != "passed" || command.exit_code == Some(0),
            "success without zero exit"
        );
    }
    ensure!(
        receipt.state != "passed"
            || receipt
                .commands
                .iter()
                .all(|c| c.informational || c.state == "passed"),
        "incomplete receipt claims pass"
    );
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
