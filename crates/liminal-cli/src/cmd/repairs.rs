//! `lim repairs` (R4 §6, §9): recorded repair decisions and nonterminal ILRP
//! intents.

use std::io::Write as _;
use std::process::ExitCode;

use camino::Utf8Path;

/// List repair records and pending intents. Nothing pending → zero output
/// (Law 3E). Exit 0 normally, 2 on error.
pub(crate) fn run(workspace: &Utf8Path) -> ExitCode {
    match liminal_daemon::runner::repairs_lines(workspace) {
        Ok(lines) => {
            let mut stdout = std::io::stdout().lock();
            for line in &lines {
                let _ = writeln!(stdout, "{line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{e}");
            ExitCode::from(2)
        }
    }
}
