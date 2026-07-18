//! Repair plans: ordered dependency DAGs of mutation-local proposals
//! (v4 §7.7; R4 §5–6). Promotion is not a mechanism — it is the user-facing
//! name for a repair whose accepted result places an Overlay under its
//! intended durable Holder (Law 3C/3G; R4 §4).

use std::collections::{BTreeMap, BTreeSet};

use liminal_id::{
    ActorId, ContentHash, EntityId, IdempotencyKey, JurisdictionSubject, NodeId, PathId,
    RelationId, RepairId, RepairStepId, RevisionId, RevisionToken, SourceId, Timestamp,
};
use liminal_revision::WorkspaceBasis;
use liminal_source::SourceRange;
use serde::{Deserialize, Serialize};

/// A repair as a dependency DAG, never an unordered list (R4 §5; v4 §7.7,
/// verbatim sketch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairPlan {
    /// Plan identity.
    pub id: RepairId,
    /// The captured Basis this plan is valid against (Law 3D).
    pub basis: WorkspaceBasis,
    /// The proposed mutations, keyed by step.
    pub steps: BTreeMap<RepairStepId, ProposedMutation>,
    /// Ordering constraints (`before` must complete before `after`).
    pub dependencies: Vec<RepairDependency>,
    /// Inverse plan for one-command revert, when constructible (R4 §6).
    pub inverse: Option<InverseRepairPlan>,
}

/// One proposed mutation, governed by ITS OWN subject's Jurisdiction — no
/// Contract commandeers another subject (Law 3F; v4 §7.7, verbatim sketch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedMutation {
    /// Step identity.
    pub id: RepairStepId,
    /// The governed subject this mutation changes.
    pub subject: JurisdictionSubject,
    /// The operation.
    pub operation: RepairOperation,
    /// Expected state before applying (ILRP verifies; mismatch = stop).
    pub expected_prestate: StatePredicate,
    /// Expected state after applying (ILRP acknowledges against this).
    pub expected_poststate: StatePredicate,
    /// Idempotency key for at-least-once external application (v4 §7.8).
    pub idempotency_key: IdempotencyKey,
}

/// One ordering edge in the repair DAG (v4 §7.7, verbatim sketch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairDependency {
    /// Must complete first.
    pub before: RepairStepId,
    /// May run only after `before` succeeds.
    pub after: RepairStepId,
}

/// The inverse of a repair plan (R4 §6). Undo against *changed* state is a new
/// Basis-checked plan via [`plan_undo`], never unconditional time reversal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InverseRepairPlan(pub Box<RepairPlan>);

/// What a repair step does. SHAPE PROVISIONAL — grows with domains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairOperation {
    /// A graph mutation, applied at ILRP Finalize.
    Graph(liminal_graph::Operation),
    /// Replace a file's bytes (staged + atomic, v4 §7.8 step 2).
    WriteFile {
        /// Target file.
        path: PathId,
        /// Full new contents.
        contents: Vec<u8>,
    },
    /// Insert a durable ID into source bytes — step 1 of the canonical
    /// two-step DAG: ID insertion must precede Relation reattachment
    /// (v4 §7.7 example; R4 §5).
    InsertSourceId {
        /// Target file.
        path: PathId,
        /// Where to insert.
        at: SourceRange,
        /// The entity whose ID is being serialized.
        entity: EntityId,
    },
    /// An external-service request with idempotency (v4 §7.8; R4 §11.2).
    External {
        /// The service.
        source: SourceId,
        /// Opaque request description.
        request: String,
        /// Idempotency key when the service supports one.
        idempotency: IdempotencyKey,
    },
}

/// Expected pre/post state of a repair step. SHAPE PROVISIONAL (v4 §7.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatePredicate {
    /// The file at `path` hashes to `hash`.
    FileContent {
        /// The file.
        path: PathId,
        /// Its expected content hash.
        hash: ContentHash,
    },
    /// The file at `path` does not exist.
    FileAbsent {
        /// The file.
        path: PathId,
    },
    /// The Node is at the given physical revision.
    NodeAt {
        /// The node.
        node: NodeId,
        /// Its expected revision.
        revision: RevisionId,
    },
    /// The Relation is at the given physical revision.
    RelationAt {
        /// The relation.
        relation: RelationId,
        /// Its expected revision.
        revision: RevisionId,
    },
    /// The external source is at the given revision token.
    ExternalRevision {
        /// The source.
        source: SourceId,
        /// Its expected revision token.
        token: RevisionToken,
    },
    /// No expectation (used sparingly; ILRP still records what it observed).
    Any,
}

/// Evidence that an automatic repair is *safe*, not merely unique (R4 §6:
/// "determinism is not safety").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SafetyEvidence {
    /// The affected semantic Nodes are structurally disjoint.
    StructurallyDisjoint {
        /// Human-readable description of the disjointness argument.
        description: String,
    },
    /// A named domain validator proved non-interference.
    DomainValidator {
        /// The validator.
        validator: String,
        /// Its report.
        report: String,
    },
    /// A human explicitly approved.
    HumanApproval {
        /// Who.
        actor: ActorId,
        /// When.
        at: Timestamp,
    },
}

/// The checker's verdict on a plan. Deliberately has NO `Reject` variant:
/// capture is never rejected (Law 3B) — the only outcomes are automatic
/// application and review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairDecision {
    /// Every conjunctive condition of v4 §7.7 holds.
    AutoApply {
        /// The safety evidence that justified automation.
        evidence: SafetyEvidence,
    },
    /// Preserved as Overlays plus one coalesced reconciliation item (Law 3F).
    NeedsReview {
        /// Why automation was refused.
        reasons: Vec<ReviewReason>,
    },
}

/// One human-readable reason automation was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewReason(pub String);

/// The decision record of an automatically accepted repair (R4 §6, all six
/// evidence fields). Must support `lim repair undo <repair-id>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairRecord {
    /// The repair.
    pub repair: RepairId,
    /// Input Basis.
    pub input_basis: WorkspaceBasis,
    /// The selected rule.
    pub selected_rule: String,
    /// The safety evidence.
    pub evidence: SafetyEvidence,
    /// The applied steps, in execution order.
    pub applied_steps: Vec<RepairStepId>,
    /// Resulting Basis (advances only at ILRP Finalize).
    pub resulting_basis: WorkspaceBasis,
    /// Inverse plan or preimages for revert.
    pub inverse: Option<RepairPlan>,
}

/// Topologically order a plan's steps, respecting every dependency edge.
///
/// Deterministic: ready steps run in `RepairStepId` order, so the same plan
/// always schedules identically (required for replayable crash matrices).
pub fn topo_order(plan: &RepairPlan) -> Result<Vec<RepairStepId>, CycleError> {
    // Kahn's algorithm over BTree structures for deterministic tie-breaking.
    let mut indegree: BTreeMap<RepairStepId, usize> =
        plan.steps.keys().map(|id| (*id, 0)).collect();
    let mut edges: BTreeMap<RepairStepId, BTreeSet<RepairStepId>> = BTreeMap::new();

    for dep in &plan.dependencies {
        if !plan.steps.contains_key(&dep.before) || !plan.steps.contains_key(&dep.after) {
            return Err(CycleError::UnknownStep {
                before: dep.before,
                after: dep.after,
            });
        }
        if edges.entry(dep.before).or_default().insert(dep.after) {
            *indegree.entry(dep.after).or_default() += 1;
        }
    }

    let mut ready: BTreeSet<RepairStepId> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(plan.steps.len());

    while let Some(next) = ready.iter().next().copied() {
        ready.remove(&next);
        order.push(next);
        if let Some(successors) = edges.get(&next) {
            for succ in successors {
                let d = indegree
                    .get_mut(succ)
                    .expect("successor registered in indegree map");
                *d -= 1;
                if *d == 0 {
                    ready.insert(*succ);
                }
            }
        }
    }

    if order.len() == plan.steps.len() {
        Ok(order)
    } else {
        let stuck = indegree
            .into_iter()
            .filter(|(_, d)| *d > 0)
            .map(|(id, _)| id)
            .collect();
        Err(CycleError::Cycle { stuck })
    }
}

/// The repair DAG is not a DAG, or references unknown steps.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CycleError {
    /// A dependency references a step that is not in the plan.
    #[error("dependency references unknown step ({before} -> {after})")]
    UnknownStep {
        /// The `before` endpoint.
        before: RepairStepId,
        /// The `after` endpoint.
        after: RepairStepId,
    },
    /// A dependency cycle.
    #[error("dependency cycle among steps: {stuck:?}")]
    Cycle {
        /// Steps that never became ready.
        stuck: Vec<RepairStepId>,
    },
}

/// Build an undo plan for an accepted repair.
///
/// If later edits invalidated the recorded inverse, undo becomes ANOTHER
/// Basis-checked `RepairPlan` — never an unconditional overwrite of current
/// state with stale bytes (R4 §6, §11.3).
pub fn plan_undo(
    record: &RepairRecord,
    current: &WorkspaceBasis,
) -> Result<RepairPlan, UndoBlocked> {
    // M04 Algorithm D. The recorded inverse restores the preimage bytes, but it
    // is only valid if the world still matches the state the inverse expects to
    // undo (i.e. nothing was written over the accepted repair since). Otherwise
    // undo must become a NEW reviewable plan — never a stale-byte overwrite.
    let inverse = record.inverse.as_ref().ok_or(UndoBlocked::NoInverse)?;

    for step in inverse.steps.values() {
        if let StatePredicate::FileContent { path, hash } = &step.expected_prestate {
            let key = liminal_id::JurisdictionKey::Path(path.clone());
            let found = match current.components.get(&key) {
                Some(liminal_revision::BasisComponent::FileContent { hash: h, .. }) => Some(*h),
                _ => None,
            };
            if found != Some(*hash) {
                let exp8 = &hash.to_hex()[..8];
                let found8 =
                    found.map_or_else(|| "absent".to_owned(), |h| h.to_hex()[..8].to_owned());
                return Err(UndoBlocked::StaleState {
                    detail: format!("{path}: expected {exp8}, found {found8}"),
                });
            }
        }
        // Graph predicates and Any pass here; ILRP re-verifies at apply.
    }

    // All prestates match: the recorded inverse is directly applicable. Reuse
    // its step/plan ids so a double-undo is detected as already-applied
    // (idempotent), but rebase onto the current Basis.
    let mut plan = inverse.clone();
    plan.basis = current.clone();
    Ok(plan)
}

/// Why an undo could not be planned automatically.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UndoBlocked {
    /// No inverse or preimages were recorded.
    #[error("repair recorded no inverse")]
    NoInverse,
    /// Later edits made the inverse's prestates unmatchable; a reviewable
    /// repair is required instead.
    #[error("current state diverged from the inverse's expected prestate: {detail}")]
    StaleState {
        /// What diverged.
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use liminal_id::TransactionId;
    use liminal_revision::BasisPerspective;

    use super::*;

    fn empty_basis() -> WorkspaceBasis {
        WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        }
    }

    fn step(subject_node: NodeId) -> ProposedMutation {
        ProposedMutation {
            id: RepairStepId::new(),
            subject: JurisdictionSubject::Node(subject_node),
            operation: RepairOperation::WriteFile {
                path: PathId("x.md".into()),
                contents: vec![],
            },
            expected_prestate: StatePredicate::Any,
            expected_poststate: StatePredicate::Any,
            idempotency_key: IdempotencyKey::new(),
        }
    }

    fn plan_of(steps: Vec<ProposedMutation>, dependencies: Vec<RepairDependency>) -> RepairPlan {
        RepairPlan {
            id: RepairId::new(),
            basis: empty_basis(),
            steps: steps.into_iter().map(|s| (s.id, s)).collect(),
            dependencies,
            inverse: None,
        }
    }

    #[test]
    fn dependencies_are_respected() {
        // The canonical two-step DAG: ID insertion before reattachment (v4 §7.7).
        let insert = step(NodeId::new());
        let reattach = step(NodeId::new());
        let (a, b) = (insert.id, reattach.id);
        let plan = plan_of(
            vec![reattach, insert],
            vec![RepairDependency {
                before: a,
                after: b,
            }],
        );
        let order = topo_order(&plan).unwrap();
        let pos = |id| order.iter().position(|s| *s == id).unwrap();
        assert!(pos(a) < pos(b), "insert must precede reattach");
    }

    #[test]
    fn cycles_are_rejected() {
        let s1 = step(NodeId::new());
        let s2 = step(NodeId::new());
        let (a, b) = (s1.id, s2.id);
        let plan = plan_of(
            vec![s1, s2],
            vec![
                RepairDependency {
                    before: a,
                    after: b,
                },
                RepairDependency {
                    before: b,
                    after: a,
                },
            ],
        );
        assert!(matches!(topo_order(&plan), Err(CycleError::Cycle { .. })));
    }

    #[test]
    fn unknown_step_references_are_rejected() {
        let s1 = step(NodeId::new());
        let a = s1.id;
        let plan = plan_of(
            vec![s1],
            vec![RepairDependency {
                before: a,
                after: RepairStepId::new(),
            }],
        );
        assert!(matches!(
            topo_order(&plan),
            Err(CycleError::UnknownStep { .. })
        ));
    }

    #[test]
    fn scheduling_is_deterministic() {
        let steps: Vec<_> = (0..6).map(|_| step(NodeId::new())).collect();
        let ids: Vec<_> = steps.iter().map(|s| s.id).collect();
        let deps = vec![
            RepairDependency {
                before: ids[0],
                after: ids[3],
            },
            RepairDependency {
                before: ids[1],
                after: ids[3],
            },
            RepairDependency {
                before: ids[3],
                after: ids[5],
            },
        ];
        let plan = plan_of(steps.clone(), deps.clone());
        let first = topo_order(&plan).unwrap();
        for _ in 0..10 {
            let again = topo_order(&plan).unwrap();
            assert_eq!(again, first, "same plan must always schedule identically");
        }
    }
}
