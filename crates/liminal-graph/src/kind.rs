//! Kind constants for the toy world (D03.2).

use crate::KindId;

/// Unset / unknown kind.
pub const UNSET: KindId = KindId(0);
/// A file-level container node.
pub const FILE: KindId = KindId(1);
/// A paragraph block within a file.
pub const PARAGRAPH: KindId = KindId(2);
/// A graph-native comment Relation.
pub const COMMENT: KindId = KindId(3);
/// A node holding a `MaterializeExternal` observation (M08.6, v4 §8.6). Toy:
/// at most one per scenario, mirroring the single FILE node convention.
pub const EXTERNAL_VALUE: KindId = KindId(4);
