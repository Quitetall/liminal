//! Workspace maintenance binary.

use anyhow::Result;
use clap::{Parser, Subcommand};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Haq { command } => match command {
            HaqCommand::Verify => {
                liminal_xtask::haq::verify_qualified_repo(&liminal_xtask::repo_root()?)?;
            }
            HaqCommand::VerifyInventory => {
                liminal_xtask::haq::verify_inventory_repo(&liminal_xtask::repo_root()?)?;
            }
            HaqCommand::RunCanaries => {
                liminal_xtask::haq::run_canaries_repo(&liminal_xtask::repo_root()?)?;
            }
            HaqCommand::Generate { cases } => {
                liminal_xtask::haq::run_generated_repo(&liminal_xtask::repo_root()?, cases)?;
            }
        },
        Command::Bench { command } => match command {
            BenchCommand::Sample { count } => {
                liminal_xtask::bench::sample_repo(&liminal_xtask::repo_root()?, count)?;
            }
            BenchCommand::BaselineCheck => {
                liminal_xtask::bench::baseline_check_repo(&liminal_xtask::repo_root()?)?;
            }
            BenchCommand::Gate => {
                liminal_xtask::bench::gate_repo(&liminal_xtask::repo_root()?)?;
            }
        },
    }
    Ok(())
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// HAQP-1 qualification helpers.
    Haq {
        #[command(subcommand)]
        command: HaqCommand,
    },
    /// Phase 1 benchmark evidence helpers.
    Bench {
        #[command(subcommand)]
        command: BenchCommand,
    },
}

#[derive(Debug, Subcommand)]
enum HaqCommand {
    /// Verify completed HAQP qualification evidence.
    Verify,
    /// Verify committed HAQP inventories and packet status before qualification.
    VerifyInventory,
    /// Execute every disposable canary against in-memory packet mutations.
    RunCanaries,
    /// Run deterministic generated evidence for all five HAQP families.
    Generate {
        /// Accepted cases per family (qualification uses 100000).
        #[arg(long, default_value_t = 100_000)]
        cases: u64,
    },
}

#[derive(Debug, Subcommand)]
enum BenchCommand {
    /// Capture exactly 30 raw samples for each frozen benchmark.
    Sample {
        #[arg(default_value_t = 30)]
        count: usize,
    },
    /// Validate committed baseline schema and derived statistics.
    BaselineCheck,
    /// Compare candidate against accepted baseline.
    Gate,
}
