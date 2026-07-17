//! ID newtypes (v4 §44: persistent 128-bit IDs; mapped to dense handles only as
//! a post-Phase-1 optimization).
//!
//! UUIDv7 gives time-ordered 128-bit identifiers with the ecosystem-standard
//! crate. Display forms are prefixed (`node:018f…`) so IDs are self-describing
//! in logs, CLI output, and NDJSON store records; `FromStr` accepts the prefixed
//! or bare form.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Failed to parse a prefixed or bare UUID identifier.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid {expected} id: {input:?}")]
pub struct ParseIdError {
    /// The rejected input.
    pub input: String,
    /// The expected Display prefix (e.g. `node`).
    pub expected: &'static str,
}

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        #[allow(clippy::new_without_default, reason = "Default minting a fresh unique id would be surprising")]
        impl $name {
            /// Mint a fresh time-ordered (UUIDv7) identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// The raw UUID.
            #[must_use]
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Wrap an existing UUID (e.g. deserialized from an external system).
            #[must_use]
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, ":{}"), self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let bare = s.strip_prefix(concat!($prefix, ":")).unwrap_or(s);
                Uuid::parse_str(bare).map(Self).map_err(|_| ParseIdError {
                    input: s.to_owned(),
                    expected: $prefix,
                })
            }
        }
    };
}

define_id!(
    /// Runtime identity of a Node (v4 §4).
    NodeId, "node"
);
define_id!(
    /// Runtime identity of a Relation (v4 §5).
    RelationId, "relation"
);
define_id!(
    /// Logical continuity of a mutable entity across accepted revisions (v4 §19, Law 13).
    EntityId, "entity"
);
define_id!(
    /// A semantic transaction (v4 §7.5, §86).
    TransactionId, "txn"
);
define_id!(
    /// A repair plan (v4 §7.7).
    RepairId, "repair"
);
define_id!(
    /// One proposed mutation inside a repair DAG (v4 §7.7).
    RepairStepId, "step"
);
define_id!(
    /// A durable unaccepted draft (v4 §7.10).
    OverlayId, "overlay"
);
define_id!(
    /// One coalesced Reconciliation Queue entry (R4 §9).
    ReconciliationItemId, "reconcile"
);
define_id!(
    /// An editor/device client holding working buffers (v4 §7.5).
    ClientId, "client"
);
define_id!(
    /// One editor buffer within a client (v4 §7.5).
    BufferId, "buffer"
);
define_id!(
    /// A human, plugin, AI, or remote actor recorded in transactions (v4 §86).
    ActorId, "actor"
);
define_id!(
    /// An external source/service identity (v4 §7.5 `ExternalRevision` / `Observation`).
    SourceId, "source"
);
define_id!(
    /// An immutable outward-facing publication revision (v4 §7.5 `Published`).
    PublicationId, "publication"
);
define_id!(
    /// A declared federated merge domain (v4 §7.5 `Federated`; R4 §11.7).
    FederationId, "federation"
);
define_id!(
    /// Idempotency key for at-least-once external repair steps (v4 §7.7–7.8).
    IdempotencyKey, "idem"
);

/// Node/Relation kind (v4 §4). Interning registry is a deferred optimization (v4 §48).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct KindId(pub u32);

/// Per-subject physical revision counter (v4 §4).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct RevisionId(pub u64);

/// Monotonic graph-store revision (v4 §7.5 `GraphSnapshot` component).
/// `Default` is revision 0: the empty pre-first-commit store.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct GraphRevisionId(pub u64);

/// Prevents buffer-generation collisions across editor sessions (v4 §7.5).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SessionEpoch(pub u64);

/// Opaque external revision token (v4 §7.5 `ExternalRevision`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RevisionToken(pub String);

/// Git object id in provisional string form (v4 §7.5 `GitCommit`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

/// UTF-8 workspace-relative path (v4 §7.5 `FileContent`).
///
/// SHAPE PROVISIONAL: numeric interning (v4 §44) is deferred; the newtype exists
/// so call sites do not commit to a raw path type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PathId(pub camino::Utf8PathBuf);

impl fmt::Display for PathId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_fromstr_round_trip() {
        let id = NodeId::new();
        let s = id.to_string();
        assert!(s.starts_with("node:"));
        assert_eq!(s.parse::<NodeId>().unwrap(), id);
        // Bare UUID form is also accepted.
        assert_eq!(id.as_uuid().to_string().parse::<NodeId>().unwrap(), id);
    }

    #[test]
    fn wrong_prefix_still_parses_as_bare_uuid_is_rejected() {
        let id = NodeId::new();
        let s = format!("relation:{}", id.as_uuid());
        // The prefix is not stripped, so parsing fails: ids are self-describing.
        assert!(s.parse::<NodeId>().is_err());
    }

    #[test]
    fn uuidv7_ids_are_time_ordered() {
        let a = TransactionId::new();
        let b = TransactionId::new();
        assert!(a <= b, "UUIDv7 must be monotonically non-decreasing");
    }

    #[test]
    fn serde_is_transparent() {
        let id = RepairId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{}\"", id.as_uuid()));
    }
}
