//! `lim jurisdiction explain` — the ONLY surface where the internal
//! vocabulary (Jurisdiction, Holder, Overlay, Promotion) appears (R4 §3).
//! Routine UI says draft / save / sync / accept / source / conflict instead;
//! the four terms are a ceiling for explanations, not a required lexicon.

use liminal_id::{JurisdictionSubject, OverlayId};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

use crate::checker::{Checker, CheckerError};
use crate::contract::JurisdictionContract;
use crate::holder::Holder;

/// A full explanation of one subject's governance, for explicit inspection.
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The subject.
    pub subject: JurisdictionSubject,
    /// Its resolved Holder at the Basis.
    pub holder: Holder,
    /// The governing Contract (surfaced only here and at boundaries, v4 §7.4).
    pub contract: JurisdictionContract,
    /// The governing perspective.
    pub perspective: BasisPerspective,
    /// Unresolved Overlays targeting the subject.
    pub overlays: Vec<OverlayId>,
    /// Human-readable narrative assembled from the above.
    pub narrative: String,
}

/// Explain one subject (backs `lim jurisdiction explain <subject>`).
pub fn explain(
    checker: &Checker<'_>,
    subject: JurisdictionSubject,
    basis: &WorkspaceBasis,
) -> Result<Explanation, CheckerError> {
    let _ = (checker, subject, basis);
    todo!("Phase -1 M3: explanation assembly (v4 §7.9; R4 §3)")
}
