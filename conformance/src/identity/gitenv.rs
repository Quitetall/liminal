//! The hermetic git runbook (D07 §C). Git operations use the REAL `git` CLI
//! (never git2/gitoxide) so users' files pass through merge-ort. Every case
//! runs in an isolated temp repo with nothing inherited from the host git
//! config (D07.6 asserts `git --version` ≥ the config floor and FAILS the run
//! on older git — never skips).

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use rand::Rng;
use rand::rngs::SmallRng;

use crate::identity::Config;
use crate::identity::ops::{OpOutput, Operation, edit_block_first_word, resolve_union};
use crate::identity::strategy::BaseWorld;

/// A git conflict-resolution policy (seeded choice, D07 §C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// `git checkout --ours -- <file>`.
    Ours,
    /// `git checkout --theirs -- <file>`.
    Theirs,
    /// Strip conflict markers, keeping both sides in order.
    Union,
}

impl ConflictPolicy {
    /// The config/report name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ConflictPolicy::Ours => "ours",
            ConflictPolicy::Theirs => "theirs",
            ConflictPolicy::Union => "union",
        }
    }

    /// Parse a policy name.
    ///
    /// # Errors
    /// Unknown policy name.
    pub fn from_name(s: &str) -> anyhow::Result<ConflictPolicy> {
        match s {
            "ours" => Ok(ConflictPolicy::Ours),
            "theirs" => Ok(ConflictPolicy::Theirs),
            "union" => Ok(ConflictPolicy::Union),
            _ => Err(anyhow::anyhow!("unknown conflict policy: {s:?}")),
        }
    }
}

/// The exact `git --version` output line (trimmed), for the golden header and
/// the floor assertion.
///
/// # Errors
/// If `git` cannot be executed.
pub fn git_version() -> anyhow::Result<String> {
    let out = Command::new("git").arg("--version").output()?;
    if !out.status.success() {
        anyhow::bail!("git --version failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Parse the `MAJOR.MINOR` from a `git version 2.55.0` line.
fn parse_version(version_line: &str) -> Option<(u64, u64)> {
    let nums = version_line
        .split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = nums.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor))
}

/// D07.6: assert the running `git` is at least `min` (e.g. `"2.44"`). FAILS the
/// run on older git — never skips. Returns the exact version line on success.
///
/// # Errors
/// If `git` is missing, unparsable, or below the floor.
pub fn assert_min_git(min: &str) -> anyhow::Result<String> {
    let line = git_version()?;
    let running = parse_version(&line)
        .ok_or_else(|| anyhow::anyhow!("cannot parse git version from {line:?}"))?;
    let floor = parse_version(min)
        .or_else(|| {
            let mut p = min.split('.');
            Some((
                p.next()?.parse().ok()?,
                p.next().unwrap_or("0").parse().ok()?,
            ))
        })
        .ok_or_else(|| anyhow::anyhow!("cannot parse min_git {min:?}"))?;
    if running < floor {
        anyhow::bail!(
            "git {}.{} is below the corpus floor {min} (merge-ort era, D07.6)",
            running.0,
            running.1
        );
    }
    Ok(line)
}

/// The hermetic environment for a case repo (D07 §C): nothing inherited. Applied
/// to every `git` invocation in the case. `home` is the case temp dir.
#[must_use]
pub fn hermetic_env(home: &Utf8Path) -> Vec<(String, String)> {
    let xdg = home.join("xdg");
    vec![
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_CONFIG_SYSTEM".into(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("HOME".into(), home.to_string()),
        ("XDG_CONFIG_HOME".into(), xdg.to_string()),
        ("GIT_AUTHOR_NAME".into(), "liminal-lab".into()),
        ("GIT_AUTHOR_EMAIL".into(), "lab@liminal.invalid".into()),
        ("GIT_COMMITTER_NAME".into(), "liminal-lab".into()),
        ("GIT_COMMITTER_EMAIL".into(), "lab@liminal.invalid".into()),
        ("GIT_AUTHOR_DATE".into(), "2026-01-01T00:00:00Z".into()),
        ("GIT_COMMITTER_DATE".into(), "2026-01-01T00:00:00Z".into()),
        ("TZ".into(), "UTC".into()),
        ("LC_ALL".into(), "C".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_PAGER".into(), "cat".into()),
        ("PAGER".into(), "cat".into()),
        ("GIT_EDITOR".into(), "true".into()),
        ("GIT_SEQUENCE_EDITOR".into(), ":".into()),
    ]
}

/// A fresh scratch directory for one case, keyed by `label` (e.g.
/// `"git_merge-seed11"`). The path also carries the process id and a
/// per-process counter so concurrently-running tests never collide on the same
/// directory — the case is hermetic (relative filenames, pinned env), so the
/// location never affects the produced bytes; only uniqueness matters here.
///
/// # Errors
/// If the directory cannot be created.
pub fn case_tmpdir(label: &str) -> anyhow::Result<Utf8PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let base = Utf8PathBuf::from_path_buf(std::env::temp_dir())
        .map_err(|p| anyhow::anyhow!("non-UTF-8 temp dir: {}", p.display()))?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = base.join(format!("liminal-identity/{pid}-{n}-{label}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Run one `git` command in `repo` with the hermetic environment, returning it
/// for the caller to assert success. Clears the ambient environment first.
#[must_use]
pub fn git<'a>(repo: &Utf8Path, args: &'a [&'a str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo).env_clear();
    for (k, v) in hermetic_env(repo) {
        cmd.env(k, v);
    }
    cmd.args(args);
    cmd
}

/// Run one git torture operation (Algorithm B9–B11) in a hermetic repo.
///
/// Runbook (D07 §C):
/// 1. `case_tmpdir` → repo; `git -c init.defaultBranch=main init`.
/// 2. Write the template (and the sidecar when the sidecar strategy is active —
///    in this driver the sidecar content is captured separately, so writing the
///    file suffices); `git add -A` + `git commit`.
/// 3. `git checkout -b b`; edit block X on `b`; commit.
/// 4. `git checkout main`; edit block Y on `main` (X == Y with probability
///    `cfg.overlap_probability`, seeded); commit.
/// 5. Run the op: `git merge -s ort --no-edit b` | `git rebase main` (on `b`) |
///    `git cherry-pick <sha>` (onto `main`).
/// 6. On conflict, apply the seeded [`ConflictPolicy`]: **ours** = `git checkout
///    --ours -- <file>`; **theirs** = `--theirs`; **union** = strip the conflict
///    markers keeping both sides in order; then `git add <file>` +
///    `merge|rebase|cherry-pick --continue`.
/// 7. Read the post-op file bytes into an [`OpOutput`] carrying the chosen
///    policy.
///
/// # Errors
/// Propagates any git command failure or a below-floor `git`.
pub fn run_git_op(
    op: Operation,
    world: &BaseWorld,
    seed: u64,
    rng: &mut SmallRng,
    cfg: &Config,
) -> anyhow::Result<OpOutput> {
    let repo = case_tmpdir(&format!("{}-seed{seed}", op.name()))?;

    // 1-2: init + base commit.
    run(git(&repo, &["-c", "init.defaultBranch=main", "init"]))?;
    std::fs::write(repo.join("notes.md"), &world.file)?;
    run(git(&repo, &["add", "-A"]))?;
    run(git(&repo, &["commit", "-m", "base"]))?;

    // 3: seeded divergence (rng calls IN THIS ORDER for determinism).
    let n = world.blocks.len();
    let x = rng.random_range(0..n);
    let overlap = rng.random_bool(cfg.overlap_probability);
    let y = if overlap || n < 2 {
        x
    } else {
        (x + 1 + rng.random_range(0..(n - 1))) % n
    };
    let policy = seeded_policy(rng, cfg);

    // 4: branch `b` edits block x.
    run(git(&repo, &["checkout", "-b", "b"]))?;
    std::fs::write(
        repo.join("notes.md"),
        edit_block_first_word(world, x, "BranchB"),
    )?;
    run(git(&repo, &["commit", "-am", "edit-b"]))?;
    let b_sha = run(git(&repo, &["rev-parse", "HEAD"]))?.trim().to_owned();

    // 5: main edits block y.
    run(git(&repo, &["checkout", "main"]))?;
    std::fs::write(
        repo.join("notes.md"),
        edit_block_first_word(world, y, "OnMain"),
    )?;
    run(git(&repo, &["commit", "-am", "edit-main"]))?;

    // 6: run the op. A conflict makes git exit non-zero — that is expected.
    let op_out = match op {
        Operation::GitMerge => git(&repo, &["merge", "-s", "ort", "--no-edit", "b"]).output()?,
        Operation::GitRebase => {
            run(git(&repo, &["checkout", "b"]))?;
            git(&repo, &["rebase", "main"]).output()?
        }
        Operation::GitCherryPick => git(&repo, &["cherry-pick", b_sha.as_str()]).output()?,
        other => anyhow::bail!("run_git_op called with non-git operation {}", other.name()),
    };

    // 7: on conflict, resolve per the seeded policy and continue the operation.
    if !op_out.status.success() {
        match policy {
            ConflictPolicy::Ours => {
                run(git(&repo, &["checkout", "--ours", "--", "notes.md"]))?;
            }
            ConflictPolicy::Theirs => {
                run(git(&repo, &["checkout", "--theirs", "--", "notes.md"]))?;
            }
            ConflictPolicy::Union => {
                let conflicted = std::fs::read_to_string(repo.join("notes.md"))?;
                std::fs::write(repo.join("notes.md"), resolve_union(&conflicted))?;
            }
        }
        run(git(&repo, &["add", "notes.md"]))?;
        match op {
            // A merge commit records both parents, so it is never "empty" even
            // when a side was fully chosen — commit it directly.
            Operation::GitMerge => {
                run(git(&repo, &["commit", "--no-edit"]))?;
            }
            // A linear replay (rebase/cherry-pick) whose conflict resolution
            // reproduces the base tree yields an EMPTY patch, which git refuses
            // to commit. Detect that (nothing staged vs HEAD) and `--skip` the
            // now-empty patch to finish the sequence cleanly; otherwise
            // `--continue`.
            Operation::GitRebase => continue_or_skip(&repo, "rebase")?,
            Operation::GitCherryPick => continue_or_skip(&repo, "cherry-pick")?,
            _ => unreachable!("filtered to git ops above"),
        }
    }

    // 8: read the post-op file.
    let file = std::fs::read_to_string(repo.join("notes.md"))?;
    Ok(OpOutput {
        file,
        sidecar_moved: true,
        policy: Some(policy),
    })
}

/// Pick a seeded conflict policy from the configured list.
#[must_use]
pub fn seeded_policy(rng: &mut SmallRng, cfg: &Config) -> ConflictPolicy {
    let i = rng.random_range(0..cfg.conflict_policies.len());
    cfg.conflict_policies[i]
}

/// Finish a conflicted rebase/cherry-pick: if the staged resolution reproduces
/// HEAD's tree the patch is empty (git rejects an empty linear commit), so
/// `--skip` it; otherwise `--continue`.
fn continue_or_skip(repo: &Utf8Path, verb: &str) -> anyhow::Result<()> {
    // `git diff --cached --quiet` exits 0 iff nothing is staged relative to HEAD.
    let empty = git(repo, &["diff", "--cached", "--quiet"])
        .output()?
        .status
        .success();
    let arg = if empty { "--skip" } else { "--continue" };
    run(git(repo, &[verb, arg]))?;
    Ok(())
}

/// Run a git command that MUST succeed; bail with its stderr on failure.
/// Returns captured stdout.
fn run(mut cmd: Command) -> anyhow::Result<String> {
    let out = cmd.output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git command failed: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
