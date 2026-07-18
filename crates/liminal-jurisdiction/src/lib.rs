//! Jurisdiction: the deliberate complexity sink, in one typed home.
//!
//! This crate is an amendment to the v4 §117 crate list, recorded as
//! `spec/rfc/0001`: Revision 4 promoted RepairPlan, ILRP, and the
//! Reconciliation Queue to constitutional machinery, and R4's final law says
//! Jurisdiction gives the ownership/repair/synchronization problems "one typed
//! home". No other subsystem may create an independent ownership model (v4 §7).
//!
//! Repair and ILRP are **modules here, not separate crates**: R4 §4 requires
//! ONE interpreter for save, synchronization, reattachment, merge, recovery,
//! writeback, and Promotion, sharing the checker's Contract types. A crate
//! seam there would invite parallel lifecycle systems — the exact bug
//! Revision 4 removed.
//!
//! Everything is interpretive reference semantics. `CompiledJurisdictionPlan`,
//! indexed dispatch, and generated policy code are forbidden until Phase -1
//! and Phase 0 freeze observable semantics (v4 §7.9, §125).
//!
//! Spec: v4 §7.2–7.11; R4 §4–9.

pub mod blob;
pub mod checker;
pub mod contract;
pub mod explain;
pub mod holder;
pub mod ilrp;
pub mod overlay;
pub mod profile;
pub mod reconcile;
pub mod repair;

pub use checker::{
    AuthorizationReport, CheckReport, Checker, CheckerError, Finding, codes, everyday_rendering,
};
pub use contract::{
    ContinuityPolicy, DanglingPolicy, ForeignEditPolicy, HolderResolution, JurisdictionContract,
    LifecyclePolicy, MutationPolicy, RepairAuthorization, SafetyRequirement, SubjectSelector,
};
pub use explain::{Explanation, explain, render_holder};
pub use holder::{Holder, MergeRuntimeRef};
pub use ilrp::{
    CrashInjector, CrashPoint, ExternalExecutor, IlrpDriver, IlrpError, IntentState, NoCrash,
    PrestateMatch, RepairIntent, StepAck,
};
pub use overlay::{Overlay, OverlayState};
pub use profile::{
    ExternalFileProfile, GraphNativeProfile, JurisdictionProfile, ProfileId, ProfileSet, grade_of,
};
pub use reconcile::{ReconciliationItem, ReconciliationQueue, ReconciliationStatus};
pub use repair::{
    CycleError, InverseRepairPlan, ProposedMutation, RepairDecision, RepairDependency,
    RepairOperation, RepairPlan, RepairRecord, ReviewReason, SafetyEvidence, StatePredicate,
    UndoBlocked, plan_undo, topo_order,
};
