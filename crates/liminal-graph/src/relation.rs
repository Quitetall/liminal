//! Relation: a typed association, dependency, order, invariant, or contract
//! between Nodes (v4 §5).

use liminal_id::{IdentityGrade, KindId, NodeId, RelationId, RevisionId};
use serde::{Deserialize, Serialize};

use crate::node::PayloadRef;

/// A typed association between Nodes (v4 §5, verbatim sketch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// Runtime identity.
    pub id: RelationId,
    /// Source endpoint.
    pub source: NodeId,
    /// Target endpoint, possibly anchored within the target (v4 §5.3).
    pub target: Target,
    /// Relation kind (containment, reference, citation, provenance, … v4 §5).
    pub kind: KindId,
    /// Physically associated data (same storage model as Node payloads).
    pub payload: PayloadRef,
    /// Per-subject physical revision counter.
    pub revision: RevisionId,
    /// Physical flags.
    pub flags: RelationFlags,
}

/// A Relation target (v4 §5, §5.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    /// The whole target Node.
    Node(NodeId),
    /// A range within the target Node or resource (v4 §5.3): a sentence, an
    /// image region, a PDF rectangle, a time interval, a stroke set.
    Anchored {
        /// The anchored Node.
        node: NodeId,
        /// Where within it.
        anchor: AnchorRef,
    },
}

impl Target {
    /// The target Node regardless of anchoring.
    #[must_use]
    pub fn node(&self) -> NodeId {
        match self {
            Self::Node(n) | Self::Anchored { node: n, .. } => *n,
        }
    }
}

/// Revision-aware anchor within a Node or resource.
///
/// SHAPE PROVISIONAL: anchors must be revision-aware and resilient to edits;
/// raw byte offsets may be cached but are never the only persistent
/// representation (v4 §5.3). Real anchor design is a Phase -1.1/-1.2
/// experiment; the toy carries an opaque description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRef {
    /// Opaque provisional anchor description.
    pub description: String,
}

/// Physical relation flags (v4 §5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationFlags(pub u16);

impl RelationFlags {
    /// The relation is deleted but retained for history (v4 §87).
    pub const TOMBSTONE: RelationFlags = RelationFlags(1);

    /// Whether every flag in `other` is set in `self`.
    #[must_use]
    pub fn contains(self, other: RelationFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Minimum identity grade this Relation requires of its target (v4 §19.1,
/// Law 12: identity strength must match Relation durability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRequirement {
    /// The declared minimum grade.
    pub minimum: IdentityGrade,
}
