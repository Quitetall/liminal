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
use liminal_graph::{GraphStore, NodeFlags, Relation};
use liminal_id::{JurisdictionKey, JurisdictionSubject, NodeId, OverlayId};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};

use crate::holder::{Holder, MergeRuntimeRef};
use crate::overlay::OverlayState;
use crate::profile::{JurisdictionProfile, ProfileSet, grade_of};
use crate::repair::{
    RepairDecision, RepairOperation, RepairPlan, ReviewReason, SafetyEvidence, StatePredicate,
};

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
    /// Enumerate every Node and Relation subject present in the workspace
    /// (v4 Part XXII final gate; AM-12.1).
    ///
    /// Final-gate audits use this whole-store surface instead of sampling
    /// aliases or known kinds, so an ungoverned subject cannot stay hidden.
    pub fn subjects(&self) -> Result<Vec<JurisdictionSubject>, CheckerError> {
        let mut subjects = self
            .store
            .nodes()?
            .into_iter()
            .map(|node| JurisdictionSubject::Node(node.id))
            .chain(
                self.store
                    .relations()?
                    .into_iter()
                    .map(|relation| JurisdictionSubject::Relation(relation.id)),
            )
            .collect::<Vec<_>>();
        subjects.sort_unstable();
        subjects.dedup();
        Ok(subjects)
    }

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
        let durable_holder = contract
            .resolution
            .read_precedence
            .first()
            .cloned()
            .ok_or(CheckerError::Ungoverned(subject))?;

        // Buffer capture belongs only to an external-file contract and only to
        // its owning Path component. Graph-native contracts remain Graph-held
        // under every perspective (v4 §7.5–7.6).
        if let (BasisPerspective::ClientScoped { client }, Holder::File { path }) =
            (&basis.perspective, &durable_holder)
            && let Some(BasisComponent::BufferGeneration {
                client: component_client,
                buffer,
                ..
            }) = basis.components.get(&JurisdictionKey::Path(path.clone()))
            && component_client == client
        {
            return Ok(Holder::Buffer {
                client: *client,
                buffer: *buffer,
            });
        }

        Ok(durable_holder)
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
    /// sufficient (Law 3G; R4 §6). The full v4 §7.7 conjunction (M04 Algorithm
    /// B). Collects reasons; `AutoApply{evidence}` iff empty, else
    /// `NeedsReview{reasons}`. There is NO Reject outcome (Law 3B).
    pub fn evaluate_repair(&self, plan: &RepairPlan) -> Result<RepairDecision, CheckerError> {
        let mut reasons: Vec<ReviewReason> = Vec::new();

        // 1. ONE VALID RESULT AT THE CAPTURED BASIS.
        let order = match crate::repair::topo_order(plan) {
            Ok(o) => o,
            Err(e) => {
                return Ok(RepairDecision::NeedsReview {
                    reasons: vec![ReviewReason(format!("not a dependency DAG: {e}"))],
                });
            }
        };
        // 1b. Per-step prestate must match the basis component for its subject
        // key, OR an earlier step's poststate on the same key (intra-plan
        // chaining). Track poststates seen so far in topo order.
        let mut produced: std::collections::BTreeMap<JurisdictionKey, StatePredicate> =
            std::collections::BTreeMap::new();
        for step_id in &order {
            let step = &plan.steps[step_id];
            if let Some((key, pre)) = predicate_key(&step.expected_prestate) {
                let basis_ok = basis_matches(&plan.basis, &key, &pre);
                let chain_ok = produced.get(&key) == Some(&pre);
                if !basis_ok && !chain_ok {
                    reasons.push(ReviewReason(format!(
                        "prestate mismatch for {}: not at captured basis",
                        step.subject
                    )));
                }
            }
            if let Some((key, post)) = predicate_key(&step.expected_poststate) {
                produced.insert(key, post);
            }
        }

        // 2. PER-SUBJECT AUTHORIZATION (mutation-local, Law 3F).
        let auth = self.authorize(plan)?;
        for refusal in &auth.refusals {
            reasons.push(ReviewReason(format!("unauthorized: {}", refusal.message)));
        }

        // 4. DOMAIN SAFETY — the profile predicate (determinism ≠ safety, R4 §6).
        // Group steps by governing profile; every governing profile must pass.
        let mut evidences: Vec<SafetyEvidence> = Vec::new();
        let mut safety_reasons: Vec<ReviewReason> = Vec::new();
        for profile in self.governing_profiles(plan) {
            match profile.safety_check(plan, self.store) {
                Ok(ev) => evidences.push(ev),
                Err(rs) => safety_reasons.extend(rs),
            }
        }
        reasons.extend(safety_reasons);

        // 5. IDEMPOTENT + REVERTIBLE (R4 §6). Every step carries an idempotency
        // key by construction; revertible := an inverse exists OR every
        // WriteFile prestate blob is recoverable from SYS_BLOB.
        let revertible = plan.inverse.is_some()
            || plan.steps.values().all(|s| match &s.operation {
                RepairOperation::WriteFile { .. } => match &s.expected_prestate {
                    StatePredicate::FileContent { hash, .. } => {
                        crate::blob::get(self.store, *hash).ok().flatten().is_some()
                    }
                    StatePredicate::FileAbsent { .. } => true,
                    _ => false,
                },
                _ => true,
            });
        if !revertible {
            reasons.push(ReviewReason("no revert path recorded".into()));
        }

        if reasons.is_empty() {
            Ok(RepairDecision::AutoApply {
                evidence: combine_evidence(evidences),
            })
        } else {
            reasons.sort_by(|a, b| a.0.cmp(&b.0));
            reasons.dedup();
            Ok(RepairDecision::NeedsReview { reasons })
        }
    }

    /// The distinct profiles governing a plan's steps (each subject dispatched
    /// via `ProfileSet::for_subject`), in registration order.
    fn governing_profiles(&self, plan: &RepairPlan) -> Vec<&dyn JurisdictionProfile> {
        let mut ids: Vec<crate::profile::ProfileId> = Vec::new();
        for step in plan.steps.values() {
            if let Some(p) = self.profiles.for_subject(step.subject, self.store) {
                let id = p.id();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids.iter().filter_map(|id| self.profiles.get(id)).collect()
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

        // JUR050: one per CONTESTED overlay — a genuinely unsound state ILRP
        // recovery could not resolve (v4 §7.8 step 5: "never guess"). Routine
        // `Active`/`RepairProposed` drafts are surfaced via `lim overlays` and
        // the Reconciliation Queue, not amplified here (R4 §2.3: the checker
        // does not duplicate the queue's one visible incident per root cause;
        // sound-but-offline/reviewable sessions stay checker-silent).
        for (_key, value) in self.store.scan_aux(JUR_OVERLAY)? {
            let overlay: crate::overlay::Overlay = serde_json::from_value(value).map_err(|e| {
                CheckerError::Store(liminal_graph::StoreError::Corrupt(e.to_string()))
            })?;
            if overlay.state == OverlayState::Contested {
                findings.push(Finding {
                    subject: overlay.subject,
                    code: codes::UNRESOLVED_OVERLAY,
                    message: format!("contested overlay {}", overlay.id),
                });
            }
        }

        // JUR042: identity-ambiguous aliases (multi-claimed nodes and
        // first-wins duplicate orphans).
        findings.extend(self.ambiguous_aliases()?);

        findings
            .sort_by(|a, b| (a.code, a.subject.to_string()).cmp(&(b.code, b.subject.to_string())));
        Ok(CheckReport { findings })
    }

    /// Detect identity-ambiguous states (JUR042). Two observable residues in
    /// the toy: a node claimed by two or more aliases (a stale alias that was
    /// never retired), and — the first-wins residue of D03.3 — a live node
    /// carrying `HAS_DURABLE_ID` that no alias claims: its `{#id}` lost a
    /// duplicate race at ingestion, so its identity is ambiguous. Either way
    /// the checker must surface it rather than stay silent or panic.
    fn ambiguous_aliases(&self) -> Result<Vec<Finding>, CheckerError> {
        use std::collections::BTreeMap;
        let mut by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (alias, value) in self.store.scan_aux(JUR_ALIAS)? {
            if let Some(node) = value.get("node").and_then(|v| v.as_str()) {
                by_node.entry(node.to_owned()).or_default().push(alias);
            }
        }
        let mut findings = Vec::new();
        for (node_str, aliases) in &by_node {
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
        for node in self.store.nodes()? {
            if node.flags.contains(NodeFlags::HAS_DURABLE_ID)
                && !node.flags.contains(NodeFlags::TOMBSTONE)
                && !by_node.contains_key(&node.id.to_string())
            {
                findings.push(Finding {
                    subject: JurisdictionSubject::Node(node.id),
                    code: codes::IDENTITY_AMBIGUOUS,
                    message: format!(
                        "node {} carries a durable-id marker but no alias claims it \
                         (its id lost a first-wins duplicate race)",
                        node.id
                    ),
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

/// The `JurisdictionKey` + normalized predicate a `StatePredicate` addresses,
/// for basis/chaining comparison. Returns `None` for `Any` (no expectation).
fn predicate_key(pred: &StatePredicate) -> Option<(JurisdictionKey, StatePredicate)> {
    match pred {
        StatePredicate::FileContent { path, .. } | StatePredicate::FileAbsent { path } => {
            Some((JurisdictionKey::Path(path.clone()), pred.clone()))
        }
        StatePredicate::NodeAt { node, .. } => Some((
            JurisdictionKey::Subject(JurisdictionSubject::Node(*node)),
            pred.clone(),
        )),
        StatePredicate::RelationAt { relation, .. } => Some((
            JurisdictionKey::Subject(JurisdictionSubject::Relation(*relation)),
            pred.clone(),
        )),
        StatePredicate::ExternalRevision { source, .. } => {
            Some((JurisdictionKey::Source(*source), pred.clone()))
        }
        StatePredicate::Any => None,
    }
}

/// Whether a plan's captured basis carries the state a file predicate expects.
/// Only file predicates are basis-checked here (graph revisions are re-verified
/// by ILRP at apply); non-file predicates pass.
fn basis_matches(basis: &WorkspaceBasis, key: &JurisdictionKey, pred: &StatePredicate) -> bool {
    match pred {
        StatePredicate::FileContent { hash, .. } => {
            matches!(
                basis.components.get(key),
                Some(BasisComponent::FileContent { hash: h, .. }) if h == hash
            )
        }
        // FileAbsent matches when the basis has no component for the path.
        StatePredicate::FileAbsent { .. } => !basis.components.contains_key(key),
        // Graph/external predicates are re-verified by ILRP; accept at plan time.
        _ => true,
    }
}

/// Combine per-profile safety evidence (M04 Algorithm B §4). All
/// `StructurallyDisjoint` → one, descriptions "; "-joined; any other mix →
/// a `DomainValidator` recording that composition is not attempted in Phase -1.
fn combine_evidence(evidences: Vec<SafetyEvidence>) -> SafetyEvidence {
    if evidences.is_empty() {
        return SafetyEvidence::StructurallyDisjoint {
            description: "no governed mutation".into(),
        };
    }
    let all_disjoint = evidences
        .iter()
        .all(|e| matches!(e, SafetyEvidence::StructurallyDisjoint { .. }));
    if all_disjoint {
        let description = evidences
            .iter()
            .filter_map(|e| match e {
                SafetyEvidence::StructurallyDisjoint { description } => Some(description.clone()),
                _ => None,
            })
            .filter(|d| !d.is_empty())
            .collect::<Vec<_>>()
            .join("; ");
        SafetyEvidence::StructurallyDisjoint { description }
    } else if evidences.len() == 1 {
        evidences.into_iter().next().unwrap()
    } else {
        SafetyEvidence::DomainValidator {
            validator: "composite".into(),
            report: "mixed safety evidence; automatic composition is not attempted in Phase -1"
                .into(),
        }
    }
}
