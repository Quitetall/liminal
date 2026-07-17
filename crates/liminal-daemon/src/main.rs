//! `liminald` — the killable Phase -1 scenario runner (R4 §10).
//!
//! Not a daemon yet: `exec` runs one scripted scenario and exits; `recover`
//! opens the workspace (which runs ILRP recovery) and prints terminal intent
//! states. The conformance harness spawns this binary, arms
//! `LIMINAL_CRASHPOINT`, and asserts on the world it leaves behind.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use liminal_jurisdiction::{CrashInjector as _, CrashPoint};

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

    /// Hidden M1 scaffold: fire a named crash point `count` times through the
    /// real env-armed injector (proves SIGABRT death + durable hit tracing).
    #[command(name = "__fire", hide = true)]
    Fire {
        point: String,
        #[arg(default_value_t = 1)]
        count: u64,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Liminald::parse();
    match args.cmd {
        Cmd::Exec { scenario } => {
            let script = liminal_daemon::scenario::ScenarioScript::load(&scenario)?;
            let root = &args.workspace;
            std::fs::create_dir_all(root)?;
            let executor = liminal_daemon::FsExecutor::new(root.clone());
            let crash = liminal_daemon::EnvCrashInjector::from_env();
            liminal_daemon::runner::exec(root, &script, executor, crash)?;
            Ok(())
        }
        Cmd::Recover => {
            let root = &args.workspace;
            let outcome = liminal_daemon::runner::recover(root)?;
            let json = serde_json::to_string(&outcome)?;
            println!("{json}");
            Ok(())
        }
        Cmd::Fire { point, count } => {
            let crash_point = CrashPoint::all()
                .into_iter()
                .find(|p| p.name() == point)
                .ok_or_else(|| anyhow::anyhow!("unknown crash point: {point}"))?;
            let injector = liminal_daemon::crash::EnvCrashInjector::from_env();
            for _ in 0..count {
                injector.crash_if_armed(crash_point);
            }
            Ok(())
        }
    }
}
