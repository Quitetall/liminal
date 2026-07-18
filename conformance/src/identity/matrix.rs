//! The identity guarantee matrix: drive every (strategy, operation, seed),
//! aggregate to the worst outcome per cell (D07.5), and render the golden
//! `conformance/golden/identity_matrix.md` (D07 §D).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::identity::ops::{self, ManagedObservation};
use crate::identity::strategy::{self, BaseWorld, Outcome};
use crate::identity::{Config, Operation, Strategy};

/// One matrix cell: the worst demonstrated outcome across all seeds, plus the
/// post-op file that WITNESSED that worst outcome (the checker-replay evidence
/// for `claims_never_exceed_evidence`, assertion 3).
#[derive(Debug, Clone)]
pub struct Cell {
    /// The worst outcome across seeds (D07.5).
    pub outcome: Outcome,
    /// The post-op file bytes of the seed that produced `outcome`.
    pub witness_file: String,
    /// Whether the operation is foreign to the graph Holder (governs the
    /// ` [foreign]` suffix on managed-graph cells).
    pub foreign: bool,
}

/// The full computed matrix, keyed by `(operation index, strategy index)` in
/// the config's declared render order.
#[derive(Debug, Clone)]
pub struct Matrix {
    /// The exact `git --version` line at generation time.
    pub git_version: String,
    /// blake3 hex of the config.toml bytes.
    pub config_hash: String,
    /// Seed count.
    pub seeds: usize,
    /// Operations (rows) in render order.
    pub operations: Vec<Operation>,
    /// Strategies (columns) in render order.
    pub strategies: Vec<Strategy>,
    /// `cells[(op_idx, strat_idx)]`.
    pub cells: BTreeMap<(usize, usize), Cell>,
}

impl Matrix {
    /// Build the matrix by running the whole corpus (D07 §D). Asserts the git
    /// floor first (D07.6) and records the exact version line.
    ///
    /// # Errors
    /// A below-floor `git`, or any operation/runbook failure.
    pub fn build(cfg: &Config) -> anyhow::Result<Matrix> {
        let git_version = crate::identity::gitenv::assert_min_git(&cfg.min_git)?;
        let config_hash = blake3::hash(&cfg.raw_bytes).to_hex().to_string();
        let world = BaseWorld::build(&cfg.template);

        let mut cells = BTreeMap::new();
        for (op_idx, &op) in cfg.operations.iter().enumerate() {
            let foreign = op.managed_observation() == ManagedObservation::Foreign;
            // Per (op, seed) compute the shared post-op world once, then observe
            // every strategy against it. Track, per strategy, the worst outcome
            // seen and the file that witnessed it.
            let mut best: BTreeMap<usize, Cell> = BTreeMap::new();
            for &seed in &cfg.seeds {
                let post = ops::apply(op, &world, seed, cfg)?;
                for (strat_idx, &strat) in cfg.strategies.iter().enumerate() {
                    let tracked = strategy::tracked_blocks(strat, &world);
                    let case = strategy::worst(
                        tracked
                            .iter()
                            .map(|&i| strategy::observe(strat, &world.blocks[i], &post, cfg)),
                    );
                    let candidate = Cell {
                        outcome: case.clone(),
                        witness_file: post.file.clone(),
                        foreign,
                    };
                    best.entry(strat_idx)
                        .and_modify(|c| {
                            if candidate.outcome < c.outcome {
                                *c = candidate.clone();
                            }
                        })
                        .or_insert(candidate);
                }
            }
            for (strat_idx, cell) in best {
                cells.insert((op_idx, strat_idx), cell);
            }
        }

        Ok(Matrix {
            git_version,
            config_hash,
            seeds: cfg.seeds.len(),
            operations: cfg.operations.clone(),
            strategies: cfg.strategies.clone(),
            cells,
        })
    }

    /// A cell's rendered string, with the ` [foreign]` suffix on managed-graph
    /// cells over foreign operations (D07 §D).
    #[must_use]
    pub fn cell_text(&self, op_idx: usize, strat_idx: usize) -> String {
        let cell = &self.cells[&(op_idx, strat_idx)];
        let mut text = cell.outcome.render();
        if self.strategies[strat_idx] == Strategy::ManagedGraph && cell.foreign {
            text.push_str(" [foreign]");
        }
        text
    }

    /// Render the full golden markdown (D07 §D). The `git:` line carries the
    /// exact version; [`redact_git_line`] normalizes it for cross-machine test
    /// comparison.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Identity guarantee matrix (Phase -1.1, M7)");
        out.push('\n');
        let _ = writeln!(out, "git: {}", self.git_version);
        let _ = writeln!(out, "config: blake3:{}", self.config_hash);
        let _ = writeln!(out, "seeds: {}", self.seeds);
        out.push('\n');

        // Header row.
        out.push_str("| operation |");
        for strat in &self.strategies {
            let _ = write!(out, " {} |", strat.kebab());
        }
        out.push('\n');
        out.push('|');
        for _ in 0..=self.strategies.len() {
            out.push_str("---|");
        }
        out.push('\n');

        for (op_idx, op) in self.operations.iter().enumerate() {
            let _ = write!(out, "| {} |", op.name());
            for strat_idx in 0..self.strategies.len() {
                let _ = write!(out, " {} |", self.cell_text(op_idx, strat_idx));
            }
            out.push('\n');
        }
        out
    }
}

/// Normalize the `git:` header line to `git: git version [pinned]` so the
/// matrix comparison is stable across machines while the committed golden keeps
/// the real version as a published claim. The `≥ 2.44` floor is enforced
/// separately (D07.6).
#[must_use]
pub fn redact_git_line(markdown: &str) -> String {
    markdown
        .lines()
        .map(|l| {
            if l.starts_with("git: ") {
                "git: git version [pinned]".to_owned()
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
