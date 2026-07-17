//! `lim overlays` (v4 §7.10.8; R4 §9): reconciliation debt — count, age,
//! affected domains, repair blockers. Uses everyday vocabulary ("draft",
//! "pending sync", "review needed"), never internal terms (R4 §3).

use camino::Utf8Path;

/// List reconciliation debt. Zero debt → zero output (Law 3E).
pub(crate) fn run(workspace: &Utf8Path, all: bool) -> anyhow::Result<()> {
    let _ = (workspace, all);
    anyhow::bail!("Phase -1 M5: lim overlays is not implemented yet (docs/implementation-plan.md)")
}
