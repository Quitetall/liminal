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
            HaqCommand::ReviewUnresolved { record } => {
                println!(
                    "{}",
                    liminal_xtask::haq::effective_unresolved_findings_repo(
                        &liminal_xtask::repo_root()?,
                        &record
                    )?
                );
            }
            HaqCommand::SeedCorpus { target } => {
                liminal_xtask::haq::write_seed_corpus_repo(&liminal_xtask::repo_root()?, &target)?;
            }
            HaqCommand::Concurrency => {
                liminal_xtask::haq::run_concurrency_repo(&liminal_xtask::repo_root()?)?;
            }
            HaqCommand::ScopeDigest {
                kind,
                scope,
                traces,
            } => {
                println!(
                    "{}",
                    liminal_xtask::haq::scope_trace_digest_repo(
                        &liminal_xtask::repo_root()?,
                        &kind,
                        &traces,
                        &scope
                    )?
                );
            }
            HaqCommand::PacketDigest => {
                println!(
                    "{}",
                    liminal_xtask::haq::packet_digest_repo(&liminal_xtask::repo_root()?)?
                );
            }
            HaqCommand::Hash { path } => {
                let bytes = std::fs::read(&path)?;
                println!("{}", blake3::hash(&bytes).to_hex());
            }
            HaqCommand::Generate { cases } => {
                liminal_xtask::haq::run_generated_repo(&liminal_xtask::repo_root()?, cases)?;
            }
            HaqCommand::Mutants { ids, run_ignored } => {
                liminal_xtask::haq::run_mutants_repo(
                    &liminal_xtask::repo_root()?,
                    &ids,
                    run_ignored,
                )?;
            }
            HaqCommand::ScopeProbe { scope } => {
                liminal_xtask::haq::run_scope_probe_repo(&liminal_xtask::repo_root()?, &scope)?;
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
    /// BLAKE3 of one file, for the fuzz campaign's `log_blake3` (M17.5 F-17).
    ///
    /// The campaign shells out here rather than to `b3sum`, which is not
    /// installed on this machine and would be an undeclared build dependency of
    /// the evidence lane.
    Hash {
        /// File to digest.
        path: camino::Utf8PathBuf,
    },
    /// A review record's unresolved verified findings after standing signed
    /// rulings (grilling decision 6; M17.5 F-48).
    ReviewUnresolved {
        /// The record JSON.
        record: camino::Utf8PathBuf,
    },
    /// Write a fuzz target's classed seed set (ADR-0020 §4; M17.5 F-46).
    SeedCorpus {
        /// `graph_interchange_codec` or `ilrp_recovery`.
        target: String,
    },
    /// Re-attest the not-applicable concurrency declaration at this base.
    Concurrency,
    /// Digest a captured scope trace with the gate's own code (`paths` or `resolved`).
    ScopeDigest {
        /// `paths` or `resolved`.
        kind: String,
        /// The scope name, which decides whether corpus access is required.
        scope: String,
        /// The captured strace files, `.zst` accepted: the stage remainder and,
        /// for `fuzz`, every carved per-target trace (F-43). Digests are over
        /// the union.
        #[arg(required = true)]
        traces: Vec<camino::Utf8PathBuf>,
    },
    /// The packet digest the markdown surface must carry.
    ///
    /// The flip shells out here rather than recomputing it: the gate hashes
    /// `serde_json::to_vec(packet)`, which serializes in STRUCT field order,
    /// not the order the keys happen to sit in the file. A reimplementation
    /// that got that wrong would disagree only sometimes, which is worse than
    /// disagreeing always.
    PacketDigest,
    /// Run deterministic generated evidence for all five HAQP families.
    Generate {
        /// Accepted cases per family (qualification uses 100000).
        #[arg(long, default_value_t = 100_000)]
        cases: u64,
    },
    /// Run declared source patches in isolated git worktrees and record results.
    Mutants {
        /// Restrict run to one or more mutant IDs; default runs all declared rows.
        #[arg(long = "id")]
        ids: Vec<String>,
        /// Include tests marked `#[ignore]` in the runnable set.
        #[arg(long)]
        run_ignored: bool,
    },
    /// Read only the committed files assigned to one corpus-audit scope.
    ScopeProbe {
        /// Closed qualification scope name (for example `generated`).
        scope: String,
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
