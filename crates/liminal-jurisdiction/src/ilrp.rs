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
use liminal_graph::{CoordinatorWriter, GraphStore, Origin, TxnMeta};
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
    /// Durable intent recorded; nothing external touched yet (R4 §7.1).
    Prepared,
    /// External steps executing in topological order (R4 §7.2).
    Applying,
    /// Every external step applied and acknowledged (R4 §7.3).
    ExternalApplied,
    /// Graph finalization in progress (R4 §7.4).
    Finalizing,
    /// Terminal: graph mutations and intent completion committed together (R4 §7.4).
    Committed,
    /// Terminal-until-human: the world matched neither prestate nor poststate;
    /// work preserved as contested Overlays. Never guessed past (R4 §7.5).
    NeedsReview,
    /// Terminal: abandoned before external effects, or explicitly discarded (R4 §7.5).
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
    store: &'s GraphStore,
    coordinator: CoordinatorWriter<'s>,
    /// External-world executor.
    executor: X,
    /// Crash injection at durable boundaries.
    crash: C,
}

/// Accepted graph-finalization evidence reconstructed from the log (R4 §7.4).
#[derive(Debug)]
pub struct CommittedRepair<'s> {
    store: &'s GraphStore,
    plan: RepairPlan,
    evidence: SafetyEvidence,
    resulting_basis: liminal_revision::WorkspaceBasis,
    applied_steps: Vec<RepairStepId>,
    repair: RepairId,
    revision: liminal_id::GraphRevisionId,
    transaction: liminal_id::TransactionId,
}

impl CommittedRepair<'_> {
    /// Whether this receipt belongs to this exact live store instance (v4 §7.8).
    pub fn belongs_to(&self, store: &GraphStore) -> bool {
        std::ptr::eq(self.store, store)
    }
    /// The immutable plan verified across the accepted intent history (R4 §7.5).
    pub fn plan(&self) -> &RepairPlan {
        &self.plan
    }
    /// Admission evidence from that same history, not replacement caller data (R4 §6).
    pub fn evidence(&self) -> &SafetyEvidence {
        &self.evidence
    }
    /// Finalized graph revision plus acknowledged external observations (R4 §7.3; R4 §7.4). This
    /// is a historical vector, not a claim that external files cannot change.
    pub fn resulting_basis(&self) -> &liminal_revision::WorkspaceBasis {
        &self.resulting_basis
    }
    /// External steps in topological order, then graph steps at finalization (R4 §7.2; R4 §7.4).
    pub fn applied_steps(&self) -> &[RepairStepId] {
        &self.applied_steps
    }
    /// Repair whose graph effects committed atomically with its terminal state (R4 §7.4).
    pub fn repair(&self) -> RepairId {
        self.repair
    }
    /// Accepted revision of that transaction (R4 §7.4).
    pub fn revision(&self) -> liminal_id::GraphRevisionId {
        self.revision
    }
    /// Identity from the accepted transaction, not a caller-supplied label (v4 §92).
    pub fn transaction(&self) -> liminal_id::TransactionId {
        self.transaction
    }
}

fn finalized_basis(
    intent: &RepairIntent,
    revision: liminal_id::GraphRevisionId,
    transaction: liminal_id::TransactionId,
) -> Result<liminal_revision::WorkspaceBasis, IlrpError> {
    use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};
    let mut components = intent.plan.basis.components.clone();
    components.retain(|_, component| !matches!(component, BasisComponent::BufferGeneration { .. }));
    components.insert(
        liminal_revision::graph_key(),
        BasisComponent::GraphSnapshot { revision },
    );
    for step in crate::repair::topo_order(&intent.plan)? {
        match &intent.acks[&step].observed_poststate {
            StatePredicate::FileContent { path, hash } => {
                components.insert(
                    liminal_id::JurisdictionKey::Path(path.clone()),
                    BasisComponent::FileContent {
                        path: path.clone(),
                        hash: *hash,
                    },
                );
            }
            StatePredicate::FileAbsent { path } => {
                components.remove(&liminal_id::JurisdictionKey::Path(path.clone()));
            }
            _ => {}
        }
    }
    Ok(WorkspaceBasis {
        transaction,
        perspective: BasisPerspective::DurableOnly,
        components,
    })
}

fn applied_order(plan: &RepairPlan) -> Result<Vec<RepairStepId>, crate::repair::CycleError> {
    let order = crate::repair::topo_order(plan)?;
    let (mut external, graph): (Vec<_>, Vec<_>) = order
        .into_iter()
        .partition(|id| !matches!(plan.steps[id].operation, RepairOperation::Graph(_)));
    external.extend(graph);
    Ok(external)
}

/// Runtime outcome; terminal state alone does not manufacture a receipt (R4 §7.4; R4 §7.5).
#[derive(Debug)]
pub struct RepairOutcome<'s> {
    state: IntentState,
    committed: Option<CommittedRepair<'s>>,
}

/// Ephemeral permission for exactly these graph effects at one checked head.
/// Never serialized or exposed to callers.
struct FinalizationPermit {
    repair: RepairId,
    head: liminal_id::GraphRevisionId,
    operations: Vec<liminal_graph::Operation>,
}

fn graph_predicate(view: &liminal_graph::StateView, predicate: &StatePredicate) -> bool {
    match predicate {
        StatePredicate::Any => true,
        StatePredicate::NodeAt { node, revision } => view
            .node(*node)
            .is_some_and(|value| value.revision == *revision),
        StatePredicate::RelationAt { relation, revision } => view
            .relation(*relation)
            .is_some_and(|value| value.revision == *revision),
        _ => false,
    }
}

impl FinalizationPermit {
    fn validate(
        store: &GraphStore,
        repair: RepairId,
        intent: &RepairIntent,
    ) -> Result<Self, IlrpError> {
        validate_intent(repair, intent)?;
        if intent.state != IntentState::Finalizing {
            return Err(IlrpError::Executor(
                "finalization requires Finalizing state".into(),
            ));
        }
        let head = store.head()?;
        if store.get_aux(AUX_NS_INTENT, &repair.to_string())? != Some(serde_json::to_value(intent)?)
        {
            return Err(liminal_graph::StoreError::Conflict(
                "intent changed before finalization".into(),
            )
            .into());
        }
        let mut operations = Vec::new();
        let mut view = store.state_at(head)?;
        for id in crate::repair::topo_order(&intent.plan)? {
            let step = &intent.plan.steps[&id];
            if let RepairOperation::Graph(operation) = &step.operation {
                if !graph_predicate(&view, &step.expected_prestate) {
                    return Err(liminal_graph::StoreError::Conflict(
                        "graph repair prestate changed".into(),
                    )
                    .into());
                }
                operations.push(operation.clone());
                view = store.preview_ops(&operations)?;
                if view.head() != head || !graph_predicate(&view, &step.expected_poststate) {
                    return Err(liminal_graph::StoreError::Conflict(
                        "graph repair poststate not established".into(),
                    )
                    .into());
                }
            }
        }
        if store.head()? != head {
            return Err(liminal_graph::StoreError::Conflict(
                "store changed during finalization validation".into(),
            )
            .into());
        }
        Ok(Self {
            repair,
            head,
            operations,
        })
    }
}

impl<'s> RepairOutcome<'s> {
    /// Observed ILRP lifecycle state (v4 §7.8).
    pub fn state(&self) -> IntentState {
        self.state
    }
    /// Present only for verified durable graph finalization (R4 §7.4).
    pub fn committed(&self) -> Option<&CommittedRepair<'s>> {
        self.committed.as_ref()
    }
}

/// Re-export for local readability.
const AUX_NS_INTENT: &str = ILRP_INTENT;

impl<'s, X: ExternalExecutor, C: CrashInjector> IlrpDriver<'s, X, C> {
    /// Assemble the coordinator from trusted store ownership (v4 §7.8).
    pub fn new(coordinator: CoordinatorWriter<'s>, executor: X, crash: C) -> Self {
        Self {
            store: coordinator.store(),
            coordinator,
            executor,
            crash,
        }
    }

    /// Read access does not convey transaction authority (v4 Law 3; v4 §7.3).
    pub fn store(&self) -> &'s GraphStore {
        self.store
    }

    /// Execute and return durable completion evidence for downstream bookkeeping (v4 §7.8).
    pub fn run_checked(&self, repair: RepairId) -> Result<RepairOutcome<'s>, IlrpError> {
        let state = self.run(repair)?;
        let committed = self.verify_history(repair)?;
        if (state == IntentState::Committed) != committed.is_some() {
            return Err(IlrpError::Executor(
                "intent state and durable receipt disagree".into(),
            ));
        }
        Ok(RepairOutcome { state, committed })
    }

    fn verify_history(&self, repair: RepairId) -> Result<Option<CommittedRepair<'s>>, IlrpError> {
        let fail = || {
            IlrpError::Store(liminal_graph::StoreError::Corrupt(
                "intent history does not prove checked admission and legal progress".into(),
            ))
        };
        let history = self
            .store
            .committed_aux_history(AUX_NS_INTENT, &repair.to_string())?;
        if history.is_empty() {
            return Err(fail());
        }
        let mut previous: Option<RepairIntent> = None;
        let mut previous_revision = None;
        let mut receipt = None;
        for record in &history {
            if previous_revision.is_some_and(|revision| record.revision <= revision) {
                return Err(fail());
            }
            previous_revision = Some(record.revision);
            let current: RepairIntent =
                serde_json::from_value(record.value.clone().ok_or_else(fail)?)?;
            validate_intent(repair, &current)?;
            let note = record.transaction.meta.provenance.as_deref();
            if let Some(old) = &previous {
                validate_progress(old, &current, note)?;
            } else if current.state != IntentState::Prepared
                || note != Some("ilrp:prepare:checked-v1")
            {
                // Legacy records remain untouched. They require explicit readmission;
                // an old DTO or arbitrary terminal label is not current authority.
                return Err(fail());
            }
            if current.state == IntentState::Committed {
                let operations = crate::repair::topo_order(&current.plan)?
                    .into_iter()
                    .filter_map(|id| match &current.plan.steps[&id].operation {
                        RepairOperation::Graph(op) => Some(op.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if operations != record.transaction.ops {
                    return Err(fail());
                }
                receipt = Some(CommittedRepair {
                    store: self.store,
                    plan: current.plan.clone(),
                    evidence: current.evidence.clone(),
                    resulting_basis: finalized_basis(
                        &current,
                        record.revision,
                        record.transaction.id,
                    )?,
                    applied_steps: applied_order(&current.plan)?,
                    repair,
                    revision: record.revision,
                    transaction: record.transaction.id,
                });
            } else if !record.transaction.ops.is_empty() {
                return Err(fail());
            }
            previous = Some(current);
        }
        if self.store.get_aux(AUX_NS_INTENT, &repair.to_string())?
            != history.last().and_then(|r| r.value.clone())
        {
            return Err(fail());
        }
        Ok(receipt)
    }

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
        let head = self.store.head()?;
        self.verify_history(id)?;
        let previous: RepairIntent = serde_json::from_value(
            self.store
                .get_aux(AUX_NS_INTENT, &key)?
                .ok_or_else(|| IlrpError::Executor("intent disappeared".into()))?,
        )?;
        validate_intent(id, intent)?;
        validate_progress(&previous, intent, Some(note))
            .map_err(|error| liminal_graph::StoreError::Conflict(error.to_string()))?;
        let mut txn = self.coordinator.begin()?.expect_head(head);
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
        validate_ack(intent, step_id, &ack)?;
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
        authorized: crate::admission::AuthorizedRepair<'s>,
    ) -> Result<RepairId, IlrpError> {
        let head = authorized.basis().revision();
        let (plan, evidence) = authorized
            .consume(self.store)
            .map_err(|error| IlrpError::Executor(error.to_string()))?;
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
        let mut txn = self.coordinator.begin()?.expect_head(head);
        txn.put_aux(AUX_NS_INTENT, &key, serde_json::to_value(&intent)?)?;
        self.crash.crash_if_armed(CrashPoint::BeforeIntentCommit);
        txn.commit(Self::meta("ilrp:prepare:checked-v1", Origin::Human))?;

        // Fault point: after the Prepare transaction commits.
        self.crash.crash_if_armed(CrashPoint::AfterIntentCommit);

        Ok(id)
    }

    /// Status-only compatibility Apply → Acknowledge → Finalize; bookkeeping requires `run_checked` (v4 §7.8).
    pub fn run(&self, repair: RepairId) -> Result<IntentState, IlrpError> {
        self.verify_history(repair)?;
        let key = repair.to_string();
        let value = self
            .store
            .get_aux(AUX_NS_INTENT, &key)?
            .ok_or(IlrpError::Store(liminal_graph::StoreError::NotFound(
                key.clone(),
            )))?;
        let mut intent: RepairIntent = serde_json::from_value(value)
            .map_err(|e| IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string())))?;
        validate_intent(repair, &intent)?;

        // Terminal → idempotent re-run.
        if intent.state.is_terminal() {
            return Ok(intent.state);
        }

        self.advance(repair, &mut intent, Origin::Human)
    }

    /// Status-only restart scan of nonterminal intents; receipts use `run_checked` (v4 §7.8 step 5).
    pub fn recover_all(&self) -> Result<Vec<(RepairId, IntentState)>, IlrpError> {
        let all = self.store.scan_aux(AUX_NS_INTENT)?;
        let mut results = Vec::new();

        for (key, value) in all {
            let id: RepairId = key.parse().map_err(|e: liminal_id::ParseIdError| {
                IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string()))
            })?;
            let mut intent: RepairIntent = serde_json::from_value(value)
                .map_err(|e| IlrpError::Store(liminal_graph::StoreError::Corrupt(e.to_string())))?;
            validate_intent(id, &intent)?;
            self.verify_history(id)?;

            if intent.state.is_terminal() {
                continue;
            }

            let state = self.advance(id, &mut intent, Origin::Recovery)?;
            results.push((id, state));
        }

        Ok(results)
    }

    /// Shared advance logic for run and recover_all (D02.1).
    // Keep the audited durable commit and crash boundaries in this one
    // interpreter; splitting finalization would relocate a frozen census site.
    #[allow(clippy::too_many_lines)]
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
            let permit = match FinalizationPermit::validate(self.store, id, intent) {
                Ok(permit) => permit,
                Err(IlrpError::Store(
                    liminal_graph::StoreError::Conflict(_) | liminal_graph::StoreError::NotFound(_),
                )) => {
                    intent.state = IntentState::NeedsReview;
                    self.commit_intent(id, intent, "state:needs-review", origin)?;
                    return Ok(IntentState::NeedsReview);
                }
                Err(error) => return Err(error),
            };
            let mut txn = self.coordinator.begin()?.expect_head(permit.head);
            for operation in permit.operations {
                txn.apply(operation)?;
            }

            intent.state = IntentState::Committed;
            txn.put_aux(
                AUX_NS_INTENT,
                &permit.repair.to_string(),
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

// One progress relation is used both before publication and during recovery.
fn validate_progress(
    old: &RepairIntent,
    current: &RepairIntent,
    note: Option<&str>,
) -> Result<(), IlrpError> {
    let fail = || {
        IlrpError::Store(liminal_graph::StoreError::Corrupt(
            "invalid intent progress".into(),
        ))
    };
    if old.plan != current.plan || old.evidence != current.evidence || old.state.is_terminal() {
        return Err(fail());
    }
    if current.state == old.state {
        if current.state != IntentState::Applying
            || current.acks.len() != old.acks.len() + 1
            || old
                .acks
                .iter()
                .any(|(id, ack)| current.acks.get(id) != Some(ack))
        {
            return Err(fail());
        }
        let (id, ack) = current
            .acks
            .iter()
            .find(|(id, _)| !old.acks.contains_key(id))
            .ok_or_else(fail)?;
        validate_ack(old, *id, ack)?;
        if note != Some(format!("ack:{id}").as_str()) {
            return Err(fail());
        }
    } else {
        if !old.state.may_transition_to(current.state) || old.acks != current.acks {
            return Err(fail());
        }
        let expected = match current.state {
            IntentState::Applying => "state:applying",
            IntentState::ExternalApplied => "state:external-applied",
            IntentState::Finalizing => "state:finalizing",
            IntentState::Committed => "ilrp:finalize",
            IntentState::NeedsReview => "state:needs-review",
            IntentState::Aborted => "state:aborted",
            IntentState::Prepared => return Err(fail()),
        };
        if note != Some(expected) {
            return Err(fail());
        }
    }
    Ok(())
}

// Structural admission of untrusted persisted DTOs. This does not establish
// provenance or subject-local authority; those require the owner/admission slice.
fn validate_intent(id: RepairId, intent: &RepairIntent) -> Result<(), IlrpError> {
    let corrupt =
        |message: &str| IlrpError::Store(liminal_graph::StoreError::Corrupt(message.to_owned()));
    if intent.plan.id != id {
        return Err(corrupt("intent key and repair identity disagree"));
    }
    crate::repair::topo_order(&intent.plan)?;
    if intent.plan.steps.iter().any(|(key, step)| *key != step.id) {
        return Err(corrupt("plan key and step identity disagree"));
    }
    for (step, ack) in &intent.acks {
        validate_ack(intent, *step, ack).map_err(|error| corrupt(&error.to_string()))?;
    }
    if intent.state == IntentState::Prepared && !intent.acks.is_empty() {
        return Err(corrupt("prepared intent already contains acknowledgements"));
    }
    if matches!(
        intent.state,
        IntentState::ExternalApplied | IntentState::Finalizing | IntentState::Committed
    ) && intent.acks.len() != intent.plan.steps.len()
    {
        return Err(corrupt("completed effects lack required acknowledgements"));
    }
    Ok(())
}

// Validate the executor's claim before it enters either memory or durable state.
// Equality checks bind the acknowledgement to the specific planned step, not
// merely to a terminal label or to a successful external call.
fn validate_ack(
    intent: &RepairIntent,
    step_id: RepairStepId,
    ack: &StepAck,
) -> Result<(), IlrpError> {
    let step = intent.plan.steps.get(&step_id).ok_or_else(|| {
        IlrpError::Executor(format!("acknowledgement names unknown step {step_id}"))
    })?;
    // UUID conversion preserves all 128 bits; poststate remains structural below.
    let key = step_id.as_uuid().as_u128();
    let planned = step.id.as_uuid().as_u128();
    let received = ack.step.as_uuid().as_u128();
    if !liminal_safety::acknowledgement_matches(key, planned, received, &[], &[]) {
        return Err(IlrpError::Executor(
            "acknowledgement step identity mismatch".into(),
        ));
    }
    if ack.observed_poststate != step.expected_poststate {
        return Err(IlrpError::Executor(
            "acknowledgement poststate mismatch".into(),
        ));
    }
    for dependency in &intent.plan.dependencies {
        if dependency.after != step_id {
            continue;
        }
        // Borrow one selected map key without allocating. The first call above
        // preserves identity-error precedence; this call checks each dependency.
        let acknowledged = intent
            .acks
            .get_key_value(&dependency.before)
            .map(|(identity, _)| identity.as_uuid().as_u128());
        if !liminal_safety::acknowledgement_matches(
            key,
            planned,
            received,
            &[dependency.before.as_uuid().as_u128()],
            acknowledged.as_slice(),
        ) {
            return Err(IlrpError::Executor(
                "acknowledgement precedes dependency".into(),
            ));
        }
    }
    Ok(())
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
