//! `lim jurisdiction explain <subject>` (v4 §7.9; R4 §3): the ONLY surface
//! where the internal vocabulary (Jurisdiction, Holder, Overlay, Promotion)
//! appears.

use camino::Utf8Path;

/// Explain one subject's governance in full.
pub(crate) fn explain(workspace: &Utf8Path, subject: &str) -> anyhow::Result<()> {
    // Parse early so address errors are reported identically pre- and
    // post-implementation.
    let parsed: Result<liminal_id::JurisdictionSubject, _> = subject.parse();
    let _ = (workspace, parsed);
    anyhow::bail!(
        "Phase -1 M3: lim jurisdiction explain is not implemented yet \
         (docs/implementation-plan.md)"
    )
}
