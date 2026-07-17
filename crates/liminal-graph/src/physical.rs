//! RESERVED physical-layout vocabulary (v4 §43–44).
//!
//! Phase -1 forbids this optimization (v4 Part XXII Phase -1 preamble, §125).
//! These types exist so the constitutional vocabulary has a compiled,
//! greppable home; nothing in the toy store uses them.

use liminal_id::{KindId, RevisionId};

use crate::node::NodeFlags;

/// Hot-path node header layout (v4 §44, verbatim sketch). RESERVED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeHeader {
    /// Node kind.
    pub kind: KindId,
    /// Which specialized store holds the payload (v4 §43).
    pub storage_class: StorageClass,
    /// Physical flags.
    pub flags: NodeFlags,
    /// Physical revision.
    pub revision: RevisionId,
}

/// Which specialized physical store a payload lives in (v4 §43). RESERVED.
///
/// The specialized stores of v4 §43 (text, ordered container, scalar,
/// resource, table, expression, ink, media timeline) arrive post-Phase-1;
/// the toy is generic-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    /// The generic fallback store — the only class the toy uses.
    Generic,
}
