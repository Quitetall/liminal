//! Auxiliary namespace constants (protocol §7).
//!
//! Every aux-namespace string lives here — no literal at call sites.

/// ILRP intent records (v4 §7.8, §92).
pub const ILRP_INTENT: &str = "ilrp.intent";
/// Jurisdiction overlay records (v4 §7.10). Empty until M5.
pub const JUR_OVERLAY: &str = "jur.overlay";
/// Reconciliation queue items (R4 §9). Empty until M5.
pub const JUR_RECONCILE: &str = "jur.reconcile";
/// Jurisdiction alias records. Reserved.
pub const JUR_ALIAS: &str = "jur.alias";
/// Jurisdiction plan records. Reserved.
pub const JUR_PLAN: &str = "jur.plan";
/// Jurisdiction decision records. Reserved.
pub const JUR_DECISION: &str = "jur.decision";
/// Jurisdiction repair records. Reserved.
pub const JUR_REPAIR: &str = "jur.repair";
/// Overlay log entries. Reserved.
pub const JUR_OVERLAY_LOG: &str = "jur.overlay_log";
/// System blob storage. Reserved.
pub const SYS_BLOB: &str = "sys.blob";
/// System unavailable-holder records. Reserved.
pub const SYS_UNAVAILABLE: &str = "sys.unavailable";
/// System clock records. Reserved.
pub const SYS_CLOCK: &str = "sys.clock";
/// System epoch records. Reserved.
pub const SYS_EPOCH: &str = "sys.epoch";
