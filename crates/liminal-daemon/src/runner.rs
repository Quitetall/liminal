//! Scenario runner: `liminald exec` and `liminald recover` semantics (M02
//! Algorithm F).
//!
//! `exec` loads a scenario, sets up files + buffers, runs steps (buffer_edit,
//! save), and exits. `recover` opens the workspace (which runs ILRP recovery)
//! and prints terminal states.

use camino::{Utf8Path, Utf8PathBuf};
use liminal_graph::ns::{ILRP_INTENT, JUR_ALIAS, JUR_DECISION, JUR_PLAN, JUR_REPAIR, SYS_BLOB};
use liminal_graph::{
    GraphStore, IdentityRequirement, Node, NodeFlags, Operation, PayloadRef, Relation,
    RelationFlags, Target,
};
use liminal_id::{
    ClientId, ContentHash, EntityId, IdempotencyKey, IdentityGrade, JurisdictionKey,
    JurisdictionSubject, NodeId, PathId, RelationId, RepairId, RepairStepId, RevisionId,
    TransactionId,
};
use liminal_jurisdiction::{
    Checker, CrashInjector, ExternalExecutor, IlrpDriver, IntentState, InverseRepairPlan,
    ProposedMutation, RepairDecision, RepairOperation, RepairPlan, RepairRecord, SafetyEvidence,
    StatePredicate, blob,
};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};
use liminal_source::merge::{self, MergeOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::scenario::{ScenarioScript, SetupGraph};
use crate::workspace::ToyWorkspace;

/// Ingest setup files: parse paragraphs, create FILE/PARAGRAPH nodes, store
/// aliases (M03 Data schemas).
fn ingest_files(store: &GraphStore, files: &[crate::scenario::SetupFile]) -> anyhow::Result<()> {
    let meta = liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: liminal_id::Timestamp::now(),
        provenance: Some("setup:file".into()),
        inverse: None,
    };

    for file in files {
        // 1. Create FILE node.
        let file_node = NodeId::new();
        {
            let mut txn = store.begin()?;
            txn.apply(Operation::CreateNode {
                node: Node {
                    id: file_node,
                    kind: liminal_graph::kind::FILE,
                    payload: PayloadRef::None,
                    revision: RevisionId(0),
                    flags: NodeFlags::default(),
                },
            })?;
            // Store raw bytes in SYS_BLOB.
            txn.put_aux(
                SYS_BLOB,
                &format!("file/{}", file.path),
                serde_json::Value::String(file.text.clone()),
            )?;
            txn.commit(meta.clone())?;
        }

        // 2. Parse paragraphs and create nodes.
        let blocks = liminal_source::paragraph::parse(&file.text);
        for (i, block) in blocks.iter().enumerate() {
            let flags = if block.id.is_some() {
                NodeFlags::HAS_DURABLE_ID
            } else {
                NodeFlags::default()
            };
            let para_node = NodeId::new();
            let mut txn = store.begin()?;
            txn.apply(Operation::CreateNode {
                node: Node {
                    id: para_node,
                    kind: liminal_graph::kind::PARAGRAPH,
                    payload: PayloadRef::Text(block.text.clone()),
                    revision: RevisionId(0),
                    flags,
                },
            })?;
            // 3. InsertChild.
            txn.apply(Operation::InsertChild {
                parent: file_node,
                child: para_node,
                index: i as u64,
            })?;
            // 4. Store alias if id present.
            if let Some(ref id) = block.id {
                let entity = EntityId::new();
                txn.put_aux(
                    JUR_ALIAS,
                    id,
                    serde_json::json!({
                        "node": para_node.to_string(),
                        "entity": entity.to_string(),
                    }),
                )?;
            }
            txn.commit(meta.clone())?;
        }
    }
    Ok(())
}

/// Ingest setup graph entries: COMMENT nodes + relations (M03 Data schemas).
fn ingest_graph(store: &GraphStore, graph_entries: &[SetupGraph]) -> anyhow::Result<()> {
    let meta = liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: liminal_id::Timestamp::now(),
        provenance: Some("setup:graph".into()),
        inverse: None,
    };

    for entry in graph_entries {
        match entry.kind.as_str() {
            "comment-relation" => {
                let body = entry
                    .extra
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let target_alias = entry
                    .target
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("comment-relation missing target"))?;
                let requires_grade = entry.requires_grade.as_deref().unwrap_or("explicit");

                // Look up the target node from the alias.
                let alias_value = store
                    .get_aux(JUR_ALIAS, target_alias)?
                    .ok_or_else(|| anyhow::anyhow!("alias not found: {target_alias}"))?;
                let node_str = alias_value["node"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("alias missing node: {target_alias}"))?;
                let target_node: NodeId = node_str
                    .parse()
                    .map_err(|e| anyhow::anyhow!("bad node id in alias: {e}"))?;

                let grade: IdentityGrade = requires_grade
                    .parse()
                    .map_err(|_| anyhow::anyhow!("unknown grade: {requires_grade}"))?;

                let comment_node = NodeId::new();
                let mut txn = store.begin()?;
                txn.apply(Operation::CreateNode {
                    node: Node {
                        id: comment_node,
                        kind: liminal_graph::kind::COMMENT,
                        payload: PayloadRef::Text(body.to_owned()),
                        revision: RevisionId(0),
                        flags: NodeFlags::default(),
                    },
                })?;
                txn.apply(Operation::AddRelation {
                    relation: Relation {
                        id: RelationId::new(),
                        source: comment_node,
                        target: Target::Node(target_node),
                        kind: liminal_graph::kind::COMMENT,
                        payload: PayloadRef::None,
                        revision: RevisionId(0),
                        flags: RelationFlags::default(),
                        requires: Some(IdentityRequirement { minimum: grade }),
                    },
                })?;
                txn.commit(meta.clone())?;
            }
            other => {
                anyhow::bail!("unknown setup.graph kind: {other}");
            }
        }
    }
    Ok(())
}

/// Build the save-as-Promotion `RepairPlan` (M04 Algorithm A). One `WriteFile`
/// step targeting the FILE node; prestate = the current durable hash (or
/// `FileAbsent`); poststate = merged-bytes hash; `inverse` restores the OLD
/// bytes. The basis records the base file hash so `safety_check` can recompute
/// disjointness.
fn build_save_plan(
    file_node: NodeId,
    rel: &PathId,
    base_hash: Option<ContentHash>,
    current_hash: Option<ContentHash>,
    old_bytes: Option<&[u8]>,
    merged: &[u8],
) -> RepairPlan {
    let merged_hash = ContentHash::of(merged);
    let expected_prestate = match current_hash {
        Some(hash) => StatePredicate::FileContent {
            path: rel.clone(),
            hash,
        },
        None => StatePredicate::FileAbsent { path: rel.clone() },
    };
    let expected_poststate = StatePredicate::FileContent {
        path: rel.clone(),
        hash: merged_hash,
    };

    // Basis carries two components:
    //  - Path(path) → the CURRENT durable hash (the prestate §1b validates and
    //    the state ILRP lands the write on);
    //  - Object(base_hash) → the buffer's BASE file hash, which
    //    ExternalFileProfile::safety_check reads to recompute disjointness.
    let mut components = BTreeMap::new();
    if let Some(hash) = current_hash {
        components.insert(
            JurisdictionKey::Path(rel.clone()),
            BasisComponent::FileContent {
                path: rel.clone(),
                hash,
            },
        );
    }
    if let Some(hash) = base_hash {
        components.insert(
            JurisdictionKey::Object(hash),
            BasisComponent::ObjectContent { hash },
        );
    }

    let step_id = RepairStepId::new();
    let step = ProposedMutation {
        id: step_id,
        subject: JurisdictionSubject::Node(file_node),
        operation: RepairOperation::WriteFile {
            path: rel.clone(),
            contents: merged.to_vec(),
        },
        expected_prestate: expected_prestate.clone(),
        expected_poststate: expected_poststate.clone(),
        idempotency_key: IdempotencyKey::new(),
    };

    // Inverse: one WriteFile restoring the OLD bytes (prestate = new hash,
    // poststate = old hash). Constructible only when we hold the old bytes.
    let inverse = match (current_hash, old_bytes) {
        (Some(old_hash), Some(old)) => {
            let inv_step_id = RepairStepId::new();
            let inv_step = ProposedMutation {
                id: inv_step_id,
                subject: JurisdictionSubject::Node(file_node),
                operation: RepairOperation::WriteFile {
                    path: rel.clone(),
                    contents: old.to_vec(),
                },
                expected_prestate: expected_poststate,
                expected_poststate: StatePredicate::FileContent {
                    path: rel.clone(),
                    hash: old_hash,
                },
                idempotency_key: IdempotencyKey::new(),
            };
            Some(InverseRepairPlan(Box::new(RepairPlan {
                id: RepairId::new(),
                basis: WorkspaceBasis {
                    transaction: TransactionId::new(),
                    perspective: BasisPerspective::DurableOnly,
                    components: BTreeMap::new(),
                },
                steps: BTreeMap::from([(inv_step_id, inv_step)]),
                dependencies: vec![],
                inverse: None,
            })))
        }
        _ => None,
    };

    RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::ClientScoped {
                client: ClientId::new(),
            },
            components,
        },
        steps: BTreeMap::from([(step_id, step)]),
        dependencies: vec![],
        inverse,
    }
}

/// Detect comment Relations whose target `{#id}` vanished from the file after a
/// foreign edit, and propose the canonical two-step repair DAG for each (M04.3
/// `ingest::foreign_change`). Each DAG is evaluated through the ONE checker;
/// heuristic reattachment is refused for automatic acceptance, so the plan +
/// its NeedsReview decision persist for explicit acceptance (`accept_repair`).
fn plan_foreign_changes(
    profiles: &liminal_jurisdiction::ProfileSet,
    root: &Utf8Path,
    store: &GraphStore,
) -> anyhow::Result<()> {
    let head = store.head()?;

    for relation in store.relations()? {
        if relation.kind != liminal_graph::kind::COMMENT || relation.requires.is_none() {
            continue;
        }
        // The relation targets a paragraph node; find its alias + entity.
        let target_node = relation.target.node();
        let target_str = target_node.to_string();
        let mut alias_entity: Option<(String, EntityId)> = None;
        for (alias, value) in store.scan_aux(JUR_ALIAS)? {
            if value.get("node").and_then(|v| v.as_str()) == Some(target_str.as_str())
                && let Some(entity_str) = value.get("entity").and_then(|v| v.as_str())
                && let Ok(entity) = entity_str.parse::<EntityId>()
            {
                alias_entity = Some((alias, entity));
                break;
            }
        }
        let Some((alias, entity)) = alias_entity else {
            continue;
        };

        // Which file holds this paragraph? Toy: the single ingested file.
        let (rel_path, file_text) = single_file(store, root)?;
        // If the `{#alias}` marker still appears, nothing vanished.
        if file_text.contains(&format!("{{#{alias}}}")) {
            continue;
        }

        // Heuristic candidate: the block whose text matches the original
        // paragraph's payload (structural match). In the toy the target node's
        // payload IS that text.
        let Some(target) = store.node_at(head, target_node)? else {
            continue;
        };
        let candidate_text = match &target.payload {
            PayloadRef::Text(t) => t.clone(),
            _ => continue,
        };
        let blocks = liminal_source::paragraph::parse(&file_text);
        let Some(candidate_block) = blocks.iter().find(|b| b.text == candidate_text) else {
            continue;
        };

        // Build the two-step DAG. The candidate node is the same target node
        // (its identity persists; only the durable marker was stripped). The
        // marker re-inserts at the end of the candidate block.
        let file_node = file_node_for(store, &rel_path)?;
        let prestate_hash = ContentHash::of(file_text.as_bytes());
        let reinserted = {
            let split = usize::try_from(candidate_block.range.end).unwrap_or(file_text.len());
            format!(
                "{} {{#{alias}}}{}",
                &file_text[..split],
                &file_text[split..]
            )
        };
        let poststate_hash = ContentHash::of(reinserted.as_bytes());

        let plan = build_dag_plan(&DagPlanInputs {
            file_node,
            rel_path: rel_path.clone(),
            relation: relation.id,
            candidate: target_node,
            entity,
            file_prestate_hash: prestate_hash,
            file_poststate_hash: poststate_hash,
            at: liminal_source::SourceRange {
                start: candidate_block.range.end,
                end: candidate_block.range.end,
            },
            relation_revision: relation.revision,
        });

        // Persist blobs (base = current file) so any downstream recompute works.
        {
            let mut txn = store.begin()?;
            blob::put(&mut txn, &file_text)?;
            txn.commit(save_meta())?;
        }

        // Evaluate through the ONE checker; persist plan + decision. Heuristic
        // reattachment is refused (Inferred < Explicit) → NeedsReview.
        let checker = Checker { store, profiles };
        let decision = checker
            .evaluate_repair(&plan)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        persist_plan(store, &plan)?;
        persist_decision(store, plan.id, &decision)?;
    }
    Ok(())
}

/// The single ingested file's (path, current on-disk text). Toy-scale: exactly
/// one `file/<path>` blob key exists.
fn single_file(store: &GraphStore, root: &Utf8Path) -> anyhow::Result<(PathId, String)> {
    for (key, _) in store.scan_aux(SYS_BLOB)? {
        if let Some(path) = key.strip_prefix("file/") {
            let text = std::fs::read_to_string(root.join(path))?;
            return Ok((PathId(path.into()), text));
        }
    }
    anyhow::bail!("no ingested file")
}

/// Accept the single pending NeedsReview plan (AM-4.2): record HumanApproval
/// evidence and drive it through the ONE IlrpDriver to Committed. Builds a
/// store-aware `FsExecutor` (with the entity→alias snapshot) so the DAG's
/// `InsertSourceId` step can resolve its marker; honors the ambient crash
/// arming so accept is crash-tested like any other repair.
fn accept_repair(store: &GraphStore, root: &Utf8Path) -> anyhow::Result<()> {
    let executor = crate::executor::FsExecutor::with_store(root.to_owned(), store)?;
    let crash = crate::crash::EnvCrashInjector::from_env();
    let driver = IlrpDriver {
        store,
        executor: &executor,
        crash,
    };
    // Find the single plan whose decision is NeedsReview.
    let decisions = store.scan_aux(JUR_DECISION)?;
    let pending: Vec<RepairId> = decisions
        .into_iter()
        .filter_map(|(key, value)| {
            let id = key.parse::<RepairId>().ok()?;
            match serde_json::from_value::<RepairDecision>(value).ok()? {
                RepairDecision::NeedsReview { .. } => Some(id),
                RepairDecision::AutoApply { .. } => None,
            }
        })
        .collect();
    anyhow::ensure!(
        pending.len() == 1,
        "accept_repair expects exactly one pending plan, found {}",
        pending.len()
    );
    let plan_value = store
        .get_aux(JUR_PLAN, &pending[0].to_string())?
        .ok_or_else(|| anyhow::anyhow!("pending plan not found"))?;
    let plan: RepairPlan = serde_json::from_value(plan_value)?;

    let evidence = SafetyEvidence::HumanApproval {
        actor: liminal_id::ActorId::new(),
        at: liminal_id::Timestamp::now(),
    };
    let id = driver.prepare(plan.clone(), evidence.clone())?;
    driver.run(id)?;
    record_repair(store, &plan, "id-insert-then-reattach", &evidence)?;
    Ok(())
}

/// Inputs to [`build_dag_plan`] — the resolved subjects and state hashes of the
/// canonical foreign-change two-step DAG.
#[derive(Debug, Clone)]
pub struct DagPlanInputs {
    /// The FILE node governing the source-id insertion.
    pub file_node: NodeId,
    /// The file's workspace-relative path.
    pub rel_path: PathId,
    /// The Relation being reattached.
    pub relation: RelationId,
    /// The candidate paragraph node the Relation retargets to.
    pub candidate: NodeId,
    /// The entity whose id the insertion serializes.
    pub entity: EntityId,
    /// The file's current (pre-insertion) content hash.
    pub file_prestate_hash: ContentHash,
    /// The file's hash after the marker is inserted.
    pub file_poststate_hash: ContentHash,
    /// Where the marker is inserted (byte span in prestate bytes).
    pub at: liminal_source::SourceRange,
    /// The Relation's current physical revision.
    pub relation_revision: RevisionId,
}

/// Build the canonical two-step repair DAG (M04 Algorithm A, foreign-change
/// case): `s_insert` inserts the source id into the file (governed by the FILE
/// node's Jurisdiction), then `s_reattach` retargets the Relation to the
/// candidate node (governed by the Relation's Jurisdiction). The dependency
/// edge forces ID insertion strictly before reattachment (R4 §5).
///
/// The step subjects are mutation-local (Law 3F): the file step's subject is
/// the FILE node, the graph step's subject is the Relation — neither
/// commandeers the other.
#[must_use]
pub fn build_dag_plan(i: &DagPlanInputs) -> RepairPlan {
    let s_insert = RepairStepId::new();
    let s_reattach = RepairStepId::new();

    let insert = ProposedMutation {
        id: s_insert,
        subject: JurisdictionSubject::Node(i.file_node),
        operation: RepairOperation::InsertSourceId {
            path: i.rel_path.clone(),
            at: i.at,
            entity: i.entity,
        },
        expected_prestate: StatePredicate::FileContent {
            path: i.rel_path.clone(),
            hash: i.file_prestate_hash,
        },
        expected_poststate: StatePredicate::FileContent {
            path: i.rel_path.clone(),
            hash: i.file_poststate_hash,
        },
        idempotency_key: IdempotencyKey::new(),
    };

    let next_rev = RevisionId(i.relation_revision.0 + 1);
    let reattach = ProposedMutation {
        id: s_reattach,
        subject: JurisdictionSubject::Relation(i.relation),
        operation: RepairOperation::Graph(Operation::RetargetRelation {
            id: i.relation,
            target: Target::Node(i.candidate),
        }),
        expected_prestate: StatePredicate::RelationAt {
            relation: i.relation,
            revision: i.relation_revision,
        },
        expected_poststate: StatePredicate::RelationAt {
            relation: i.relation,
            revision: next_rev,
        },
        idempotency_key: IdempotencyKey::new(),
    };

    RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                JurisdictionKey::Path(i.rel_path.clone()),
                BasisComponent::FileContent {
                    path: i.rel_path.clone(),
                    hash: i.file_prestate_hash,
                },
            )]),
        },
        steps: BTreeMap::from([(s_insert, insert), (s_reattach, reattach)]),
        dependencies: vec![liminal_jurisdiction::RepairDependency {
            before: s_insert,
            after: s_reattach,
        }],
        inverse: None,
    }
}

/// Perform one `save` step (M04 Algorithm C): merge, evaluate, persist plan +
/// decision, and either auto-apply through ILRP or leave a NeedsReview
/// proposal. Save IS Promotion IS a RepairPlan through the ONE interpreter.
fn perform_save<X: ExternalExecutor, C: CrashInjector>(
    driver: &IlrpDriver<'_, X, C>,
    profiles: &liminal_jurisdiction::ProfileSet,
    root: &Utf8Path,
    bases: &BTreeMap<String, String>,
    buffers: &BTreeMap<(String, String), Vec<u8>>,
    client: &str,
    path: &str,
) -> anyhow::Result<RepairId> {
    let store = driver.store;
    let key = (client.to_owned(), path.to_owned());
    let ours = String::from_utf8(
        buffers
            .get(&key)
            .ok_or_else(|| anyhow::anyhow!("buffer not found: {client}/{path}"))?
            .clone(),
    )?;
    let rel = PathId(path.into());
    let abs = root.join(path);

    // Current durable file bytes/hash (foreign edits may have touched it) and
    // the buffer's base text (captured at open, pre-foreign-edit).
    let current = liminal_source::observe(&abs).map_err(|e| anyhow::anyhow!("{e}"))?;
    let current_bytes = if abs.exists() {
        Some(std::fs::read_to_string(&abs)?)
    } else {
        None
    };
    let current_hash = current.as_ref().map(|o| o.hash);
    let base = bases.get(path).cloned();
    let base_hash = base.as_ref().map(|b| ContentHash::of(b.as_bytes()));

    // Persist base + current blobs so the safety predicate can recompute merges.
    {
        let mut txn = store.begin()?;
        if let Some(b) = &base {
            blob::put(&mut txn, b)?;
        }
        if let Some(c) = &current_bytes {
            blob::put(&mut txn, c)?;
        }
        txn.commit(save_meta())?;
    }
    let file_node = file_node_for(store, &rel)?;

    // Three-way merge. Base absent (new file) → the buffer is the merge.
    // Conflict → capture never rejected (Law 3B): persist a draft, don't apply.
    let merged = match (&base, &current_bytes) {
        (Some(b), Some(c)) => match merge::three_way(b, &ours, c) {
            MergeOutcome::Disjoint { merged } | MergeOutcome::UniqueOverlap { merged, .. } => {
                merged
            }
            MergeOutcome::Conflict => {
                let plan = build_save_plan(
                    file_node,
                    &rel,
                    base_hash,
                    current_hash,
                    current_bytes.as_deref().map(str::as_bytes),
                    ours.as_bytes(),
                );
                persist_plan(store, &plan)?;
                return Ok(plan.id);
            }
        },
        _ => ours.clone(),
    };

    let plan = build_save_plan(
        file_node,
        &rel,
        base_hash,
        current_hash,
        current_bytes.as_deref().map(str::as_bytes),
        merged.as_bytes(),
    );
    apply_or_review(driver, profiles, &plan)
}

/// Evaluate a save plan through the checker conjunction, persist the plan +
/// decision, and either auto-apply through the ONE ILRP interpreter (recording
/// the R4 §6 decision log) or leave a NeedsReview proposal (M04 Algorithm C
/// steps 5–7).
fn apply_or_review<X: ExternalExecutor, C: CrashInjector>(
    driver: &IlrpDriver<'_, X, C>,
    profiles: &liminal_jurisdiction::ProfileSet,
    plan: &RepairPlan,
) -> anyhow::Result<RepairId> {
    let store = driver.store;
    let checker = Checker { store, profiles };
    let decision = checker
        .evaluate_repair(plan)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    persist_plan(store, plan)?;
    persist_decision(store, plan.id, &decision)?;

    match decision {
        RepairDecision::AutoApply { evidence } => {
            let id = driver.prepare(plan.clone(), evidence.clone())?;
            driver.run(id)?;
            record_repair(store, plan, "save-promotion", &evidence)?;
            Ok(plan.id)
        }
        // Proposal persisted; nothing applied. M5 adds Overlay + queue item.
        RepairDecision::NeedsReview { .. } => Ok(plan.id),
    }
}

/// The FILE node governing a path (via its SYS_BLOB `file/<path>` ingestion).
fn file_node_for(store: &GraphStore, rel: &PathId) -> anyhow::Result<NodeId> {
    let head = store.head()?;
    for node in store.nodes()? {
        if node.kind == liminal_graph::kind::FILE {
            // In the toy each workspace has one file; return it. (Multi-file
            // dispatch keys on the SYS_BLOB path — M6 territory.)
            let _ = (head, rel);
            return Ok(node.id);
        }
    }
    anyhow::bail!("no FILE node for {rel}")
}

/// Metadata for save-path graph transactions.
fn save_meta() -> liminal_graph::TxnMeta {
    liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: liminal_id::Timestamp::now(),
        provenance: Some("save".into()),
        inverse: None,
    }
}

/// Persist a plan under `JUR_PLAN[repair:<uuid>]`.
fn persist_plan(store: &GraphStore, plan: &RepairPlan) -> anyhow::Result<()> {
    let mut txn = store.begin()?;
    txn.put_aux(JUR_PLAN, &plan.id.to_string(), serde_json::to_value(plan)?)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Persist a decision under `JUR_DECISION[repair:<uuid>]`.
fn persist_decision(
    store: &GraphStore,
    id: RepairId,
    decision: &RepairDecision,
) -> anyhow::Result<()> {
    let mut txn = store.begin()?;
    txn.put_aux(
        JUR_DECISION,
        &id.to_string(),
        serde_json::to_value(decision)?,
    )?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Record an accepted repair under `JUR_REPAIR[repair:<uuid>]` (R4 §6).
fn record_repair(
    store: &GraphStore,
    plan: &RepairPlan,
    selected_rule: &str,
    evidence: &SafetyEvidence,
) -> anyhow::Result<()> {
    let record = RepairRecord {
        repair: plan.id,
        input_basis: plan.basis.clone(),
        selected_rule: selected_rule.to_owned(),
        evidence: evidence.clone(),
        applied_steps: plan.steps.keys().copied().collect(),
        resulting_basis: plan.basis.clone(),
        inverse: plan.inverse.as_ref().map(|i| (*i.0).clone()),
    };
    let mut txn = store.begin()?;
    txn.put_aux(
        JUR_REPAIR,
        &plan.id.to_string(),
        serde_json::to_value(&record)?,
    )?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Run one scripted scenario to completion (or to an armed crash point).
/// Read a required string field from a step's pass-through table.
fn field<'a>(step: &'a crate::scenario::Step, name: &str) -> anyhow::Result<&'a str> {
    step.extra
        .get(name)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} missing {name}", step.kind))
}

/// `buffer_edit`: replace the first `find` with `replace` in a client buffer.
fn apply_buffer_edit(
    step: &crate::scenario::Step,
    buffers: &mut BTreeMap<(String, String), Vec<u8>>,
) -> anyhow::Result<()> {
    let (client, path) = (field(step, "client")?, field(step, "path")?);
    let (find, replace) = (field(step, "find")?, field(step, "replace")?);
    let buf = buffers
        .get_mut(&(client.to_owned(), path.to_owned()))
        .ok_or_else(|| anyhow::anyhow!("buffer not found: {client}/{path}"))?;
    let text = String::from_utf8_lossy(buf).to_string();
    let pos = text
        .find(find)
        .ok_or_else(|| anyhow::anyhow!("find text not found in buffer: {find:?}"))?;
    *buf = format!("{}{}{}", &text[..pos], replace, &text[pos + find.len()..]).into_bytes();
    Ok(())
}

/// `foreign_edit`: a foreign tool rewrites the durable file directly (v4 §8.5).
fn apply_foreign_edit(step: &crate::scenario::Step, root: &Utf8Path) -> anyhow::Result<()> {
    let (path, find, replace) = (
        field(step, "path")?,
        field(step, "find")?,
        field(step, "replace")?,
    );
    let abs = root.join(path);
    let text = std::fs::read_to_string(&abs)?;
    let pos = text
        .find(find)
        .ok_or_else(|| anyhow::anyhow!("foreign_edit find not found: {find:?}"))?;
    let new_text = format!("{}{}{}", &text[..pos], replace, &text[pos + find.len()..]);
    std::fs::write(&abs, new_text.as_bytes())?;
    Ok(())
}

/// Run one scripted scenario to completion (or to an armed crash point).
pub fn exec<X: ExternalExecutor, C: CrashInjector>(
    root: &Utf8Path,
    scenario: &ScenarioScript,
    executor: X,
    crash: C,
) -> anyhow::Result<()> {
    // Write setup files to disk.
    for file in &scenario.setup.files {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, file.text.as_bytes())?;
    }

    let mut buffers: BTreeMap<(String, String), Vec<u8>> = BTreeMap::new();
    for buf in &scenario.setup.buffers {
        let path = root.join(&buf.path);
        buffers.insert(
            (buf.client.clone(), buf.path.clone()),
            std::fs::read(&path)?,
        );
    }

    // Base text per path, captured BEFORE any foreign edit (the buffer's base).
    let mut bases: BTreeMap<String, String> = BTreeMap::new();
    for file in &scenario.setup.files {
        bases.insert(file.path.clone(), file.text.clone());
    }

    let ws = ToyWorkspace::open(root)?;
    let profiles = liminal_jurisdiction::ProfileSet::phase_minus_1();

    // Ingest files and graph entries (M03).
    ingest_files(ws.store(), &scenario.setup.files)?;
    ingest_graph(ws.store(), &scenario.setup.graph)?;

    let store = ws.store();
    let driver = IlrpDriver {
        store,
        executor: &executor,
        crash,
    };

    for step in &scenario.steps {
        match step.kind.as_str() {
            "buffer_edit" => apply_buffer_edit(step, &mut buffers)?,
            "foreign_edit" => {
                apply_foreign_edit(step, root)?;
                // Reconcile immediately: any comment Relation whose target's
                // `{#id}` vanished gets a two-step repair DAG proposed (M04.3
                // ingest::foreign_change). Refused for AUTOMATIC acceptance
                // (heuristic reattachment), so it persists as NeedsReview.
                plan_foreign_changes(&profiles, root, store)?;
            }
            "save" => {
                let client = field(step, "client")?;
                let path = field(step, "path")?;
                perform_save(&driver, &profiles, root, &bases, &buffers, client, path)?;
            }
            "accept_repair" => accept_repair(store, root)?,
            other => anyhow::bail!("step kind {other:?} not implemented until M3/M4"),
        }
    }

    if let Some(ref expected) = scenario.expect.terminal {
        let normalize = |s: &str| s.to_lowercase().replace('-', "");
        let expected_norm = normalize(expected);
        let intents = store.scan_aux(ILRP_INTENT)?;
        if intents.is_empty() {
            // No ILRP intent was prepared — the save landed as a NeedsReview
            // proposal (JUR_DECISION only). The terminal state is NeedsReview.
            let decisions = store.scan_aux(JUR_DECISION)?;
            anyhow::ensure!(
                !decisions.is_empty(),
                "expected terminal {expected:?} but no intent or decision was produced"
            );
            for (_, value) in &decisions {
                let decision: RepairDecision = serde_json::from_value(value.clone())?;
                let actual = match decision {
                    RepairDecision::AutoApply { .. } => "committed",
                    RepairDecision::NeedsReview { .. } => "needsreview",
                };
                anyhow::ensure!(
                    expected_norm == actual,
                    "expected terminal {expected:?}, decision was {actual}"
                );
            }
        } else {
            for (_, value) in &intents {
                let intent: liminal_jurisdiction::RepairIntent =
                    serde_json::from_value(value.clone())?;
                let actual_norm = normalize(&format!("{:?}", intent.state));
                anyhow::ensure!(
                    expected_norm == actual_norm,
                    "expected terminal state {expected:?}, got {:?}",
                    intent.state
                );
            }
        }
    }
    Ok(())
}

/// Recovery outcome: terminal states and world digest.
#[derive(Debug, Serialize, Deserialize)]
pub struct RecoverOutcome {
    /// Terminal states per intent.
    pub terminals: Vec<IntentState>,
    /// World digest for idempotence assertion.
    pub world_digest: String,
}

/// Open the workspace (which runs ILRP recovery), then collect all intent
/// states and compute the world digest.
pub fn recover(root: &Utf8Path) -> anyhow::Result<RecoverOutcome> {
    let ws = ToyWorkspace::open(root)?;
    let store = ws.store();

    // Collect ALL intent states (not just nonterminal — a crash after finalize
    // still reports committed).
    let intents = store.scan_aux(ILRP_INTENT)?;
    let mut terminals = Vec::new();
    for (_, value) in &intents {
        let intent: liminal_jurisdiction::RepairIntent = serde_json::from_value(value.clone())?;
        terminals.push(intent.state);
    }

    // Verify all terminal.
    for state in &terminals {
        if !state.is_terminal() {
            anyhow::bail!("recovery bug: nonterminal state after recovery: {state:?}");
        }
    }

    let world_digest =
        crate::digest::world_digest(root, store).map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(RecoverOutcome {
        terminals,
        world_digest,
    })
}

/// The normalized world digest of a workspace (M04 Algorithm F) — ids replaced
/// by first-appearance ordinals, timestamps dropped. Two workspaces reached by
/// the same interpreter path over the same inputs hash equally.
pub fn normalized_digest(root: &Utf8Path) -> anyhow::Result<String> {
    let ws = ToyWorkspace::open(root)?;
    crate::digest::normalized_world_digest(root, ws.store()).map_err(|e| anyhow::anyhow!("{e}"))
}

/// D04.4 self-check for the gate test: a plan whose file/external step depends
/// on a Graph step is unexecutable under ILRP and refused at `prepare`. Returns
/// true iff `IlrpDriver::prepare` rejects such a plan (reattach-before-insert).
#[must_use]
pub fn graph_before_file_is_refused() -> bool {
    // Hand-build an inverted DAG: a Graph step BEFORE a file step.
    let graph_step = RepairStepId::new();
    let file_step = RepairStepId::new();
    let file_node = NodeId::new();
    let relation = RelationId::new();
    let rel = PathId("notes.md".into());

    let g = ProposedMutation {
        id: graph_step,
        subject: JurisdictionSubject::Relation(relation),
        operation: RepairOperation::Graph(Operation::RetargetRelation {
            id: relation,
            target: Target::Node(file_node),
        }),
        expected_prestate: StatePredicate::Any,
        expected_poststate: StatePredicate::Any,
        idempotency_key: IdempotencyKey::new(),
    };
    let f = ProposedMutation {
        id: file_step,
        subject: JurisdictionSubject::Node(file_node),
        operation: RepairOperation::WriteFile {
            path: rel.clone(),
            contents: b"x".to_vec(),
        },
        expected_prestate: StatePredicate::Any,
        expected_poststate: StatePredicate::Any,
        idempotency_key: IdempotencyKey::new(),
    };
    let plan = RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps: BTreeMap::from([(graph_step, g), (file_step, f)]),
        // file step depends on the graph step → unexecutable (D04.4).
        dependencies: vec![liminal_jurisdiction::RepairDependency {
            before: graph_step,
            after: file_step,
        }],
        inverse: None,
    };

    // A throwaway in-memory store to attempt prepare against.
    let Ok(dir) = tempdir_for("d044") else {
        return false;
    };
    let Ok(store) = GraphStore::open(&dir) else {
        return false;
    };
    let executor = crate::executor::FsExecutor::new(dir.clone());
    let driver = IlrpDriver {
        store: &store,
        executor: &executor,
        crash: liminal_jurisdiction::NoCrash,
    };
    matches!(
        driver.prepare(
            plan,
            SafetyEvidence::StructurallyDisjoint {
                description: "d044 check".into(),
            }
        ),
        Err(liminal_jurisdiction::IlrpError::Executor(_))
    )
}

/// A fresh scratch store dir under the system temp.
fn tempdir_for(label: &str) -> anyhow::Result<Utf8PathBuf> {
    let dir = Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
        "liminal-d044-{label}-{}-{:x}",
        std::process::id(),
        liminal_id::Timestamp::now().0
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Build the `lim repairs` report lines (M04 Algorithm E), sorted by repair id
/// (UUIDv7 ⇒ chronological). Zero records + zero nonterminal intents → empty.
///
/// Lines:
///   `repair:<uuid>  <rule>  <evidence-kind>  <n> steps  undo available|undone|no inverse`
///   `repair:<uuid>  needs review: <first reason>`      (decision without a record)
///   `repair:<uuid>  interrupted (<state>) — run recovery`   (nonterminal intent)
pub fn repairs_lines(root: &Utf8Path) -> anyhow::Result<Vec<String>> {
    let ws = ToyWorkspace::open(root)?;
    let store = ws.store();

    // Accepted records + the set of repairs undone by a later `undo:<id>` record.
    let records = ws.repairs().map_err(|e| anyhow::anyhow!("{e}"))?;
    let undone: std::collections::BTreeSet<RepairId> = records
        .iter()
        .filter_map(|r| r.selected_rule.strip_prefix("undo:").map(ToOwned::to_owned))
        .filter_map(|s| s.parse::<RepairId>().ok())
        .collect();

    // Map id → line. BTreeMap keeps them sorted by id.
    let mut lines: BTreeMap<RepairId, String> = BTreeMap::new();

    for r in &records {
        let evidence_kind = match &r.evidence {
            SafetyEvidence::StructurallyDisjoint { .. } => "structural-disjointness",
            SafetyEvidence::DomainValidator { .. } => "domain-validator",
            SafetyEvidence::HumanApproval { .. } => "human-approval",
        };
        let undo_state = if undone.contains(&r.repair) {
            "undone"
        } else if r.inverse.is_some() {
            "undo available"
        } else {
            "no inverse"
        };
        lines.insert(
            r.repair,
            format!(
                "{}  {}  {evidence_kind}  {} steps  {undo_state}",
                r.repair,
                r.selected_rule,
                r.applied_steps.len()
            ),
        );
    }

    // NeedsReview decisions without an accepted record.
    let record_ids: std::collections::BTreeSet<RepairId> =
        records.iter().map(|r| r.repair).collect();
    for (key, value) in store.scan_aux(JUR_DECISION)? {
        let Ok(id) = key.parse::<RepairId>() else {
            continue;
        };
        if record_ids.contains(&id) {
            continue;
        }
        if let Ok(RepairDecision::NeedsReview { reasons }) = serde_json::from_value(value) {
            let first = reasons.first().map_or("", |r| r.0.as_str());
            lines.insert(id, format!("{id}  needs review: {first}"));
        }
    }

    // Nonterminal ILRP intents.
    for (key, value) in store.scan_aux(ILRP_INTENT)? {
        let Ok(id) = key.parse::<RepairId>() else {
            continue;
        };
        if let Ok(intent) = serde_json::from_value::<liminal_jurisdiction::RepairIntent>(value)
            && !intent.state.is_terminal()
        {
            let state = format!("{:?}", intent.state).to_lowercase();
            lines.insert(id, format!("{id}  interrupted ({state}) — run recovery"));
        }
    }

    Ok(lines.into_values().collect())
}

/// The result of a `lim repair undo` (M04 Algorithm D CLI flow).
#[derive(Debug)]
pub enum UndoOutcome {
    /// The recorded inverse applied cleanly; a new undo record was written.
    Undone {
        /// The undo repair id.
        repair: RepairId,
    },
    /// Later edits diverged from the inverse's prestate; a reviewable proposal
    /// was queued instead of a stale-byte overwrite.
    QueuedForReview {
        /// The queued proposal's repair id.
        repair: RepairId,
        /// What diverged.
        detail: String,
    },
    /// The repair recorded no inverse.
    NoInverse,
}

/// Undo an accepted repair by id (M04 Algorithm D). Applies the recorded
/// inverse when the world still matches it; otherwise queues a reviewable
/// proposal (never a stale-byte overwrite, R4 §6/§11.3).
pub fn undo(root: &Utf8Path, repair_id: &str) -> anyhow::Result<UndoOutcome> {
    let target: RepairId = repair_id
        .parse()
        .map_err(|e| anyhow::anyhow!("bad repair id: {e}"))?;

    let ws = ToyWorkspace::open(root)?;
    let store = ws.store();

    // Load the accepted record.
    let record = store
        .get_aux(JUR_REPAIR, &target.to_string())?
        .ok_or_else(|| anyhow::anyhow!("unknown repair: {repair_id}"))?;
    let record: RepairRecord = serde_json::from_value(record)?;

    // Build the current Basis: the file's current durable hash per path in the
    // inverse's file steps.
    let current = current_basis_for(store, root, &record)?;

    let executor = crate::executor::FsExecutor::new(root.to_owned());
    let driver = IlrpDriver {
        store,
        executor: &executor,
        crash: liminal_jurisdiction::NoCrash,
    };

    match liminal_jurisdiction::plan_undo(&record, &current) {
        Ok(plan) => {
            let evidence = SafetyEvidence::StructurallyDisjoint {
                description: format!("undo of {target}: restores recorded preimages"),
            };
            let id = plan.id;
            let d = driver.prepare(plan.clone(), evidence.clone())?;
            driver.run(d)?;
            record_repair(store, &plan, &format!("undo:{target}"), &evidence)?;
            Ok(UndoOutcome::Undone { repair: id })
        }
        Err(liminal_jurisdiction::UndoBlocked::NoInverse) => Ok(UndoOutcome::NoInverse),
        Err(liminal_jurisdiction::UndoBlocked::StaleState { detail }) => {
            // Build a reviewable proposal: preimage WriteFile steps whose
            // prestate is the CURRENTLY observed hash (acknowledging it would
            // overwrite intervening edits — hence review).
            let proposal = build_review_undo(&record, root)?;
            persist_plan(store, &proposal)?;
            persist_decision(
                store,
                proposal.id,
                &RepairDecision::NeedsReview {
                    reasons: vec![liminal_jurisdiction::ReviewReason(detail.clone())],
                },
            )?;
            Ok(UndoOutcome::QueuedForReview {
                repair: proposal.id,
                detail,
            })
        }
    }
}

/// The current Basis over the paths a record's inverse touches (their current
/// durable hashes).
fn current_basis_for(
    store: &GraphStore,
    root: &Utf8Path,
    record: &RepairRecord,
) -> anyhow::Result<WorkspaceBasis> {
    let mut components = BTreeMap::new();
    if let Some(inv) = &record.inverse {
        for step in inv.steps.values() {
            if let RepairOperation::WriteFile { path, .. } = &step.operation {
                let abs = root.join(&path.0);
                if let Some(obs) =
                    liminal_source::observe(&abs).map_err(|e| anyhow::anyhow!("{e}"))?
                {
                    components.insert(
                        JurisdictionKey::Path(path.clone()),
                        BasisComponent::FileContent {
                            path: path.clone(),
                            hash: obs.hash,
                        },
                    );
                }
            }
        }
    }
    let _ = store;
    Ok(WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components,
    })
}

/// A reviewable undo proposal when the direct inverse is stale: preimage
/// WriteFile steps whose prestate is the CURRENTLY observed durable hash.
fn build_review_undo(record: &RepairRecord, root: &Utf8Path) -> anyhow::Result<RepairPlan> {
    let inv = record
        .inverse
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no inverse to review"))?;
    let mut steps = BTreeMap::new();
    for step in inv.steps.values() {
        if let RepairOperation::WriteFile { path, contents } = &step.operation {
            let abs = root.join(&path.0);
            let current_hash = liminal_source::observe(&abs)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .map(|o| o.hash);
            let prestate = match current_hash {
                Some(hash) => StatePredicate::FileContent {
                    path: path.clone(),
                    hash,
                },
                None => StatePredicate::FileAbsent { path: path.clone() },
            };
            let sid = RepairStepId::new();
            steps.insert(
                sid,
                ProposedMutation {
                    id: sid,
                    subject: step.subject,
                    operation: RepairOperation::WriteFile {
                        path: path.clone(),
                        contents: contents.clone(),
                    },
                    expected_prestate: prestate,
                    expected_poststate: StatePredicate::FileContent {
                        path: path.clone(),
                        hash: ContentHash::of(contents),
                    },
                    idempotency_key: IdempotencyKey::new(),
                },
            );
        }
    }
    Ok(RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps,
        dependencies: vec![],
        inverse: None,
    })
}
