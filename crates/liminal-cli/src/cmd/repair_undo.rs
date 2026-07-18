//! `lim repair undo <repair-id>` (R4 §6): one-command revert. Undo against
//! changed state becomes another Basis-checked RepairPlan — never stale bytes.

use std::io::Write as _;
use std::process::ExitCode;

use camino::Utf8Path;
use liminal_daemon::runner::{self, UndoOutcome};

/// Revert an accepted repair. Exit 0 on undo or queued-for-review; exit 1 on
/// unknown id or no recorded inverse (M04 Algorithm D CLI flow).
pub(crate) fn run(workspace: &Utf8Path, repair_id: &str) -> ExitCode {
    match runner::undo(workspace, repair_id) {
        Ok(UndoOutcome::Undone { repair }) => {
            println!("undone: {repair}");
            ExitCode::SUCCESS
        }
        Ok(UndoOutcome::QueuedForReview { repair, detail }) => {
            println!("undo queued for review: {repair} ({detail})");
            ExitCode::SUCCESS
        }
        Ok(UndoOutcome::NoInverse) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "repair:{repair_id} recorded no inverse");
            ExitCode::from(1)
        }
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{e}");
            ExitCode::from(1)
        }
    }
}
