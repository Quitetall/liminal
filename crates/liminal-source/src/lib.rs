//! File-Holder mechanics for ILRP file steps (v4 §7.8 step 2: verify expected
//! hash → produce staged content → flush → atomic replacement → observe
//! resulting hash) and source ranges (v4 §5.3, §22).
//!
//! Staged files use deterministic `.lim-staged` sibling names — deliberately
//! not `tempfile` — so crash recovery can *scan* for abandoned staging
//! (R4 §7.5 Recover).

mod file;
mod range;

pub use file::{FileObservation, StageError, StagedWrite, observe, scan_staged, stage};
pub use range::SourceRange;
