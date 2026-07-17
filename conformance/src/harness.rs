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

use std::process::Output;

use camino::{Utf8Path, Utf8PathBuf};
use liminal_jurisdiction::IntentState;

use crate::scenario::ScenarioScript;

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

    /// Sound run: spawn `liminald exec <scenario>` with NO fault armed;
    /// return the ordered hit trace (the derived crash matrix).
    pub fn baseline(&self, scenario: &ScenarioScript) -> anyhow::Result<HitTrace> {
        let _ = scenario;
        anyhow::bail!("Phase -1 M2: baseline subprocess run (docs/implementation-plan.md)")
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
        let _ = (scenario, point, occurrence);
        anyhow::bail!("Phase -1 M2: crash subprocess run (docs/implementation-plan.md)")
    }

    /// Fresh subprocess, same store dir: `liminald recover`. Asserts every
    /// intent reaches a terminal state.
    pub fn recover(&self) -> anyhow::Result<RecoveryReport> {
        anyhow::bail!("Phase -1 M2: recovery subprocess run (docs/implementation-plan.md)")
    }

    /// The full derived crash matrix for one scenario: baseline → for every
    /// observed `(point, occurrence)`: crash, recover, assert terminal +
    /// no hidden half-state + recovery idempotence (world digests equal on a
    /// second recovery).
    pub fn crash_matrix(scenario: &ScenarioScript) -> anyhow::Result<()> {
        let _ = scenario;
        anyhow::bail!("Phase -1 M2: derived crash matrix (docs/implementation-plan.md)")
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

/// Verify a locked held-out corpus directory against its BLAKE3 manifest
/// (`MANIFEST.b3`: `<hex>  <relative-path>` per line, sorted by path).
/// Any drift is a process violation (v4 §7.4: locked corpora are never tuned
/// against; changes create a new version).
pub fn verify_heldout_manifest(version_dir: &Utf8Path) -> anyhow::Result<()> {
    let manifest_path = version_dir.join("MANIFEST.b3");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{manifest_path}: locked corpus has no manifest: {e}"))?;

    let mut expected = std::collections::BTreeMap::new();
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, rel) = line
            .split_once("  ")
            .ok_or_else(|| anyhow::anyhow!("{manifest_path}: bad manifest line: {line:?}"))?;
        expected.insert(rel.trim().to_owned(), hash.trim().to_owned());
    }

    let mut actual = std::collections::BTreeMap::new();
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
    out: &mut std::collections::BTreeMap<String, String>,
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
            out.insert(rel, liminal_id::ContentHash::of(&bytes).to_hex());
        }
    }
    Ok(())
}
