//! `lim check` (v4 §7.9): sound workspace → exit 0, no output, no badge.

use camino::Utf8Path;

/// Run the interpretive checker. On a sound workspace this function prints
/// NOTHING and exits 0 — asserted byte-exactly by the conformance suite
/// (`sound_states_are_silent`).
pub(crate) fn run(workspace: &Utf8Path) -> anyhow::Result<()> {
    let _ = workspace;
    anyhow::bail!("Phase -1 M3: lim check is not implemented yet (docs/implementation-plan.md)")
}
