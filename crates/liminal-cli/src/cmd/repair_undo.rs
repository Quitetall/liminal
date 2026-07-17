//! `lim repair undo <repair-id>` (R4 §6): one-command revert. Undo against
//! changed state becomes another Basis-checked RepairPlan — never stale bytes.

use camino::Utf8Path;

/// Revert an accepted repair.
pub(crate) fn run(workspace: &Utf8Path, repair_id: &str) -> anyhow::Result<()> {
    let _ = (workspace, repair_id);
    anyhow::bail!(
        "Phase -1 M4: lim repair undo is not implemented yet (docs/implementation-plan.md)"
    )
}
