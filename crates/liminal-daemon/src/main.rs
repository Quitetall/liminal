//! `liminald` — the killable Phase -1 scenario runner (R4 §10).
//!
//! Not a daemon yet: `exec` runs one scripted scenario and exits; `recover`
//! opens the workspace (which runs ILRP recovery) and prints terminal intent
//! states. The conformance harness spawns this binary, arms
//! `LIMINAL_CRASHPOINT`, and asserts on the world it leaves behind.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "liminald",
    version,
    about = "Liminal Phase -1 toy scenario runner"
)]
struct Liminald {
    /// Workspace root (state lives at `<root>/state/`).
    #[arg(long, global = true, default_value = ".")]
    workspace: Utf8PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run one scripted scenario to completion (or to an armed crash point).
    Exec {
        /// Path to a *.scenario.toml fixture (see conformance/fixtures/scenarios/).
        scenario: Utf8PathBuf,
    },
    /// Open the workspace, run ILRP recovery, and print each nonterminal
    /// intent's terminal outcome (Committed / NeedsReview / Aborted).
    Recover,
}

fn main() -> anyhow::Result<()> {
    let args = Liminald::parse();
    match args.cmd {
        Cmd::Exec { scenario } => {
            let _ = scenario;
            anyhow::bail!(
                "Phase -1 M2: scenario execution is not implemented yet \
                 (docs/implementation-plan.md)"
            );
        }
        Cmd::Recover => {
            anyhow::bail!(
                "Phase -1 M2: recovery is not implemented yet \
                 (docs/implementation-plan.md)"
            );
        }
    }
}
