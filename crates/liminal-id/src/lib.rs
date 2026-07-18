//! Identity vocabulary for the Liminal semantic kernel.
//!
//! Pure data, zero policy, zero I/O beyond hashing. This crate is the shared
//! address vocabulary that keeps every crate above it acyclic.
//!
//! Spec: v4 §4.1 (Node identity), §19 (identity model: entity, version, anchor,
//! alias), §44 (128-bit persistent IDs), §7.2 (Jurisdiction subjects).

mod grade;
mod hash;
mod ids;
mod subject;
mod time;

pub use grade::{IdentityGrade, ParseGradeError};
pub use hash::{ContentHash, ParseHashError, VersionId};
pub use ids::{
    ActorId, BufferId, ClientId, EntityId, FederationId, GraphRevisionId, IdempotencyKey, KindId,
    NodeId, ObjectId, OverlayId, ParseIdError, PathId, PublicationId, ReconciliationItemId,
    RelationId, RepairId, RepairStepId, RevisionId, RevisionToken, SessionEpoch, SourceId,
    TransactionId,
};
pub use subject::{JurisdictionKey, JurisdictionSubject, ParseSubjectError};
pub use time::Timestamp;
