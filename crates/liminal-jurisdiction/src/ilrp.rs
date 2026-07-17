//! The Intent-Logged Repair Protocol (v4 §7.8; R4 §7).
//!
//! A file and a graph store cannot share one ordinary atomic transaction.
//! ILRP is a crash-consistent saga, not two-phase commit. Its guarantee:
//!
//! ```text
//! at-least-once idempotent external application
//! + exactly-once accepted graph finalization
//! + durable evidence of every partial state
//! ```
//!
//! Recovery never guesses (v4 §7.8 step 5): prestate → resume; poststate →
//! acknowledge and continue; neither → stop and preserve contested Overlays.

use std::collections::BTreeMap;

use liminal_graph::GraphStore;
use liminal_id::{RepairId, RepairStepId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::repair::{ProposedMutation, RepairPlan, SafetyEvidence, StatePredicate};

/// Intent lifecycle (v4 §7.8, verbatim state diagram):
///
/// ```text
/// Prepared → Applying → ExternalApplied → Finalizing → Committed
///                      ↘ NeedsReview
/// Prepared/Applying    ↘ Aborted
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntentState {
    /// Durable intent recorded; nothing external touched yet.
    Prepared,
    /// External steps executing in topological order.
    Applying,
    /// Every external step applied and acknowledged.
    ExternalApplied,
    /// Graph finalization in progress.
    Finalizing,
    /// Terminal: graph mutations and intent completion committed together.
    Committed,
    /// Terminal-until-human: the world matched neither prestate nor poststate;
    /// work preserved as contested Overlays. Never guessed past.
    NeedsReview,
    /// Terminal: abandoned before external effects, or explicitly discarded.
    Aborted,
}

impl IntentState {
    /// Whether `self → next` is a legal transition (v4 §7.8 diagram).
    #[must_use]
    pub fn may_transition_to(self, next: IntentState) -> bool {
        use IntentState::{
            Aborted, Applying, Committed, ExternalApplied, Finalizing, NeedsReview, Prepared,
        };
        matches!(
            (self, next),
            (Prepared, Applying)
                | (Applying, ExternalApplied)
                | (ExternalApplied, Finalizing)
                | (Finalizing, Committed)
                | (
                    Prepared | Applying | ExternalApplied | Finalizing,
                    NeedsReview
                )
                | (Prepared | Applying, Aborted)
        )
    }

    /// Whether this state is terminal (crash recovery must always reach one:
    /// `Committed`, `NeedsReview`, or `Aborted` — R4 §10).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            IntentState::Committed | IntentState::NeedsReview | IntentState::Aborted
        )
    }
}

/// A durable repair intent: the plan plus its progress evidence, persisted in
/// the graph store's aux namespace `ilrp.intent` (v4 §92: the graph store is
/// the coordinator).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairIntent {
    /// The plan being executed.
    pub plan: RepairPlan,
    /// Current lifecycle state.
    pub state: IntentState,
    /// The safety evidence that authorized execution (R4 §6).
    pub evidence: SafetyEvidence,
    /// Acknowledged steps and their observed poststates (v4 §7.8 step 3).
    pub acks: BTreeMap<RepairStepId, StepAck>,
}

/// Durable acknowledgement of one completed external step (v4 §7.8 step 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepAck {
    /// The step.
    pub step: RepairStepId,
    /// What was actually observed after applying.
    pub observed_poststate: StatePredicate,
    /// When.
    pub at: Timestamp,
}

/// What the world looked like when a step's subject was inspected
/// (v4 §7.8 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrestateMatch {
    /// Matches the expected prestate → the step may run (or resume).
    Prestate,
    /// Matches the expected poststate → already applied; acknowledge and move on.
    Poststate,
    /// Matches neither → stop, preserve contested Overlays, never guess.
    Neither,
}

/// Executes external repair steps (files, services). Implementations MUST be
/// idempotent: repeating a completed step is harmless or detected as already
/// applied (v4 §7.8 step 3).
pub trait ExternalExecutor {
    /// Inspect the step's subject and classify the observed state.
    fn verify(&self, mutation: &ProposedMutation) -> Result<PrestateMatch, IlrpError>;
    /// Apply the step and report the observed poststate.
    fn apply(&self, mutation: &ProposedMutation) -> Result<StepAck, IlrpError>;
}

/// The five durable boundaries after which Phase -1 kills the process
/// (R4 §10). Per-step boundaries fire once per step in topological order;
/// the harness disambiguates by occurrence count, not name explosion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrashPoint {
    /// After the Prepare transaction commits.
    AfterIntentCommit,
    /// After one external step's effect lands (per step).
    AfterExternalApply,
    /// After one step's acknowledgement persists (per step).
    AfterAcknowledge,
    /// Immediately before the Finalize transaction.
    BeforeFinalize,
    /// After Finalize commits but before any notification.
    AfterFinalizeBeforeNotify,
}

impl CrashPoint {
    /// Stable name used in `LIMINAL_CRASHPOINT` env arming and hit-trace files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::AfterIntentCommit => "ilrp/after_intent_commit",
            Self::AfterExternalApply => "ilrp/after_external_apply",
            Self::AfterAcknowledge => "ilrp/after_ack",
            Self::BeforeFinalize => "ilrp/before_finalize",
            Self::AfterFinalizeBeforeNotify => "ilrp/after_finalize_before_notify",
        }
    }

    /// Every boundary, for harness enumeration (a registered point that never
    /// fires in a baseline run is a dead, untested boundary — a finding).
    #[must_use]
    pub fn all() -> [CrashPoint; 5] {
        [
            Self::AfterIntentCommit,
            Self::AfterExternalApply,
            Self::AfterAcknowledge,
            Self::BeforeFinalize,
            Self::AfterFinalizeBeforeNotify,
        ]
    }
}

/// Injected process death at durable boundaries. The real injector
/// (`liminal-daemon`) appends the hit to a trace file then calls
/// `std::process::abort()` — a real SIGABRT with no unwinding, so fsync
/// honesty is actually tested.
pub trait CrashInjector {
    /// Called at every durable boundary, in order.
    fn crash_if_armed(&self, at: CrashPoint);
}

/// The no-op injector for production paths and sound baseline runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCrash;

impl CrashInjector for NoCrash {
    fn crash_if_armed(&self, _at: CrashPoint) {}
}

/// The ILRP interpreter (v4 §7.8). ONE driver serves save, sync, merge,
/// recovery, writeback, and Promotion (R4 §4).
#[derive(Debug)]
pub struct IlrpDriver<'s, X, C> {
    /// The coordinating store (holds intents in aux ns `ilrp.intent`).
    pub store: &'s GraphStore,
    /// External-world executor.
    pub executor: X,
    /// Crash injection at durable boundaries.
    pub crash: C,
}

impl<X: ExternalExecutor, C: CrashInjector> IlrpDriver<'_, X, C> {
    /// **Prepare** (v4 §7.8 step 1): in ONE graph transaction, record the
    /// plan, captured Basis, dependency DAG, expected pre/poststates,
    /// idempotency keys, and safety evidence. The intent enters `Prepared`.
    pub fn prepare(
        &self,
        plan: RepairPlan,
        evidence: SafetyEvidence,
    ) -> Result<RepairId, IlrpError> {
        let _ = (plan, evidence);
        todo!("Phase -1 M2: ILRP Prepare (v4 §7.8 step 1)")
    }

    /// **Apply → Acknowledge → Finalize** (v4 §7.8 steps 2–4): execute steps
    /// in topological order with prestate verification; persist each ack;
    /// commit graph mutations and intent completion in one transaction. The
    /// accepted Basis advances only at finalization.
    pub fn run(&self, repair: RepairId) -> Result<IntentState, IlrpError> {
        let _ = repair;
        todo!("Phase -1 M2: ILRP Apply/Acknowledge/Finalize (v4 §7.8 steps 2–4)")
    }

    /// **Recover** (v4 §7.8 step 5): scan nonterminal intents on restart.
    /// Prestate → resume; poststate → acknowledge and continue; neither →
    /// `NeedsReview` with contested Overlays. Never guess. Running recovery
    /// twice must be a no-op (crash-matrix idempotence assertion).
    pub fn recover_all(&self) -> Result<Vec<(RepairId, IntentState)>, IlrpError> {
        todo!("Phase -1 M2: ILRP Recover (v4 §7.8 step 5)")
    }
}

/// ILRP failure.
#[derive(Debug, thiserror::Error)]
pub enum IlrpError {
    /// The store rejected a coordinator transaction.
    #[error(transparent)]
    Store(#[from] liminal_graph::StoreError),
    /// A step's observed state matched neither prestate nor poststate.
    #[error("step {step} observed contested state; preserved for review")]
    Contested {
        /// The contested step.
        step: RepairStepId,
    },
    /// An illegal lifecycle transition was attempted (bug, not world state).
    #[error("illegal intent transition {from:?} -> {to:?}")]
    IllegalTransition {
        /// Current state.
        from: IntentState,
        /// Attempted state.
        to: IntentState,
    },
    /// The plan's dependency graph is not a DAG.
    #[error(transparent)]
    Cycle(#[from] crate::repair::CycleError),
    /// External execution failed in a way that is not a state mismatch.
    #[error("external executor: {0}")]
    Executor(String),
}

#[cfg(test)]
mod tests {
    use super::IntentState::{
        Aborted, Applying, Committed, ExternalApplied, Finalizing, NeedsReview, Prepared,
    };
    use super::*;

    const ALL: [IntentState; 7] = [
        Prepared,
        Applying,
        ExternalApplied,
        Finalizing,
        Committed,
        NeedsReview,
        Aborted,
    ];

    #[test]
    fn happy_path_is_legal() {
        assert!(Prepared.may_transition_to(Applying));
        assert!(Applying.may_transition_to(ExternalApplied));
        assert!(ExternalApplied.may_transition_to(Finalizing));
        assert!(Finalizing.may_transition_to(Committed));
    }

    #[test]
    fn needs_review_reachable_from_every_nonterminal_state() {
        for s in [Prepared, Applying, ExternalApplied, Finalizing] {
            assert!(s.may_transition_to(NeedsReview), "{s:?} -> NeedsReview");
        }
    }

    #[test]
    fn abort_only_before_external_effects_complete() {
        assert!(Prepared.may_transition_to(Aborted));
        assert!(Applying.may_transition_to(Aborted));
        // Once external effects fully landed, the only exits are Finalize or review:
        assert!(!ExternalApplied.may_transition_to(Aborted));
        assert!(!Finalizing.may_transition_to(Aborted));
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for terminal in [Committed, NeedsReview, Aborted] {
            assert!(terminal.is_terminal());
            for next in ALL {
                assert!(
                    !terminal.may_transition_to(next),
                    "{terminal:?} must not transition to {next:?}"
                );
            }
        }
    }

    #[test]
    fn no_skipping_forward() {
        assert!(!Prepared.may_transition_to(ExternalApplied));
        assert!(!Prepared.may_transition_to(Finalizing));
        assert!(!Prepared.may_transition_to(Committed));
        assert!(!Applying.may_transition_to(Finalizing));
        assert!(!Applying.may_transition_to(Committed));
        assert!(!ExternalApplied.may_transition_to(Committed));
    }

    #[test]
    fn no_moving_backward() {
        assert!(!Applying.may_transition_to(Prepared));
        assert!(!ExternalApplied.may_transition_to(Applying));
        assert!(!Finalizing.may_transition_to(ExternalApplied));
    }
}
