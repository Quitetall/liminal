//! `lim` — the Liminal CLI (v4 §7.9–7.10, §20; R4 §3, §9).
//!
//! Output discipline (conformance-asserted):
//! - A sound workspace produces exit 0 and ZERO bytes of output — no summary
//!   line, no badge (Law 3E; v4 §7.9).
//! - Everyday vocabulary (draft, pending sync, review needed) everywhere
//!   except `lim jurisdiction explain`, the only surface where Jurisdiction /
//!   Holder / Overlay / Promotion may appear (R4 §3).

mod cmd;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lim",
    version,
    about = "Liminal workspace tool (Phase -1 toy surface)"
)]
struct Lim {
    /// Workspace root.
    #[arg(long, global = true, default_value = ".")]
    workspace: Utf8PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Interpretive Jurisdiction checker. Sound workspace: exit 0, no output.
    Check,
    /// Reconciliation debt: count, age, affected subjects, blockers.
    Overlays {
        /// Include archived (searchable) overlays.
        #[arg(long)]
        all: bool,
    },
    /// Repair records and nonterminal ILRP intents.
    Repairs,
    /// Repair operations.
    Repair {
        #[command(subcommand)]
        cmd: RepairCmd,
    },
    /// Advanced Jurisdiction inspection (the internal-vocabulary ceiling).
    Jurisdiction {
        #[command(subcommand)]
        cmd: JurisdictionCmd,
    },
}

#[derive(Subcommand)]
enum RepairCmd {
    /// One-command revert of an accepted repair. If later edits made direct
    /// reversal unsafe, this becomes a new reviewable RepairPlan — never a
    /// stale-byte overwrite (R4 §6).
    Undo {
        /// The repair id (`repair:<uuid>`).
        repair_id: String,
    },
}

#[derive(Subcommand)]
enum JurisdictionCmd {
    /// Explain which Holder governs a subject, under which Contract, at the
    /// current Basis (R4 §3: the ONLY place internal vocabulary surfaces).
    Explain {
        /// Subject address: `node:<uuid>`, `relation:<uuid>`, or a path.
        subject: String,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Lim::parse();
    match args.cmd {
        Cmd::Check => cmd::check::run(&args.workspace),
        Cmd::Overlays { all } => cmd::overlays::run(&args.workspace, all),
        Cmd::Repairs => cmd::repairs::run(&args.workspace),
        Cmd::Repair {
            cmd: RepairCmd::Undo { repair_id },
        } => cmd::repair_undo::run(&args.workspace, &repair_id),
        Cmd::Jurisdiction {
            cmd: JurisdictionCmd::Explain { subject },
        } => cmd::jurisdiction::explain(&args.workspace, &subject),
    }
}
