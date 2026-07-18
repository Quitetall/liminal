//! The immutable input vector of a computation (v4 §7.5).

use std::collections::BTreeMap;

use liminal_id::{
    BufferId, ClientId, ContentHash, GraphRevisionId, JurisdictionKey, ObjectId, PathId,
    RevisionToken, SessionEpoch, SourceId, Timestamp, TransactionId,
};
use serde::{Deserialize, Serialize};

use crate::perspective::BasisPerspective;

/// One immutable Workspace Basis per computation (Law 3D; v4 §7.5).
///
/// Logically workspace-wide, physically dependency-granular: queries record
/// which components they read, so editing buffer A must not invalidate
/// computations that depend only on buffer B.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceBasis {
    /// Graph transaction this Basis was captured at.
    pub transaction: TransactionId,
    /// The declared perspective that selected these components (R4 §8).
    pub perspective: BasisPerspective,
    /// Selected component per durable subject.
    ///
    /// The spec says `PersistentMap`; `BTreeMap` is the interpretive stand-in
    /// (persistence is a deferred optimization, v4 §7.9).
    pub components: BTreeMap<JurisdictionKey, BasisComponent>,
}

/// One selected input state for one durable subject (v4 §7.5, field-for-field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BasisComponent {
    /// An unsaved editor buffer as first-class working Holder. `epoch` prevents
    /// generation collisions across editor sessions; `generation` supplies
    /// exact ordering and LSP-compatible edit identity (v4 §7.5).
    BufferGeneration {
        /// Owning client.
        client: ClientId,
        /// Buffer within that client.
        buffer: BufferId,
        /// Session epoch (collision guard across restarts).
        epoch: SessionEpoch,
        /// Monotonic edit generation within the epoch.
        generation: u64,
        /// Content hash, computed incrementally or lazily; permits cache reuse
        /// when different generations contain identical bytes.
        content_hash: Option<ContentHash>,
        /// Hash of the durable file this buffer is based on, if any.
        base_file_hash: Option<ContentHash>,
    },
    /// Durable file bytes.
    FileContent {
        /// Workspace-relative path.
        path: PathId,
        /// Content hash of the file bytes.
        hash: ContentHash,
    },
    /// A Git object (published Holder in external-file profiles).
    GitCommit {
        /// Git object id.
        oid: ObjectId,
    },
    /// A graph-store snapshot revision.
    GraphSnapshot {
        /// Monotonic graph revision.
        revision: GraphRevisionId,
    },
    /// A content-addressed immutable object.
    ObjectContent {
        /// Content hash of the object bytes.
        hash: ContentHash,
    },
    /// An external service revision.
    ExternalRevision {
        /// The external source.
        source: SourceId,
        /// Opaque revision token from that source.
        token: RevisionToken,
    },
    /// A point-in-time observation of external state.
    Observation {
        /// The observed source.
        source: SourceId,
        /// When the observation was made.
        observed_at: Timestamp,
        /// Content hash of the observed value.
        hash: ContentHash,
    },
}

/// The stable Basis map key for the graph-store `GraphSnapshot` component
/// (AM-8.2). Queries that read graph state record this key so a new graph
/// revision invalidates exactly the computations that consulted the graph. The
/// reserved documented path `.liminal/graph` addresses the graph-as-Holder.
#[must_use]
pub fn graph_key() -> JurisdictionKey {
    JurisdictionKey::Path(PathId(".liminal/graph".into()))
}

/// One accepted causal frontier of a federated merge domain (v4 §7.6).
///
/// Opaque until a declared merge runtime exists — Jurisdiction names and
/// constrains that runtime; it never replaces it (R4 §11.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalFrontier(pub Vec<u8>);
