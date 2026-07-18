//! The interpretive Jurisdiction Checker (v4 §7.9) — the reference semantics.
//!
//! Normal behavior is silence: `sound workspace → exit 0, no output, no badge,
//! no notification`. The conformance suite asserts empty BYTE STRINGS, not
//! "no errors".
//!
//! This interpreter is the conformance oracle: compiled plans, indexed
//! dispatch, and generated policy code are forbidden until differential tests
//! prove an optimized path equivalent (v4 §125).

use liminal_graph::ns::{JUR_ALIAS, JUR_OVERLAY};
use liminal_graph::{GraphStore, Relation};
use liminal_id::{JurisdictionSubject, NodeId, OverlayId};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};

use crate::holder::{Holder, MergeRuntimeRef};
use crate::overlay::OverlayState;
use crate::profile::{ProfileSet, grade_of};
use crate::repair::{RepairDecision, RepairPlan};

/// Finding codes (D — internal; UI renders everyday language, R4 §3).
pub mod codes {
    /// holder-unavailable — source unavailable or stale.
    pub const HOLDER_UNAVAILABLE: &str = "JUR010";
    /// write-route-unavailable — draft not yet saved or synced.
    pub const WRITE_ROUTE_UNAVAILABLE: &str = "JUR020";
    /// merge-runtime-missing — conflicting changes need review.
    pub const MERGE_RUNTIME_MISSING: &str = "JUR030";
    /// identity-grade-unsatisfied — conflicting changes need review.
    pub const IDENTITY_GRADE_UNSATISFIED: &str = "JUR041";
    /// identity-ambiguous (duplicate id/alias) — conflicting changes need review.
    pub const IDENTITY_AMBIGUOUS: &str = "JUR042";
    /// unresolved-overlay — draft not yet saved or synced.
    pub const UNRESOLVED_OVERLAY: &str = "JUR050";
    /// repair-not-authorized — repair could not be applied safely.
    pub const REPAIR_NOT_AUTHORIZED: &str = "JUR051";
    /// repair-unsafe (deterministic != safe) — repair could not be applied safely.
    pub const REPAIR_UNSAFE: &str = "JUR052";
    /// repair-stale-basis — conflicting changes need review.
    pub const REPAIR_STALE_BASIS: &str = "JUR053";
    /// dangling-relation — conflicting changes need review.
    pub const DANGLING_RELATION: &str = "JUR060";
}

/// Map an internal finding code to its everyday-language rendering (R4 §3).
#[must_use]
pub fn everyday_rendering(code: &str) -> &'static str {
    match code {
        codes::HOLDER_UNAVAILABLE => "source unavailable or stale",
        codes::WRITE_ROUTE_UNAVAILABLE | codes::UNRESOLVED_OVERLAY => {
            "draft not yet saved or synced"
        }
        codes::REPAIR_NOT_AUTHORIZED | codes::REPAIR_UNSAFE => "repair could not be applied safely",
        _ => "conflicting changes need review",
    }
}

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
    /// The governing Contract for a subject (dispatch + `contract_for`).
    pub(crate) fn contract_for(
        &self,
        subject: JurisdictionSubject,
    ) -> Result<crate::contract::JurisdictionContract, CheckerError> {
        let profile = self
            .profiles
            .for_subject(subject, self.store)
            .ok_or(CheckerError::Ungoverned(subject))?;
        Ok(profile.contract_for(subject, self.store))
    }

    /// Q1: Which Holder does this subject resolve to at this Basis?
    ///
    /// If the Basis perspective is `ClientScoped{client}` AND the owning file's
    /// component is that client's `BufferGeneration`, resolve to the buffer;
    /// else the contract's first read-precedence Holder.
    pub fn resolve_holder(
        &self,
        subject: JurisdictionSubject,
        basis: &WorkspaceBasis,
    ) -> Result<Holder, CheckerError> {
        let contract = self.contract_for(subject)?;

        // Buffer capture: if the perspective is client-scoped and the owning
        // file's component is a matching BufferGeneration, resolve to buffer.
        if let BasisPerspective::ClientScoped { client } = &basis.perspective {
            for component in basis.components.values() {
                if let BasisComponent::BufferGeneration {
                    client: c, buffer, ..
                } = component
                    && c == client
                {
                    return Ok(Holder::Buffer {
                        client: *client,
                        buffer: *buffer,
                    });
                }
            }
        }

        contract
            .resolution
            .read_precedence
            .first()
            .cloned()
            .ok_or(CheckerError::Ungoverned(subject))
    }

    /// Q2: Where would a write go? (Buffer capture is not routing.)
    pub fn write_route(
        &self,
        subject: JurisdictionSubject,
        _basis: &WorkspaceBasis,
    ) -> Result<Holder, CheckerError> {
        Ok(self.contract_for(subject)?.mutation.write_route)
    }

    /// Q3: Is a merge runtime required? (Always None in Phase -1.)
    pub fn merge_runtime_required(
        &self,
        subject: JurisdictionSubject,
    ) -> Result<Option<MergeRuntimeRef>, CheckerError> {
        Ok(self.contract_for(subject)?.resolution.merge_runtime)
    }

    /// Q4: Does identity satisfy every incoming durable Relation?
    ///
    /// For each Relation targeting `node` with `requires = Some(req)`:
    /// `!grade_of(node).satisfies(req.minimum)` → JUR041.
    pub fn identity_satisfies_relations(
        &self,
        node: NodeId,
        _basis: &WorkspaceBasis,
    ) -> Result<Vec<Finding>, CheckerError> {
        let head = self.store.head()?;
        let mut findings = Vec::new();
        let node_grade = grade_of(JurisdictionSubject::Node(node), self.store);

        for relation in self.store.relations()? {
            if relation.target.node() != node {
                continue;
            }
            let Relation {
                requires: Some(req),
                ..
            } = &relation
            else {
                continue;
            };
            if !node_grade.satisfies(req.minimum) {
                findings.push(Finding {
                    subject: JurisdictionSubject::Node(node),
                    code: codes::IDENTITY_GRADE_UNSATISFIED,
                    message: format!(
                        "node {node} grade {node_grade:?} does not satisfy required {:?} \
                         for relation {}",
                        req.minimum, relation.id
                    ),
                });
            }
        }
        let _ = head;
        Ok(findings)
    }

    /// Q5: Are there unresolved Overlays (optionally scoped to a subject)?
    ///
    /// Scans `JUR_OVERLAY`, keeps `Active | RepairProposed | Contested`, and
    /// filters by subject when given. Empty before M5.
    pub fn unresolved_overlays(
        &self,
        subject: Option<JurisdictionSubject>,
    ) -> Result<Vec<OverlayId>, CheckerError> {
        let mut out = Vec::new();
        for (_key, value) in self.store.scan_aux(JUR_OVERLAY)? {
            let overlay: crate::overlay::Overlay = serde_json::from_value(value).map_err(|e| {
                CheckerError::Store(liminal_graph::StoreError::Corrupt(e.to_string()))
            })?;
            let unresolved = matches!(
                overlay.state,
                OverlayState::Active | OverlayState::RepairProposed(_) | OverlayState::Contested
            );
            if !unresolved {
                continue;
            }
            if let Some(s) = subject
                && overlay.subject != s
            {
                continue;
            }
            out.push(overlay.id);
        }
        out.sort();
        Ok(out)
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
        let mut refusals = Vec::new();
        for step in plan.steps.values() {
            let contract = self.contract_for(step.subject)?;
            if matches!(
                contract.mutation.repair_authorization,
                crate::contract::RepairAuthorization::ReviewRequired
            ) {
                refusals.push(Finding {
                    subject: step.subject,
                    code: codes::REPAIR_NOT_AUTHORIZED,
                    message: format!("subject {} requires review", step.subject),
                });
            }
        }
        Ok(AuthorizationReport { refusals })
    }

    /// Q8: Is the repair deterministic, safe, ordered, recoverable, and
    /// reversible at the captured Basis? A unique result is necessary but not
    /// sufficient (Law 3G; R4 §6).
    pub fn evaluate_repair(&self, plan: &RepairPlan) -> Result<RepairDecision, CheckerError> {
        let _ = plan;
        // ── STUB (M04.4, T1 — the killer #2 core). BLOCKED on DG-4.1. ──
        //
        // Implement the full v4 §7.7 conjunction (M04 Algorithm B). Collect
        // `reasons: Vec<ReviewReason>`; return `AutoApply{evidence}` iff empty,
        // else `NeedsReview{reasons: sorted+deduped}`. NO Reject variant.
        //
        // 1. ONE VALID RESULT AT THE CAPTURED BASIS
        //    a. topo_order(&plan.into_dag)? — Err → "not a dependency DAG: {e}".
        //    b. per step (topo order): expected_prestate must match the basis
        //       component for its subject key, OR an earlier step's
        //       expected_poststate on the same key (intra-plan chaining).
        //       Mismatch → JUR053 "prestate mismatch for {subject}: {detail}".
        //    c. WriteFile merge steps: recompute three_way(base, ours, current)
        //       from SYS_BLOB + file; Conflict → JUR052-class
        //       "no unique result at the captured basis".
        // 2. PER-SUBJECT AUTHORIZATION (mutation-local, Law 3F)
        //    a. self.authorize(plan)? — each refusal → JUR051 "unauthorized: {msg}".
        //    b. any subject with RepairAuthorization::ReviewRequired → JUR051
        //       "{subject}: repairs to this item always require review".
        // 3. IDENTITY + INVARIANTS (what stops the DAG — v4 §7.7 worked example)
        //    a. per InsertSourceId step: claim grade := Explicit if the target
        //       block ALREADY carries alias(entity), else Inferred.
        //    b. per RetargetRelation step: req = relation.requires.minimum;
        //       effective grade = min(claim grades establishing target identity;
        //       else grade_of(target)); !satisfies(req) → JUR041 "reattachment
        //       relies on a heuristic match (inferred) but the relation
        //       requires {req}".
        //    c. dangling: no step may leave a PreserveAndSurface relation
        //       dangling → JUR060.
        // 4. DOMAIN SAFETY: group steps by governing profile; call
        //    profile.safety_check(plan, store); Err(rs) → reasons += rs.
        //    Evidence combination: all StructurallyDisjoint → one, descriptions
        //    "; "-joined; mixed kinds → "mixed safety evidence; automatic
        //    composition is not attempted in Phase -1".
        // 5. IDEMPOTENT + REVERTIBLE: every step has an idempotency_key;
        //    revertible := plan.inverse.is_some() OR every WriteFile prestate
        //    blob exists in SYS_BLOB; neither → JUR052 "no revert path recorded".
        // 6. reasons empty → AutoApply{evidence}; else NeedsReview{reasons}.
        //
        // Finding codes live in `checker::codes`. This is the conformance
        // oracle for auto-apply — interpret, never compile (v4 §125).
        todo!("Phase -1 M4: evaluate_repair conjunction (Algorithm B; blocked on DG-4.1)")
    }

    /// The whole-workspace check backing `lim check`. Sound workspace → empty
    /// report → exit 0, zero bytes of output (Law 3E).
    ///
    /// Collects: Q4 over every node with incoming required Relations, JUR050
    /// per unresolved overlay, JUR042 for ambiguous aliases. Findings sorted by
    /// `(code, subject)`.
    pub fn check_workspace(&self, basis: &WorkspaceBasis) -> Result<CheckReport, CheckerError> {
        let mut findings = Vec::new();

        // Q4: identity grade over every node with an incoming required Relation.
        let mut checked_nodes = std::collections::BTreeSet::new();
        for relation in self.store.relations()? {
            if relation.requires.is_some() {
                checked_nodes.insert(relation.target.node());
            }
        }
        for node in checked_nodes {
            findings.extend(self.identity_satisfies_relations(node, basis)?);
        }

        // JUR050: one per unresolved overlay.
        for overlay_id in self.unresolved_overlays(None)? {
            findings.push(Finding {
                subject: JurisdictionSubject::Node(NodeId::from_uuid(overlay_id.as_uuid())),
                code: codes::UNRESOLVED_OVERLAY,
                message: format!("unresolved overlay {overlay_id}"),
            });
        }

        // JUR042: aliases claimed by two or more live blocks.
        findings.extend(self.ambiguous_aliases()?);

        findings
            .sort_by(|a, b| (a.code, a.subject.to_string()).cmp(&(b.code, b.subject.to_string())));
        Ok(CheckReport { findings })
    }

    /// Detect aliases claimed by two or more live nodes (JUR042). In the toy
    /// each alias maps to exactly one node; a duplicate is a data error the
    /// checker must surface rather than panic on.
    fn ambiguous_aliases(&self) -> Result<Vec<Finding>, CheckerError> {
        use std::collections::BTreeMap;
        let mut by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (alias, value) in self.store.scan_aux(JUR_ALIAS)? {
            if let Some(node) = value.get("node").and_then(|v| v.as_str()) {
                by_node.entry(node.to_owned()).or_default().push(alias);
            }
        }
        let mut findings = Vec::new();
        for (node_str, aliases) in by_node {
            if aliases.len() > 1
                && let Ok(node) = node_str.parse::<NodeId>()
            {
                findings.push(Finding {
                    subject: JurisdictionSubject::Node(node),
                    code: codes::IDENTITY_AMBIGUOUS,
                    message: format!("node {node_str} claimed by aliases {aliases:?}"),
                });
            }
        }
        Ok(findings)
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
