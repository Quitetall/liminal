//! M07 — Killer #4: the persistent-identity torture corpus (v4 §-1.1, §19;
//! Law 12/13).
//!
//! Six identity strategies × eleven operations × eight seeds → the identity
//! guarantee matrix. The matrix empirically caps every identity claim: no
//! strategy may claim stronger continuity than the corpus demonstrates
//! (v4 §-1.1, §19.2 — "no magical round-trip guarantee"). Git operations use
//! the REAL `git` CLI in hermetic temp repos (D07.6), never git2/gitoxide:
//! merge-ort is what users' files actually pass through.
//!
//! Layout (D07.1): `strategy.rs` (outcomes + the six observers), `ops.rs` (the
//! eleven seeded mutations), `gitenv.rs` (hermetic git runbook), `matrix.rs`
//! (renderer), `claims.rs` (`claims_never_exceed_evidence`).

pub mod claims;
pub mod gitenv;
pub mod matrix;
pub mod ops;
pub mod strategy;

use camino::Utf8PathBuf;

pub use ops::Operation;
pub use strategy::{Anchor, Outcome, Strategy};

/// The corpus configuration, parsed from `fixtures/identity/config.toml`. The
/// exact bytes of that file are hashed into the golden header (`config:
/// blake3:<hex>`), so any config drift is a visible golden change.
#[derive(Debug, Clone)]
pub struct Config {
    /// Deterministic seeds (D07.4). Every random choice derives from these.
    pub seeds: Vec<u64>,
    /// Minimum acceptable `git` version (D07.6). Below this the run FAILS.
    pub min_git: String,
    /// The base document template (one template, id `notes`, in Phase -1).
    pub template: String,
    /// Strategy columns, in render order.
    pub strategies: Vec<Strategy>,
    /// N for the structural n-gram sets (D07.3).
    pub ngram: usize,
    /// Jaccard acceptance threshold for structural recovery (D07.3).
    pub jaccard_threshold: f64,
    /// Operation rows, in render order.
    pub operations: Vec<Operation>,
    /// Git conflict-resolution policies exercised (seeded choice).
    pub conflict_policies: Vec<gitenv::ConflictPolicy>,
    /// Probability that branch/main edits overlap on the same block.
    pub overlap_probability: f64,
    /// The raw config bytes, retained for the golden `config:` hash line.
    pub raw_bytes: Vec<u8>,
}

impl Config {
    /// Load and validate `fixtures/identity/config.toml`. Fails loudly on any
    /// unknown strategy/operation/policy name — the corpus never silently drops
    /// a column or row (protocol §3).
    ///
    /// # Errors
    /// Returns an error if the fixture is missing, malformed, or names an
    /// unknown strategy/operation/policy.
    pub fn load() -> anyhow::Result<Self> {
        let path = fixture_path();
        let raw_bytes = std::fs::read(&path).map_err(|e| anyhow::anyhow!("read {path}: {e}"))?;
        Self::from_bytes(&raw_bytes)
    }

    /// Parse config from raw TOML bytes (kept separate from [`Self::load`] so
    /// the `config:` hash covers exactly the bytes parsed).
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn from_bytes(raw_bytes: &[u8]) -> anyhow::Result<Self> {
        let doc: toml::Value = toml::from_str(std::str::from_utf8(raw_bytes)?)?;

        let corpus = doc
            .get("corpus")
            .ok_or_else(|| anyhow::anyhow!("[corpus] missing"))?;
        let seeds = corpus
            .get("seeds")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("corpus.seeds missing"))?
            .iter()
            .map(|v| {
                let i = v
                    .as_integer()
                    .ok_or_else(|| anyhow::anyhow!("seed not an integer"))?;
                u64::try_from(i).map_err(|_| anyhow::anyhow!("seed out of range: {i}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let min_git = corpus
            .get("min_git")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("corpus.min_git missing"))?
            .to_owned();

        let template = doc
            .get("template")
            .and_then(toml::Value::as_array)
            .and_then(|a| a.first())
            .and_then(|t| t.get("text"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("[[template]].text missing"))?
            .to_owned();

        let strategies = doc
            .get("strategies")
            .and_then(|s| s.get("enabled"))
            .and_then(toml::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("strategies.enabled missing"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| anyhow::anyhow!("strategy not a string"))
                    .and_then(Strategy::from_kebab)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let structural = doc.get("structural");
        let ngram = usize::try_from(
            structural
                .and_then(|s| s.get("ngram"))
                .and_then(toml::Value::as_integer)
                .ok_or_else(|| anyhow::anyhow!("structural.ngram missing"))?,
        )
        .map_err(|_| anyhow::anyhow!("structural.ngram out of range"))?;
        let jaccard_threshold = structural
            .and_then(|s| s.get("jaccard_threshold"))
            .and_then(toml::Value::as_float)
            .ok_or_else(|| anyhow::anyhow!("structural.jaccard_threshold missing"))?;

        let operations = doc
            .get("operations")
            .and_then(|o| o.get("enabled"))
            .and_then(toml::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("operations.enabled missing"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| anyhow::anyhow!("operation not a string"))
                    .and_then(Operation::from_name)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let git = doc.get("operations").and_then(|o| o.get("git"));
        let conflict_policies = git
            .and_then(|g| g.get("conflict_policies"))
            .and_then(toml::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("operations.git.conflict_policies missing"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| anyhow::anyhow!("policy not a string"))
                    .and_then(gitenv::ConflictPolicy::from_name)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let overlap_probability = git
            .and_then(|g| g.get("overlap_probability"))
            .and_then(toml::Value::as_float)
            .ok_or_else(|| anyhow::anyhow!("operations.git.overlap_probability missing"))?;

        Ok(Self {
            seeds,
            min_git,
            template,
            strategies,
            ngram,
            jaccard_threshold,
            operations,
            conflict_policies,
            overlap_probability,
            raw_bytes: raw_bytes.to_vec(),
        })
    }
}

/// Absolute path to the corpus config fixture.
#[must_use]
pub fn fixture_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/identity/config.toml")
}

/// D07.2 normalization, shared by sidecar / content-hash / structural: per line
/// `trim`, collapse internal whitespace runs to a single space, join with
/// `"\n"`, then drop leading/trailing empty lines. Deterministic and total.
#[must_use]
pub fn normalize(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    // Drop leading/trailing empty lines.
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

/// blake3 hex of `normalize(text)` — the sidecar / content-hash key (D07.2).
#[must_use]
pub fn content_hash_hex(text: &str) -> String {
    blake3::hash(normalize(text).as_bytes())
        .to_hex()
        .to_string()
}
