//! Holders: the source currently selected to provide or accept a piece of
//! information (v4 §7.1, §7.6 matrix).

use liminal_id::{BufferId, ClientId, ContentHash, ObjectId, PathId, SourceId};
use serde::{Deserialize, Serialize};

/// The source selected to provide or accept a governed subject's state
/// (v4 §7.1 advanced vocabulary; routine UI says file/document/repository/
/// service/device instead, R4 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Holder {
    /// Durable file bytes.
    File {
        /// Workspace-relative path.
        path: PathId,
    },
    /// A client's dirty editor buffer as *working* Holder (v4 §7.5).
    Buffer {
        /// The owning client.
        client: ClientId,
        /// The buffer.
        buffer: BufferId,
    },
    /// The graph store itself (graph-native profile, v4 §7.6).
    Graph,
    /// A content-addressed immutable object (immutable-resource profile).
    Object {
        /// The object hash.
        hash: ContentHash,
    },
    /// A Git object (published Holder in external-file profiles).
    GitObject {
        /// The object id.
        oid: ObjectId,
    },
    /// An external service or database (external-service profile).
    Service {
        /// The source identity.
        source: SourceId,
    },
}

/// A *named* merge runtime for federated domains.
///
/// Jurisdiction names and constrains the runtime; it never replaces the CRDT,
/// OT, Git, database, or domain-specific algorithm (v4 §7.6; R4 §11.7). Never
/// implemented in this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeRuntimeRef(pub String);
