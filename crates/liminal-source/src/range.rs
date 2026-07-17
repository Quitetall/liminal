//! Byte ranges within one exact source basis.

use serde::{Deserialize, Serialize};

/// A byte range within one exact source/resource basis (v4 §5.3, §22).
///
/// Cached convenience only: raw byte offsets may be cached but must never be
/// the sole persistent representation of an anchor (v4 §5.3). Revision-aware
/// anchors are a Phase -1.1/-1.2 experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceRange {
    /// Inclusive start byte offset.
    pub start: u64,
    /// Exclusive end byte offset.
    pub end: u64,
}

impl SourceRange {
    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}
