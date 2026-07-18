//! Crash-orchestration harness (R4 §10; docs/implementation-plan.md §3).
//!
//! Architecture: in-process fault points + REAL subprocess death. The harness
//! spawns `liminald exec` with `LIMINAL_CRASHPOINT` armed; the injector
//! appends every boundary hit to a trace file and `abort()`s on match (real
//! SIGABRT — no unwinding, no flushes, honest fsync testing).
//!
//! The crash matrix is DERIVED, never hand-listed: every `(point, occurrence)`
//! observed in a sound baseline run becomes one crash case automatically, so a
//! newly added durable boundary cannot be silently untested.

use std::collections::BTreeMap;
use std::process::Output;

use camino::{Utf8Path, Utf8PathBuf};
use liminal_id::ContentHash;
use liminal_jurisdiction::IntentState;

use crate::scenario::ScenarioScript;

/// Resolve the `lim-toy` binary path. At test-compile time `CARGO_BIN_EXE_*`
/// is available; at lib-compile time we search the target directory.
fn lim_toy_path() -> Utf8PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_lim-toy") {
        return Utf8PathBuf::from(path);
    }
    resolve_target_bin("lim-toy")
}

/// Resolve the `lim` CLI binary (owned by `liminal-cli`, so never available via
/// this crate's `CARGO_BIN_EXE_*`). Searches the workspace target directory.
fn lim_path() -> Utf8PathBuf {
    resolve_target_bin("lim")
}

/// Search the workspace `target/{debug,release}/` for a built binary.
fn resolve_target_bin(name: &str) -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().unwrap_or(&manifest);
    let candidates = [
        workspace.join(format!("target/debug/{name}")),
        workspace.join(format!("target/release/{name}")),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    panic!(
        "{name} binary not found; build the workspace binaries or run tests via `cargo nextest run`"
    )
}

/// Exclusions forced by `ToyRun::trace_path()` living inside the root.
const EXCLUDED_NAMES: &[&str] = &["crash-trace.log", "recover-trace.log", "scenario.toml"];

/// Hash every regular non-excluded file under `root`, sorted by relative path.
pub fn workspace_file_hashes(root: &Utf8Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() == Some("state") {
                    continue;
                }
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|e| anyhow::anyhow!("path escape: {e}"))?
                    .to_string();
                if EXCLUDED_NAMES.contains(&rel.as_str()) {
                    continue;
                }
                let bytes = std::fs::read(&path)?;
                out.insert(rel, ContentHash::of(&bytes).to_hex());
            }
        }
    }
    Ok(out)
}

/// The ordered boundary hits of one run, parsed from the injector's trace
/// file (`<name>:<occurrence>` per line).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitTrace {
    /// `(boundary name, occurrence)` in firing order.
    pub hits: Vec<(String, u64)>,
}

impl HitTrace {
    /// Parse a trace file's contents.
    #[must_use]
    pub fn parse(contents: &str) -> Self {
        let hits = contents
            .lines()
            .filter_map(|line| {
                let (name, occ) = line.rsplit_once(':')?;
                Some((name.to_owned(), occ.parse().ok()?))
            })
            .collect();
        Self { hits }
    }

    /// Whether a given `(point, occurrence)` fired. A crash test whose armed
    /// point never fired is a FALSE PASS and must fail.
    #[must_use]
    pub fn fired(&self, point: &str, occurrence: u64) -> bool {
        self.hits
            .iter()
            .any(|(n, o)| n == point && *o == occurrence)
    }

    /// Every `(point, occurrence)` pair, i.e. the derived crash matrix for
    /// the scenario that produced this baseline trace.
    #[must_use]
    pub fn enumerate_faults(&self) -> Vec<(String, u64)> {
        self.hits.clone()
    }
}

/// Observed world state after a crash run.
#[derive(Debug, Clone)]
pub struct CrashState {
    /// The hit trace up to (and including) the fatal boundary.
    pub trace: HitTrace,
}

/// The outcome of running `liminald recover` on a crashed workspace.
#[derive(Debug, Clone)]
pub struct RecoveryReport {
    /// Terminal state reached per intent — MUST all be terminal
    /// (`Committed` / `NeedsReview` / `Aborted`), never hidden half-state
    /// (R4 §10).
    pub terminals: Vec<IntentState>,
    /// Digest of the observable world (store head + file hashes) used for the
    /// recovery-idempotence assertion: recovering twice must be a no-op.
    pub world_digest: String,
}

/// One toy workspace under harness control: a scratch directory holding
/// `state/` (the store) and the scenario's files.
#[derive(Debug)]
pub struct ToyRun {
    /// Workspace root.
    pub root: Utf8PathBuf,
}

impl ToyRun {
    /// Create a fresh scratch workspace.
    pub fn new(label: &str) -> anyhow::Result<Self> {
        let root = Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
            "liminal-toyrun-{label}-{}-{:x}",
            std::process::id(),
            liminal_id::Timestamp::now().0
        ));
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Run `lim-toy exec <root> <scenario>` with NO fault armed and NO trace
    /// requirement (a scenario may legitimately reach no ILRP boundary). Fails
    /// if the process exits nonzero — that is a rejected write path.
    pub fn exec_scenario(&self, scenario: &ScenarioScript) -> anyhow::Result<Output> {
        let scenario_path = self.root.join("scenario.toml");
        std::fs::write(&scenario_path, toml::to_string(scenario)?)?;
        let output = std::process::Command::new(lim_toy_path())
            .args(["exec", self.root.as_str(), scenario_path.as_str()])
            .env_remove("LIMINAL_CRASHPOINT")
            .env("LIMINAL_CRASH_TRACE", self.trace_path())
            .output()?;
        Ok(output)
    }

    /// Run `lim check` against this workspace, returning its raw output for
    /// byte-exact silence assertions (`assert_silent`).
    pub fn check(&self) -> anyhow::Result<Output> {
        self.lim(&["check"])
    }

    /// Run an arbitrary `lim <args...>` subcommand against this workspace.
    pub fn lim(&self, args: &[&str]) -> anyhow::Result<Output> {
        let mut cmd = std::process::Command::new(lim_path());
        cmd.args(["--workspace", self.root.as_str()]).args(args);
        Ok(cmd.output()?)
    }

    /// Sound run: spawn `lim-toy exec <root> <scenario>` with NO fault armed;
    /// return the ordered hit trace (the derived crash matrix).
    pub fn baseline(&self, scenario: &ScenarioScript) -> anyhow::Result<HitTrace> {
        let scenario_path = self.root.join("scenario.toml");
        std::fs::write(&scenario_path, toml::to_string(scenario)?)?;

        let output = std::process::Command::new(lim_toy_path())
            .args(["exec", self.root.as_str(), scenario_path.as_str()])
            .env_remove("LIMINAL_CRASHPOINT")
            .env("LIMINAL_CRASH_TRACE", self.trace_path())
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "baseline exec failed: status={:?}\nstderr={}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let trace_contents = std::fs::read_to_string(self.trace_path())?;
        let trace = HitTrace::parse(&trace_contents);
        anyhow::ensure!(!trace.hits.is_empty(), "baseline trace must be non-empty");
        Ok(trace)
    }

    /// Crash run: spawn with `LIMINAL_CRASHPOINT=<point>:<occurrence>`;
    /// assert the process died by SIGABRT AND that the trace proves the point
    /// actually fired.
    pub fn run_to_crash(
        &self,
        scenario: &ScenarioScript,
        point: &str,
        occurrence: u64,
    ) -> anyhow::Result<CrashState> {
        let scenario_path = self.root.join("scenario.toml");
        std::fs::write(&scenario_path, toml::to_string(scenario)?)?;

        let status = std::process::Command::new(lim_toy_path())
            .args(["exec", self.root.as_str(), scenario_path.as_str()])
            .env("LIMINAL_CRASHPOINT", format!("{point}:{occurrence}"))
            .env("LIMINAL_CRASH_TRACE", self.trace_path())
            .status()?;

        anyhow::ensure!(!status.success(), "crash run must not exit 0");
        anyhow::ensure!(
            status.code().is_none(),
            "must die by signal, not exit code (got {:?})",
            status.code()
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            anyhow::ensure!(
                status.signal() == Some(6),
                "must die by SIGABRT (signal 6), got {:?}",
                status.signal()
            );
        }

        let trace_contents = std::fs::read_to_string(self.trace_path())?;
        let trace = HitTrace::parse(&trace_contents);
        anyhow::ensure!(
            trace.fired(point, occurrence),
            "crash test's armed point {point}:{occurrence} never fired — false pass"
        );

        Ok(CrashState { trace })
    }

    /// Fresh subprocess, same store dir: `lim-toy recover`. Asserts every
    /// intent reaches a terminal state.
    pub fn recover(&self) -> anyhow::Result<RecoveryReport> {
        let recover_trace = self.root.join("recover-trace.log");

        let output = std::process::Command::new(lim_toy_path())
            .args(["recover", self.root.as_str()])
            .env_remove("LIMINAL_CRASHPOINT")
            .env("LIMINAL_CRASH_TRACE", &recover_trace)
            .output()?;

        anyhow::ensure!(
            output.status.success(),
            "recover failed: status={:?}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );

        let line = String::from_utf8(output.stdout)?
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("recover produced no output"))?
            .to_owned();

        let outcome: liminal_daemon::runner::RecoverOutcome = serde_json::from_str(&line)?;

        // Assert every terminal is actually terminal.
        for state in &outcome.terminals {
            anyhow::ensure!(
                state.is_terminal(),
                "recovery bug: nonterminal state after recovery: {state:?}"
            );
        }

        Ok(RecoveryReport {
            terminals: outcome.terminals,
            world_digest: outcome.world_digest,
        })
    }

    /// The full derived crash matrix for one scenario: baseline → for every
    /// observed `(point, occurrence)`: crash, recover, assert terminal +
    /// no hidden half-state + recovery idempotence (world digests equal on a
    /// second recovery).
    pub fn crash_matrix(scenario: &ScenarioScript) -> anyhow::Result<()> {
        // 1. Baseline in its own root.
        let baseline_run = Self::new(&format!("baseline-{}", scenario.scenario.id))?;
        let trace = baseline_run.baseline(scenario)?;

        let expected = scenario.expect.terminal.as_deref().unwrap_or("committed");
        let normalize = |s: &str| s.to_lowercase().replace('-', "");
        let expected_norm = normalize(expected);

        // 2. For every (point, occurrence) in the trace → crash + recover.
        for (point, occurrence) in trace.enumerate_faults() {
            let label = format!(
                "{}-{}-{}",
                scenario.scenario.id,
                point.replace('/', "-"),
                occurrence
            );
            let run = Self::new(&label)?;

            // Crash.
            run.run_to_crash(scenario, &point, occurrence)?;

            // First recovery.
            let r1 = run.recover()?;
            for state in &r1.terminals {
                let actual_norm = normalize(&format!("{state:?}"));
                anyhow::ensure!(
                    actual_norm == expected_norm,
                    "crash matrix [{label}]: expected terminal {expected:?}, got {state:?}"
                );
            }

            // Second recovery — idempotence.
            let r2 = run.recover()?;
            anyhow::ensure!(
                r2.world_digest == r1.world_digest,
                "crash matrix [{label}]: recovery idempotence failed — digests differ"
            );
            anyhow::ensure!(
                r2.terminals == r1.terminals,
                "crash matrix [{label}]: recovery idempotence failed — terminals differ"
            );

            // If committed, verify file hashes match baseline.
            if expected_norm == "committed" {
                let baseline_hashes = workspace_file_hashes(&baseline_run.root)?;
                let recovered_hashes = workspace_file_hashes(&run.root)?;
                anyhow::ensure!(
                    baseline_hashes == recovered_hashes,
                    "crash matrix [{label}]: file world diverges from baseline"
                );
            }

            // No staged files remain.
            let staged = liminal_source::scan_staged(&run.root)?;
            anyhow::ensure!(
                staged.is_empty(),
                "crash matrix [{label}]: {} staged files remain after recovery",
                staged.len()
            );
        }

        Ok(())
    }

    /// Path of the workspace store.
    #[must_use]
    pub fn store_dir(&self) -> Utf8PathBuf {
        self.root.join("state")
    }

    /// Path of the injector's hit-trace file for this run.
    #[must_use]
    pub fn trace_path(&self) -> Utf8PathBuf {
        self.root.join("crash-trace.log")
    }
}

/// Assert the silence law (Law 3E; v4 §7.9): exit 0 AND byte-empty stdout AND
/// byte-empty stderr. "No errors printed" is NOT silence.
pub fn assert_silent(output: &Output) {
    assert!(
        output.status.success(),
        "sound state must exit 0, got {:?}",
        output.status
    );
    assert!(
        output.stdout.is_empty(),
        "sound state must write ZERO bytes to stdout, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "sound state must write ZERO bytes to stderr, got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The single source of truth for crash-matrix scenario scope.
/// M4 appends "dag_id_then_reattach".
pub fn runnable_crash_scenarios() -> anyhow::Result<Vec<ScenarioScript>> {
    const RUNNABLE: &[&str] = &["promote_single_step"];
    let dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/scenarios");
    let mut out = Vec::new();
    for id in RUNNABLE {
        let path = dir.join(format!("{id}.scenario.toml"));
        out.push(ScenarioScript::load(&path)?);
    }
    Ok(out)
}

/// Scenarios whose write paths cannot be driven end-to-end yet.
///
/// Shrunk by later milestones: M4 removes the repair scenarios, M5 removes the
/// offline one. Empty (and deleted) by M5. `dag_accept` (M04.8 authored) stays
/// here until the `accept_repair` runner step (AM-4.2) lands.
pub const NOT_YET_DRIVEN: &[&str] = &[
    // Need the foreign-change DAG planner (M04.3 ingest::foreign_change) and
    // the accept_repair step (AM-4.2), respectively.
    "dag_accept",
    "dag_id_then_reattach",
    // Needs the offline-Holder / Overlay path (M5).
    "offline_holder_overlay",
];

/// Load every authored scenario fixture, sorted by id.
pub fn all_scenarios() -> anyhow::Result<Vec<ScenarioScript>> {
    let dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/scenarios");
    ScenarioScript::load_dir(&dir)
}

/// Verify a locked held-out corpus directory against its BLAKE3 manifest
/// (`MANIFEST.b3`: `<hex>  <relative-path>` per line, sorted by path).
/// Any drift is a process violation (v4 §7.4: locked corpora are never tuned
/// against; changes create a new version).
pub fn verify_heldout_manifest(version_dir: &Utf8Path) -> anyhow::Result<()> {
    let manifest_path = version_dir.join("MANIFEST.b3");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{manifest_path}: locked corpus has no manifest: {e}"))?;

    let mut expected = BTreeMap::new();
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, rel) = line
            .split_once("  ")
            .ok_or_else(|| anyhow::anyhow!("{manifest_path}: bad manifest line: {line:?}"))?;
        expected.insert(rel.trim().to_owned(), hash.trim().to_owned());
    }

    let mut actual = BTreeMap::new();
    collect_hashes(version_dir, version_dir, &mut actual)?;
    actual.remove("MANIFEST.b3");

    anyhow::ensure!(
        expected == actual,
        "held-out corpus drift at {version_dir}: manifest and tree disagree \
         (expected {} entries, found {}). Locked corpora are immutable — create \
         a new version instead (v4 §7.4).",
        expected.len(),
        actual.len(),
    );
    Ok(())
}

fn collect_hashes(
    root: &Utf8Path,
    dir: &Utf8Path,
    out: &mut BTreeMap<String, String>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path.is_dir() {
            collect_hashes(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| anyhow::anyhow!("path escape"))?
                .to_string();
            let bytes = std::fs::read(&path)?;
            out.insert(rel, ContentHash::of(&bytes).to_hex());
        }
    }
    Ok(())
}
