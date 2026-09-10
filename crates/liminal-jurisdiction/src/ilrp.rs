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

use liminal_graph::ns::ILRP_INTENT;
use liminal_graph::{GraphStore, Origin, TxnMeta};
use liminal_id::{RepairId, RepairStepId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::repair::{
    ProposedMutation, RepairOperation, RepairPlan, SafetyEvidence, StatePredicate,
};

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
    /// The state's stable name. M17.5 F-66 (A06): recovery's terminal states
    /// travel into crash evidence and are compared there against the
    /// protocol's table, so their spelling is a contract. `Debug` is not one —
    /// it is a derive that may be changed or replaced — and this is.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prepared => "Prepared",
            Self::Applying => "Applying",
            Self::ExternalApplied => "ExternalApplied",
            Self::Finalizing => "Finalizing",
            Self::Committed => "Committed",
            Self::NeedsReview => "NeedsReview",
            Self::Aborted => "Aborted",
        }
    }

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

/// Durable boundaries around ILRP transitions (R4 §10). Per-step boundaries
/// fire once per step in topological order; the harness disambiguates by
/// occurrence count, not name explosion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrashPoint {
    /// Immediately before the Prepare transaction commits.
    BeforeIntentCommit,
    /// After the Prepare transaction commits.
    AfterIntentCommit,
    /// Immediately before one external step's effect is requested (per step).
    BeforeExternalApply,
    /// After one external step's effect lands (per step).
    AfterExternalApply,
    /// Immediately before one step's acknowledgement persists (per step).
    BeforeAcknowledge,
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
            Self::BeforeIntentCommit => "ilrp/before_intent_commit",
            Self::AfterIntentCommit => "ilrp/after_intent_commit",
            Self::BeforeExternalApply => "ilrp/before_external_apply",
            Self::AfterExternalApply => "ilrp/after_external_apply",
            Self::BeforeAcknowledge => "ilrp/before_ack",
            Self::AfterAcknowledge => "ilrp/after_ack",
            Self::BeforeFinalize => "ilrp/before_finalize",
            Self::AfterFinalizeBeforeNotify => "ilrp/after_finalize_before_notify",
        }
    }

    /// Every boundary, for harness enumeration (a registered point that never
    /// fires in a baseline run is a dead, untested boundary — a finding).
    #[must_use]
    pub fn all() -> [CrashPoint; 8] {
        [
            Self::BeforeIntentCommit,
            Self::AfterIntentCommit,
            Self::BeforeExternalApply,
            Self::AfterExternalApply,
            Self::BeforeAcknowledge,
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

impl<C: CrashInjector> CrashInjector for &C {
    fn crash_if_armed(&self, at: CrashPoint) {
        (*self).crash_if_armed(at);
    }
}

impl<X: ExternalExecutor> ExternalExecutor for &X {
    fn verify(&self, mutation: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        (*self).verify(mutation)
    }
    fn apply(&self, mutation: &ProposedMutation) -> Result<StepAck, IlrpError> {
        (*self).apply(mutation)
    }
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

/// Re-export for local readability.
const AUX_NS_INTENT: &str = ILRP_INTENT;

impl<X: ExternalExecutor, C: CrashInjector> IlrpDriver<'_, X, C> {
    /// Build `TxnMeta` with a provenance string.
    fn meta(provenance: &str, origin: Origin) -> TxnMeta {
        TxnMeta {
            actor: None,
            origin,
            at: Timestamp::now(),
            provenance: Some(provenance.to_owned()),
            inverse: None,
        }
    }

    /// Commit the full intent to the aux store in one transaction.
    fn commit_intent(
        &self,
        id: RepairId,
        intent: &RepairIntent,
        note: &str,
        origin: Origin,
    ) -> Result<(), IlrpError> {
        let key = id.to_string();
        let mut txn = self.store.begin()?;
        txn.put_aux(AUX_NS_INTENT, &key, serde_json::to_value(intent)?)?;
        txn.commit(Self::meta(note, origin))?;
        Ok(())
    }

    fn acknowledge_step(
        &self,
        id: RepairId,
        intent: &mut RepairIntent,
        step_id: RepairStepId,
        ack: StepAck,
        origin: Origin,
    ) -> Result<(), IlrpError> {
        intent.acks.insert(step_id, ack);
        self.crash.crash_if_armed(CrashPoint::BeforeAcknowledge);
        self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;
        self.crash.crash_if_armed(CrashPoint::AfterAcknowledge);
        Ok(())
    }

    /// **Prepare** (v4 §7.8 step 1): in ONE graph transaction, record the
    /// plan, captured Basis, dependency DAG, expected pre/poststates,
    /// idempotency keys, and safety evidence. The intent enters `Prepared`.
    pub fn prepare(
        &self,
        plan: RepairPlan,
        evidence: SafetyEvidence,
    ) -> Result<RepairId, IlrpError> {
        // Validate DAG up front.
        let order = crate::repair::topo_order(&plan)?;

        // D04.4: Graph steps execute inside the Finalize transaction, AFTER all
        // external steps. A plan where any file/external step depends on a Graph
        // step is unexecutable under ILRP — refuse it here, before any effect.
        let graph_steps: std::collections::BTreeSet<RepairStepId> = plan
            .steps
            .iter()
            .filter(|(_, m)| matches!(m.operation, RepairOperation::Graph(_)))
            .map(|(id, _)| *id)
            .collect();
        for dep in &plan.dependencies {
            let after_is_external = !graph_steps.contains(&dep.after);
            if graph_steps.contains(&dep.before) && after_is_external {
                return Err(IlrpError::Executor(
                    "graph-before-file ordering cannot be executed under ILRP".into(),
                ));
            }
        }
        let _ = order;

        let id = plan.id;
        let key = id.to_string();

        // Duplicate check.
        if self.store.get_aux(AUX_NS_INTENT, &key)?.is_some() {
            return Err(IlrpError::Store(liminal_graph::StoreError::Conflict(
                format!("intent exists: {key}"),
            )));
        }

        let intent = RepairIntent {
            plan,
            state: IntentState::Prepared,
            evidence,
            acks: BTreeMap::new(),
        };

        // One transaction: persist the intent.
        let mut txn = self.store.begin()?;
        txn.put_aux(AUX_NS_INTENT, &key, serde_json::to_value(&intent)?)?;
        self.crash.crash_if_armed(CrashPoint::BeforeIntentCommit);
        txn.commit(Self::meta("ilrp:prepare", Origin::Human))?;

        // Fault point: after the Prepare transaction commits.
        self.crash.crash_if_armed(CrashPoint::AfterIntentCommit);

        Ok(id)
    }

    /// **Apply → Acknowledge → Finalize** (v4 §7.8 steps 2–4).
    pub fn run(&self, repair: RepairId) -> Result<IntentState, IlrpError> {
        let key = repair.to_string();
        let value = self
            .store
            .get_aux(AUX_NS_INTENT, &key)?
            .ok_or(IlrpError::Store(liminal_graph::StoreError::NotFound(
                key.clone(),
            )))?;
        let mut intent: RepairIntent = serde_json::from_value(value)
            .map_err(|e| IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string())))?;

        // Terminal → idempotent re-run.
        if intent.state.is_terminal() {
            return Ok(intent.state);
        }

        self.advance(repair, &mut intent, Origin::Human)
    }

    /// **Recover** (v4 §7.8 step 5): scan nonterminal intents on restart.
    pub fn recover_all(&self) -> Result<Vec<(RepairId, IntentState)>, IlrpError> {
        let all = self.store.scan_aux(AUX_NS_INTENT)?;
        let mut results = Vec::new();

        for (key, value) in all {
            let id: RepairId = key.parse().map_err(|e: liminal_id::ParseIdError| {
                IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string()))
            })?;
            let mut intent: RepairIntent = serde_json::from_value(value)
                .map_err(|e| IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string())))?;

            if intent.state.is_terminal() {
                continue;
            }

            let state = self.advance(id, &mut intent, Origin::Recovery)?;
            results.push((id, state));
        }

        Ok(results)
    }

    /// Shared advance logic for run and recover_all (D02.1).
    fn advance(
        &self,
        id: RepairId,
        intent: &mut RepairIntent,
        origin: Origin,
    ) -> Result<IntentState, IlrpError> {
        match intent.state {
            IntentState::Prepared => {
                // Transition to Applying.
                intent.state = IntentState::Applying;
                self.commit_intent(id, intent, "state:applying", origin)?;
            }
            // Already in Applying — continue from where we left off.
            IntentState::Applying | IntentState::Finalizing => {}
            IntentState::ExternalApplied => {
                // All external steps done — proceed to Finalizing.
                intent.state = IntentState::Finalizing;
                self.commit_intent(id, intent, "state:finalizing", origin)?;
            }
            _ => {
                return Err(IlrpError::IllegalTransition {
                    from: intent.state,
                    to: intent.state,
                });
            }
        }

        // Apply phase: execute steps in topological order.
        // Only runs when in Applying state (ExternalApplied skips this via the
        // match above transitioning to Finalizing).
        if intent.state == IntentState::Applying {
            let order = crate::repair::topo_order(&intent.plan)?;

            for step_id in &order {
                // Already acked → skip.
                if intent.acks.contains_key(step_id) {
                    continue;
                }

                let step = &intent.plan.steps[step_id];

                // Graph steps short-circuit: verify is Prestate, apply is no-op ack.
                if matches!(step.operation, RepairOperation::Graph(_)) {
                    let ack = StepAck {
                        step: *step_id,
                        observed_poststate: step.expected_poststate.clone(),
                        at: Timestamp::now(),
                    };
                    self.acknowledge_step(id, intent, *step_id, ack, origin)?;
                    continue;
                }

                // External step: verify prestate.
                match self.executor.verify(step)? {
                    PrestateMatch::Prestate => {
                        // Apply.
                        self.crash.crash_if_armed(CrashPoint::BeforeExternalApply);
                        let ack = self.executor.apply(step)?;
                        self.crash.crash_if_armed(CrashPoint::AfterExternalApply);
                        self.acknowledge_step(id, intent, *step_id, ack, origin)?;
                    }
                    PrestateMatch::Poststate => {
                        // Already applied; synthesize ack.
                        let ack = StepAck {
                            step: *step_id,
                            observed_poststate: step.expected_poststate.clone(),
                            at: Timestamp::now(),
                        };
                        self.acknowledge_step(id, intent, *step_id, ack, origin)?;
                    }
                    PrestateMatch::Neither => {
                        // Contested — stop and preserve.
                        intent.state = IntentState::NeedsReview;
                        self.commit_intent(id, intent, "state:needs-review", origin)?;
                        return Ok(IntentState::NeedsReview);
                    }
                }
            }

            // All steps applied.
            intent.state = IntentState::ExternalApplied;
            self.commit_intent(id, intent, "state:external-applied", origin)?;
        }

        // Finalize phase.
        if intent.state == IntentState::ExternalApplied {
            intent.state = IntentState::Finalizing;
            self.commit_intent(id, intent, "state:finalizing", origin)?;
        }

        if intent.state == IntentState::Finalizing {
            // Fault point: immediately before the Finalize transaction.
            self.crash.crash_if_armed(CrashPoint::BeforeFinalize);

            // Build ONE txn: graph ops + state=committed.
            let order = crate::repair::topo_order(&intent.plan)?;
            let mut txn = self.store.begin()?;

            for step_id in &order {
                let step = &intent.plan.steps[step_id];
                if let RepairOperation::Graph(op) = &step.operation {
                    txn.apply(op.clone())?;
                }
            }

            intent.state = IntentState::Committed;
            txn.put_aux(
                AUX_NS_INTENT,
                &id.to_string(),
                serde_json::to_value(&intent)?,
            )?;

            match txn.commit(Self::meta("ilrp:finalize", origin)) {
                Ok(_) => {}
                Err(
                    liminal_graph::StoreError::Conflict(_) | liminal_graph::StoreError::NotFound(_),
                ) => {
                    // Graph ops invalid against current state — stop and
                    // preserve for review (M02 Algorithm B §5).
                    intent.state = IntentState::NeedsReview;
                    self.commit_intent(id, intent, "state:needs-review", origin)?;
                    return Ok(IntentState::NeedsReview);
                }
                Err(e) => return Err(e.into()),
            }

            // Fault point: after Finalize commits, before notification.
            self.crash
                .crash_if_armed(CrashPoint::AfterFinalizeBeforeNotify);

            return Ok(IntentState::Committed);
        }

        Ok(intent.state)
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
    /// Serialization/deserialization of an intent record failed.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
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
