//! `lim jurisdiction explain` — the ONLY surface where the internal
//! vocabulary (Jurisdiction, Holder, Overlay, Promotion) appears (R4 §3).
//! Routine UI says draft / save / sync / accept / source / conflict instead;
//! the four terms are a ceiling for explanations, not a required lexicon.

use liminal_id::{JurisdictionSubject, OverlayId};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

use crate::checker::{Checker, CheckerError};
use crate::contract::JurisdictionContract;
use crate::holder::Holder;
use crate::profile::grade_of;

/// A full explanation of one subject's governance, for explicit inspection.
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The subject.
    pub subject: JurisdictionSubject,
    /// Its resolved Holder at the Basis.
    pub holder: Holder,
    /// The write route.
    pub write_route: Holder,
    /// The governing Contract (surfaced only here and at boundaries, v4 §7.4).
    pub contract: JurisdictionContract,
    /// The governing perspective.
    pub perspective: BasisPerspective,
    /// Unresolved Overlays targeting the subject.
    pub overlays: Vec<OverlayId>,
    /// Human-readable narrative assembled from the above.
    pub narrative: String,
}

/// Render a Holder in the internal-vocabulary form used by `explain`.
#[must_use]
pub fn render_holder(holder: &Holder) -> String {
    match holder {
        Holder::File { path } => format!("file:{path}"),
        Holder::Buffer { client, buffer } => format!("buffer:{client}/{buffer}"),
        Holder::Graph => "graph".to_owned(),
        Holder::Object { hash } => format!("object:{hash}"),
        Holder::GitObject { oid } => format!("git:{}", oid.0),
        Holder::Service { source } => format!("service:{source}"),
    }
}

/// Render a perspective in kebab-case for display.
fn render_perspective(perspective: &BasisPerspective) -> String {
    match perspective {
        BasisPerspective::DurableOnly => "durable-only".to_owned(),
        BasisPerspective::ClientScoped { client } => format!("client-scoped ({client})"),
        BasisPerspective::Published { .. } => "published".to_owned(),
        BasisPerspective::Federated { .. } => "federated".to_owned(),
    }
}

/// Explain one subject (backs `lim jurisdiction explain <subject>`).
pub fn explain(
    checker: &Checker<'_>,
    subject: JurisdictionSubject,
    basis: &WorkspaceBasis,
) -> Result<Explanation, CheckerError> {
    let contract = checker.contract_for(subject)?;
    let holder = checker.resolve_holder(subject, basis)?;
    let write_route = checker.write_route(subject, basis)?;
    let perspective = checker.governing_perspective(basis);
    let overlays = checker.unresolved_overlays(Some(subject))?;

    let grade = grade_of(subject, checker.store);
    let required = contract.continuity.required_identity;
    let merge = contract
        .resolution
        .merge_runtime
        .as_ref()
        .map_or_else(|| "none".to_owned(), |m| m.0.clone());
    let overlays_str = if overlays.is_empty() {
        "none".to_owned()
    } else {
        overlays
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };

    let narrative = format!(
        "Subject {subject} is governed by the {} Holder as its write route \
         (perspective {}). Its identity grade is {grade:?} against a required \
         floor of {required:?}. Merge runtime: {merge}. Overlays: {overlays_str}. \
         This Jurisdiction, Holder, and any Overlay/Promotion vocabulary appears \
         only in this explanation surface (R4 §3).",
        render_holder(&write_route),
        render_perspective(&perspective),
    );

    Ok(Explanation {
        subject,
        holder,
        write_route,
        contract,
        perspective,
        overlays,
        narrative,
    })
}
