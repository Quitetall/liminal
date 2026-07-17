//! The interpretive Jurisdiction Checker (v4 §7.9) — the reference semantics.
//!
//! Normal behavior is silence: `sound workspace → exit 0, no output, no badge,
//! no notification`. The conformance suite asserts empty BYTE STRINGS, not
//! "no errors".
//!
//! This interpreter is the conformance oracle: compiled plans, indexed
//! dispatch, and generated policy code are forbidden until differential tests
//! prove an optimized path equivalent (v4 §125).

use liminal_graph::GraphStore;
use liminal_id::{JurisdictionSubject, NodeId, OverlayId};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

use crate::holder::{Holder, MergeRuntimeRef};
use crate::profile::ProfileSet;
use crate::repair::{RepairDecision, RepairPlan};

/// The interpretive checker. Answers the eight questions of v4 §7.9, verbatim,
/// as methods.
#[derive(Debug)]
pub struct Checker<'w> {
    /// The graph store.
    pub store: &'w GraphStore,
    /// The active profiles (exactly two in Phase -1).
    pub profiles: &'w ProfileSet,
}

impl Checker<'_> {
    /// Q1: Which Holder does this subject resolve to at this Basis?
    pub fn resolve_holder(
        &self,
        subject: JurisdictionSubject,
        basis: &WorkspaceBasis,
    ) -> Result<Holder, CheckerError> {
        let _ = (subject, basis);
        todo!("Phase -1 M3: holder resolution (v4 §7.9 Q1)")
    }

    /// Q2: Where would a write go?
    pub fn write_route(
        &self,
        subject: JurisdictionSubject,
        basis: &WorkspaceBasis,
    ) -> Result<Holder, CheckerError> {
        let _ = (subject, basis);
        todo!("Phase -1 M3: write routing (v4 §7.9 Q2)")
    }

    /// Q3: Is a merge runtime required?
    pub fn merge_runtime_required(
        &self,
        subject: JurisdictionSubject,
    ) -> Result<Option<MergeRuntimeRef>, CheckerError> {
        let _ = subject;
        todo!("Phase -1 M3: merge-runtime requirement (v4 §7.9 Q3)")
    }

    /// Q4: Does identity satisfy every incoming durable Relation?
    pub fn identity_satisfies_relations(
        &self,
        node: NodeId,
        basis: &WorkspaceBasis,
    ) -> Result<Vec<Finding>, CheckerError> {
        let _ = (node, basis);
        todo!("Phase -1 M3: identity-grade checking (v4 §7.9 Q4, §19.1)")
    }

    /// Q5: Are there unresolved Overlays (optionally scoped to a subject)?
    pub fn unresolved_overlays(
        &self,
        subject: Option<JurisdictionSubject>,
    ) -> Result<Vec<OverlayId>, CheckerError> {
        let _ = subject;
        todo!("Phase -1 M5: overlay query (v4 §7.9 Q5, §7.10)")
    }

    /// Q6: Which `BasisPerspective` governs this computation?
    #[must_use]
    pub fn governing_perspective(&self, basis: &WorkspaceBasis) -> BasisPerspective {
        basis.perspective.clone()
    }

    /// Q7: Can a proposed repair be applied without crossing another
    /// Jurisdiction illegally? Mutation-local conjunction (Law 3F): every
    /// mutated subject's Jurisdiction must authorize ITS OWN mutation; no
    /// Contract commandeers the other subject (v4 §7.7).
    pub fn authorize(&self, plan: &RepairPlan) -> Result<AuthorizationReport, CheckerError> {
        let _ = plan;
        todo!("Phase -1 M4: mutation-local conjunctive authorization (v4 §7.7, Law 3F)")
    }

    /// Q8: Is the repair deterministic, safe, ordered, recoverable, and
    /// reversible at the captured Basis? A unique result is necessary but not
    /// sufficient (Law 3G; R4 §6).
    pub fn evaluate_repair(&self, plan: &RepairPlan) -> Result<RepairDecision, CheckerError> {
        let _ = plan;
        todo!("Phase -1 M4: full repair evaluation (v4 §7.9 Q8, §7.7 conjunction)")
    }

    /// The whole-workspace check backing `lim check`. Sound workspace → empty
    /// report → exit 0, zero bytes of output (Law 3E).
    pub fn check_workspace(&self, basis: &WorkspaceBasis) -> Result<CheckReport, CheckerError> {
        let _ = basis;
        todo!("Phase -1 M3: workspace soundness check (v4 §7.9)")
    }
}

/// Per-mutation authorization outcome (Law 3F).
#[derive(Debug, Clone)]
pub struct AuthorizationReport {
    /// Findings, one per refused mutation; empty = fully authorized.
    pub refusals: Vec<Finding>,
}

/// The whole-workspace report. Silence is the normal state.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    /// All findings. Empty = sound = silent.
    pub findings: Vec<Finding>,
}

impl CheckReport {
    /// Sound ⇔ nothing to say (Law 3E).
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.findings.is_empty()
    }
}

/// One diagnostic finding. Internal codes exist for tests and maintainers;
/// ordinary UI reduces them to familiar situations (draft not saved,
/// conflicting changes, source unavailable, repair not applied — v4 §7.9).
#[derive(Debug, Clone)]
pub struct Finding {
    /// The affected subject.
    pub subject: JurisdictionSubject,
    /// Stable internal diagnostic code.
    pub code: &'static str,
    /// Maintainer-facing message.
    pub message: String,
}

/// Checker failure (distinct from findings: this is the checker itself
/// failing, not the workspace being unsound).
#[derive(Debug, thiserror::Error)]
pub enum CheckerError {
    /// Store access failed.
    #[error(transparent)]
    Store(#[from] liminal_graph::StoreError),
    /// No profile governs the subject (a scaffold-phase impossibility that
    /// must become a Finding, never a panic, by M3).
    #[error("no profile governs subject {0}")]
    Ungoverned(JurisdictionSubject),
}
