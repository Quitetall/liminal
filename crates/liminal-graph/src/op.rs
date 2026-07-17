//! Semantic transactions and operations (v4 §86).

use liminal_id::{
    ActorId, ContentHash, GraphRevisionId, NodeId, RelationId, SourceId, Timestamp, TransactionId,
};
use serde::{Deserialize, Serialize};

use crate::node::{Node, PayloadRef};
use crate::relation::{Relation, Target};

/// One semantic operation inside a transaction (v4 §86).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    /// Create a Node.
    CreateNode {
        /// The node to create.
        node: Node,
    },
    /// Delete a Node (tombstoned in history, v4 §87).
    DeleteNode {
        /// The node to delete.
        id: NodeId,
    },
    /// Replace a Node payload.
    SetPayload {
        /// The node to modify.
        id: NodeId,
        /// The new payload.
        payload: PayloadRef,
    },
    /// Add a Relation.
    AddRelation {
        /// The relation to add.
        relation: Relation,
    },
    /// Remove a Relation.
    RemoveRelation {
        /// The relation to remove.
        id: RelationId,
    },
    /// Redirect a Relation endpoint — the repair reattachment primitive
    /// (v4 §7.7: mutating the Relation endpoint is governed by the Relation's
    /// Holder, never by the target's Contract).
    RetargetRelation {
        /// The relation to retarget.
        id: RelationId,
        /// The new target.
        target: Target,
    },
    /// Insert an ordered child (containment/order are physically privileged
    /// Relations, v4 §5.2).
    InsertChild {
        /// Parent node.
        parent: NodeId,
        /// Child node.
        child: NodeId,
        /// Insertion index (clamped to the current child count).
        index: u64,
    },
    /// Move an ordered child.
    MoveChild {
        /// Parent node.
        parent: NodeId,
        /// Child node.
        child: NodeId,
        /// Destination index (clamped).
        index: u64,
    },
    /// Attach a content-addressed resource to a Node (v4 §47).
    AttachResource {
        /// The node.
        node: NodeId,
        /// The object hash.
        object: ContentHash,
    },
    /// Materialize an external observation into the graph (v4 §86; effects
    /// happen OUTSIDE the query engine and enter as transactions, v4 §8.6).
    MaterializeExternal {
        /// The node holding the materialization.
        node: NodeId,
        /// The observed source.
        source: SourceId,
        /// Hash of the observed value.
        observed: ContentHash,
        /// Observation time.
        at: Timestamp,
    },
}

/// Where a transaction came from (v4 §86).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A human edit.
    Human,
    /// A plugin.
    Plugin,
    /// An AI-proposed operation (derived until approved, v4 §84).
    Ai,
    /// A remote replica or service.
    Remote,
    /// Crash recovery (ILRP Recover, v4 §7.8 step 5).
    Recovery,
}

/// Transaction metadata (v4 §86).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxnMeta {
    /// Acting identity, when known.
    pub actor: Option<ActorId>,
    /// Human, plugin, AI, remote, or recovery origin.
    pub origin: Origin,
    /// Logical time of acceptance.
    pub at: Timestamp,
    /// Free-form provenance (Law 15). SHAPE PROVISIONAL — becomes structured.
    pub provenance: Option<String>,
    /// Optional inverse operations for one-command revert (R4 §6).
    pub inverse: Option<Vec<Operation>>,
}

/// An accepted semantic transaction (v4 §86).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction identity.
    pub id: TransactionId,
    /// The graph revision this transaction was built on.
    pub parent: GraphRevisionId,
    /// Metadata.
    pub meta: TxnMeta,
    /// The operations, applied atomically.
    pub ops: Vec<Operation>,
}
