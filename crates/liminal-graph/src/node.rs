//! Node: an identifiable unit of state (v4 §4).

use liminal_id::{ContentHash, KindId, NodeId, RevisionId};
use serde::{Deserialize, Serialize};

/// An identifiable unit of state (v4 §4, verbatim sketch).
///
/// `kind`, `revision`, and `flags` are physical fast paths; semantically even a
/// kind may be understood as a Relation to a schema definition (v4 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Runtime identity (v4 §4.1).
    pub id: NodeId,
    /// Node kind (v4 §4).
    pub kind: KindId,
    /// Physically associated data — not a third semantic entity (v4 §4.2).
    pub payload: PayloadRef,
    /// Per-subject physical revision counter.
    pub revision: RevisionId,
    /// Physical flags.
    pub flags: NodeFlags,
}

/// Physical payload association (v4 §4.2).
///
/// Toy variants only: typed record slots, interned symbols, shared text ranges,
/// and expression handles arrive with domains and the text subsystem
/// (`liminal-text`, v4 §46 — "benchmark rather than commit ideologically").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadRef {
    /// No payload.
    None,
    /// Inline text — toy stand-in for shared text ranges (v4 §46 owns this later).
    Text(String),
    /// Inline bytes.
    Bytes(Vec<u8>),
    /// Content-addressed object handle (v4 §47).
    Object(ContentHash),
}

/// Physical node flags (v4 §4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFlags(pub u16);

impl NodeFlags {
    /// The node is deleted but retained for history (v4 §87).
    pub const TOMBSTONE: NodeFlags = NodeFlags(1);
    /// The node carries a durable identifier serialized in Holder-controlled
    /// source (identity grade Explicit, v4 §19.1).
    pub const HAS_DURABLE_ID: NodeFlags = NodeFlags(1 << 1);
    /// The node is derived from other subjects and carries provenance (Law 15).
    pub const DERIVED: NodeFlags = NodeFlags(1 << 2);

    /// Whether every flag in `other` is set in `self`.
    #[must_use]
    pub fn contains(self, other: NodeFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Union of two flag sets.
    #[must_use]
    pub fn union(self, other: NodeFlags) -> NodeFlags {
        NodeFlags(self.0 | other.0)
    }
}
