//! Jurisdiction Contracts: internal policy, normally supplied by a domain
//! profile — not common-case authoring (v4 §7.3, Law 3E).

use std::time::Duration;

use liminal_id::{IdentityGrade, JurisdictionSubject, KindId};
use serde::{Deserialize, Serialize};

use crate::holder::{Holder, MergeRuntimeRef};

/// A Contract over Nodes and Relations, grouped by concern (v4 §7.3, verbatim
/// grouping). Plain data — the checker and repair interpreter read it; nothing
/// compiles it (v4 §7.9, §125).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JurisdictionContract {
    /// Which subjects this Contract governs.
    pub scope: SubjectSelector,
    /// Candidate Holders, read precedence, fallback, federation (v4 §7.3).
    pub resolution: HolderResolution,
    /// Write route, foreign edits, repair authorization, safety (v4 §7.3).
    pub mutation: MutationPolicy,
    /// Identity requirement, revision behavior, dangling references (v4 §7.3).
    pub continuity: ContinuityPolicy,
    /// Overlay durability, aging, surfacing, publication (v4 §7.3).
    pub lifecycle: LifecyclePolicy,
}

/// Subject selection. SHAPE PROVISIONAL — a profile may use schema selectors
/// (`image.caption`) as compile-time shorthand, but those are addresses over
/// Nodes and Relations, never a third primitive (v4 §7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectSelector {
    /// Exactly one subject.
    Exact(JurisdictionSubject),
    /// Every subject of a kind.
    Kind(KindId),
    /// Everything the profile governs.
    All,
}

/// Candidate Holders and read precedence (v4 §7.3 `HolderResolution`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HolderResolution {
    /// Candidate Holders in declaration order.
    pub candidates: Vec<Holder>,
    /// Read precedence (working before durable, per perspective rules v4 §7.5).
    pub read_precedence: Vec<Holder>,
    /// Fallback when preferred Holders are unavailable (Law 3B: capture still
    /// never blocks — an unavailable fallback produces an Overlay).
    pub fallback: Option<Holder>,
    /// Declared federated merge runtime, if any (named, never implemented here).
    pub merge_runtime: Option<MergeRuntimeRef>,
}

/// Write routing and repair policy (v4 §7.3 `MutationPolicy`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationPolicy {
    /// Where writes go.
    pub write_route: Holder,
    /// What happens when a foreign tool edits the subject (v4 §8.5).
    pub foreign_edits: ForeignEditPolicy,
    /// Whether repairs may auto-apply (never blocks capture — Law 3B).
    pub repair_authorization: RepairAuthorization,
    /// The domain safety requirement for automatic acceptance (R4 §6:
    /// determinism is not safety).
    pub safety: SafetyRequirement,
}

/// Foreign-edit handling. SHAPE PROVISIONAL (v4 §8.5 steps 4–6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ForeignEditPolicy {
    /// Accept and rederive; identity reused only at the declared grade.
    Accept,
    /// Accept but mark continuity as inferred pending review.
    Reidentify,
    /// Preserve as Overlay and surface for review.
    Review,
}

/// Whether this subject's repairs may apply automatically.
///
/// Deliberately has NO `Forbidden` variant: capture is never rejected
/// (Law 3B); the only alternatives are auto-apply and review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairAuthorization {
    /// Auto-apply when every conjunctive condition holds (v4 §7.7).
    Automatic,
    /// Always route through review.
    ReviewRequired,
}

/// Domain-specific safety evidence required for automatic acceptance (R4 §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SafetyRequirement {
    /// Structural disjointness at the relevant Node/syntax/anchor level.
    StructuralDisjointness,
    /// A named language/domain validator proving non-interference.
    DomainValidator(String),
    /// Explicit human approval.
    HumanApproval,
}

/// Identity and reference continuity (v4 §7.3 `ContinuityPolicy`, Law 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuityPolicy {
    /// Minimum identity grade this subject must sustain.
    pub required_identity: IdentityGrade,
    /// What happens when an incoming durable Relation dangles.
    pub on_dangling: DanglingPolicy,
}

/// Dangling-reference behavior. SHAPE PROVISIONAL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DanglingPolicy {
    /// Preserve the Relation and surface one reconciliation item (v4 §7.7).
    PreserveAndSurface,
    /// Detach and record provenance.
    Detach,
}

/// Overlay lifecycle policy (v4 §7.3 `LifecyclePolicy`, §7.10.3; R4 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecyclePolicy {
    /// Age at which an unresolved Overlay surfaces contextually.
    pub surface_after: Duration,
    /// Age at which it escalates in the Reconciliation Queue.
    pub escalate_after: Duration,
    /// Profile-declared transient drafts (e.g. offline work awaiting sync) are
    /// reported separately and become debt past their window (R4 §2.3).
    pub declared_transient: bool,
}
