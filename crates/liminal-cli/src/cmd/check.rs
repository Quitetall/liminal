//! `lim check` (v4 §7.9): sound workspace → exit 0, no output, no badge.

use std::io::Write as _;
use std::process::ExitCode;

use camino::Utf8Path;
use liminal_daemon::ToyWorkspace;
use liminal_jurisdiction::everyday_rendering;
use liminal_revision::{BasisPerspective, WorkspaceBasis};

/// Run the interpretive checker. On a sound workspace this function prints
/// NOTHING and exits 0 — asserted byte-exactly by the conformance suite
/// (`sound_states_are_silent`).
///
/// Unsound → exit 1; one line per finding sorted by `(code, subject)`:
/// `"{everyday}: {subject} [{code}]\n"`; stderr empty. Checker error → exit 2.
pub(crate) fn run(workspace: &Utf8Path) -> ExitCode {
    match run_inner(workspace) {
        Ok(code) => code,
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{e}");
            ExitCode::from(2)
        }
    }
}

fn run_inner(workspace: &Utf8Path) -> anyhow::Result<ExitCode> {
    let ws = ToyWorkspace::open(workspace)?;
    let checker = ws.checker();

    // Phase -1: check against a DurableOnly Basis (M6 wires real capture).
    let basis = WorkspaceBasis {
        transaction: liminal_id::TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: std::collections::BTreeMap::new(),
    };

    let report = checker.check_workspace(&basis)?;
    if report.is_sound() {
        // Byte-silent: no stdout, no stderr, exit 0 (Law 3E).
        return Ok(ExitCode::SUCCESS);
    }

    let mut stdout = std::io::stdout().lock();
    for finding in &report.findings {
        // Render subject as its alias if one exists, else its id.
        let subject = ws
            .alias_for(finding.subject)
            .unwrap_or_else(|| finding.subject.to_string());
        writeln!(
            stdout,
            "{}: {subject} [{}]",
            everyday_rendering(finding.code),
            finding.code
        )?;
    }
    Ok(ExitCode::from(1))
}
