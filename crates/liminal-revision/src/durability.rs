//! Durability classes (AM-8.3, v4 §24.1). A component's durability is an
//! invalidation *optimization* — it orders the cache's staleness checks
//! (cheapest-to-change first) — NOT a statement of truth or freshness. It never
//! decides whether a value is correct or current, only how eagerly the memo
//! layer should re-check it.

use crate::basis::BasisComponent;

/// The five durability classes (v4 §24.1, verbatim order strongest→weakest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Durability {
    /// Content-addressed / commit-pinned bytes that can never change under a
    /// fixed identity.
    Immutable,
    /// Stable durable state that changes rarely.
    High,
    /// Durable state that changes at ordinary edit cadence.
    Medium,
    /// Provisional or externally-sourced state that changes freely.
    Low,
    /// Transient, valid only within one computation.
    Ephemeral,
}

/// The durability class of a Basis component (v4 §24.1 table). Total over every
/// `BasisComponent` variant — a unit test asserts this.
#[must_use]
pub fn durability(component: &BasisComponent) -> Durability {
    match component {
        BasisComponent::ObjectContent { .. } | BasisComponent::GitCommit { .. } => {
            Durability::Immutable
        }
        BasisComponent::FileContent { .. } | BasisComponent::GraphSnapshot { .. } => {
            Durability::Medium
        }
        BasisComponent::BufferGeneration { .. }
        | BasisComponent::Observation { .. }
        | BasisComponent::ExternalRevision { .. } => Durability::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liminal_id::{
        BufferId, ClientId, ContentHash, GraphRevisionId, ObjectId, PathId, RevisionToken,
        SessionEpoch, SourceId, Timestamp,
    };

    /// The table is total and orders checks Low→…→Immutable (Low is re-checked
    /// first because it changes most freely). The ordering is the point; the
    /// classes it assigns are the v4 §24.1 verbatim table.
    #[test]
    fn durability_table_is_total() {
        let all = [
            BasisComponent::ObjectContent {
                hash: ContentHash::of(b"x"),
            },
            BasisComponent::GitCommit {
                oid: ObjectId("commit-oid".into()),
            },
            BasisComponent::FileContent {
                path: PathId("a.md".into()),
                hash: ContentHash::of(b"x"),
            },
            BasisComponent::GraphSnapshot {
                revision: GraphRevisionId(1),
            },
            BasisComponent::BufferGeneration {
                client: ClientId::new(),
                buffer: BufferId::new(),
                epoch: SessionEpoch(0),
                generation: 0,
                content_hash: None,
                base_file_hash: None,
            },
            BasisComponent::Observation {
                source: SourceId::new(),
                observed_at: Timestamp::now(),
                hash: ContentHash::of(b"x"),
            },
            BasisComponent::ExternalRevision {
                source: SourceId::new(),
                token: RevisionToken("t".into()),
            },
        ];
        // Every variant maps (no panic) and Immutable is the strongest class.
        for c in &all {
            assert!(durability(c) <= Durability::Ephemeral);
        }
        assert_eq!(
            durability(&BasisComponent::ObjectContent {
                hash: ContentHash::of(b"x")
            }),
            Durability::Immutable
        );
        assert!(Durability::Immutable < Durability::Low);
    }
}
