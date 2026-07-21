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
}

#[derive(Debug, Subcommand)]
enum HaqCommand {
    /// Verify completed HAQP qualification evidence.
    Verify,
    /// Verify committed HAQP inventories and packet status before qualification.
    VerifyInventory,
}
