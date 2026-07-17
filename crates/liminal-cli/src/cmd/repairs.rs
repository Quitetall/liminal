//! `lim repairs` (R4 §6, §9): recorded repair decisions and nonterminal ILRP
//! intents.

use camino::Utf8Path;

/// List repair records and pending intents. Nothing pending → zero output.
pub(crate) fn run(workspace: &Utf8Path) -> anyhow::Result<()> {
    let _ = workspace;
    anyhow::bail!("Phase -1 M4: lim repairs is not implemented yet (docs/implementation-plan.md)")
}
