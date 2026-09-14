//! Scenario runner: `liminald exec` and `liminald recover` semantics (M02
//! Algorithm F).
//!
//! `exec` loads a scenario, sets up files + buffers, runs steps (buffer_edit,
//! save), and exits. `recover` opens the workspace (which runs ILRP recovery)
//! and prints terminal states.

use camino::{Utf8Path, Utf8PathBuf};
use liminal_graph::ns::{
    ILRP_INTENT, JUR_ALIAS, JUR_DECISION, JUR_OVERLAY, JUR_OVERLAY_LOG, JUR_PLAN, JUR_REPAIR,
    SYS_BLOB, SYS_CLOCK, SYS_UNAVAILABLE,
};
use liminal_graph::{
    GraphStore, IdentityRequirement, Node, NodeFlags, Operation, PayloadRef, Relation,
    RelationFlags, Target,
};
use liminal_id::{
    ClientId, ContentHash, EntityId, IdempotencyKey, IdentityGrade, JurisdictionKey,
    JurisdictionSubject, NodeId, OverlayId, PathId, ReconciliationItemId, RelationId, RepairId,
    RepairStepId, RevisionId, SourceId, Timestamp, TransactionId,
};
use liminal_jurisdiction::{
    Checker, CrashInjector, ExternalExecutor, IlrpDriver, IntentState, InverseRepairPlan, Overlay,
    OverlayState, ProfileSet, ProposedMutation, ReconciliationItem, ReconciliationQueue,
    ReconciliationStatus, RepairDecision, RepairOperation, RepairPlan, RepairRecord,
    SafetyEvidence, StatePredicate, blob,
};
use liminal_query::Query;
use liminal_resolver::ReplayableResolver;
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};
use liminal_source::merge::{self, MergeOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::reactor::{Observation, ScriptedStockResolver, parse_utc_timestamp};
use crate::scenario::{ScenarioScript, SetupGraph};
use crate::workspace::ToyWorkspace;

/// Ingest setup files: parse paragraphs, create FILE/PARAGRAPH nodes, store
/// aliases (M03 Data schemas).
pub(crate) fn ingest_files(
    store: &GraphStore,
    files: &[crate::scenario::SetupFile],
) -> anyhow::Result<()> {
    let meta = liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: Timestamp::now(),
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
            // 4. Store alias if id present. D03.3: the first occurrence wins;
            // later duplicates get NO alias — the checker surfaces the
            // orphaned durable-id marker as JUR042.
            if let Some(ref id) = block.id
                && store.get_aux(JUR_ALIAS, id)?.is_none()
            {
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
pub(crate) fn ingest_graph(store: &GraphStore, graph_entries: &[SetupGraph]) -> anyhow::Result<()> {
    let meta = liminal_graph::TxnMeta {
        actor: None,
        origin: liminal_graph::Origin::Human,
        at: Timestamp::now(),
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
    profiles: &ProfileSet,
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

        // Creation site (3), M05.md Data schemas: a NeedsReview DAG proposal
        // writes TWO overlays (source-id insertion + relation endpoint) plus
        // ONE coalesced `identity:` item (D05.5) — the plan stays actionable
        // via `accept_repair` until explicitly accepted or discarded (D05.6).
        if matches!(decision, RepairDecision::NeedsReview { .. }) {
            propose_dag_review(store, profiles, &plan, &rel_path, &alias)?;
        }
    }
    Ok(())
}

/// Build the `RepairProposed` Overlay for one plan step, dated `now` (creation
/// sites 2/3, M05.md Data schemas): its lifecycle comes from the step
/// subject's OWN governing profile (Law 3F — no cross-subject borrowing).
fn review_overlay(
    profiles: &ProfileSet,
    store: &GraphStore,
    subject: JurisdictionSubject,
    base: WorkspaceBasis,
    operation: RepairOperation,
    plan_id: RepairId,
    now: Timestamp,
) -> anyhow::Result<Overlay> {
    let profile = profiles
        .for_subject(subject, store)
        .ok_or_else(|| anyhow::anyhow!("no profile governs {subject}"))?;
    let lifecycle = profile.contract_for(subject, store).lifecycle;
    Ok(Overlay {
        id: OverlayId::new(),
        subject,
        base,
        operation,
        created_at: now,
        last_activity: now,
        profile: profile.id(),
        lifecycle,
        state: OverlayState::RepairProposed(plan_id),
    })
}

/// Creation site (2), M04 Algorithm C step 7 / D05.5: a single-step save plan
/// that came back NeedsReview writes ONE Overlay plus ONE coalesced
/// `<category>:path:<p>` item, in one transaction.
fn propose_review(
    store: &GraphStore,
    profiles: &ProfileSet,
    plan: &RepairPlan,
) -> anyhow::Result<()> {
    let step = plan
        .steps
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("NeedsReview plan has no steps"))?;
    let path = overlay_operation_path(&step.operation)
        .ok_or_else(|| anyhow::anyhow!("NeedsReview save plan must have a file step"))?;
    let now = now_with_offset(store)?;
    let overlay = review_overlay(
        profiles,
        store,
        step.subject,
        plan.basis.clone(),
        step.operation.clone(),
        plan.id,
        now,
    )?;
    let item = ReconciliationItem {
        id: ReconciliationItemId::new(),
        root_cause: format!("conflict:path:{path}"),
        subjects: vec![step.subject],
        overlays: vec![overlay.id],
        repairs: vec![plan.id],
        created_at: now,
        status: ReconciliationStatus::Pending,
    };
    let mut txn = store.begin()?;
    txn.put_aux(
        JUR_OVERLAY,
        &overlay.id.to_string(),
        serde_json::to_value(&overlay)?,
    )?;
    ReconciliationQueue { store }.upsert_coalesced(&mut txn, item)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Creation site (3), M05.md Data schemas: the two-step foreign-change DAG
/// (source-id insertion + Relation reattachment) writes TWO overlays plus ONE
/// coalesced `identity:path:<p>#<alias>` item (D05.5), in one transaction.
fn propose_dag_review(
    store: &GraphStore,
    profiles: &ProfileSet,
    plan: &RepairPlan,
    rel_path: &PathId,
    alias: &str,
) -> anyhow::Result<()> {
    let now = now_with_offset(store)?;
    let mut overlays = Vec::with_capacity(plan.steps.len());
    for step in plan.steps.values() {
        overlays.push(review_overlay(
            profiles,
            store,
            step.subject,
            plan.basis.clone(),
            step.operation.clone(),
            plan.id,
            now,
        )?);
    }
    let item = ReconciliationItem {
        id: ReconciliationItemId::new(),
        root_cause: format!("identity:path:{rel_path}#{alias}"),
        subjects: overlays.iter().map(|o| o.subject).collect(),
        overlays: overlays.iter().map(|o| o.id).collect(),
        repairs: vec![plan.id],
        created_at: now,
        status: ReconciliationStatus::Pending,
    };
    let mut txn = store.begin()?;
    for overlay in &overlays {
        txn.put_aux(
            JUR_OVERLAY,
            &overlay.id.to_string(),
            serde_json::to_value(overlay)?,
        )?;
    }
    ReconciliationQueue { store }.upsert_coalesced(&mut txn, item)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Retire every Overlay proposed for an accepted plan (D05.6: accepted repair
/// is a departure path — the Finalize txn deletes the overlay key, history
/// stays in the log) and resolve any reconciliation item that covered it.
fn retire_proposed_overlays(store: &GraphStore, plan_id: RepairId) -> anyhow::Result<()> {
    let mut txn = store.begin()?;
    let mut any = false;
    for (key, value) in store.scan_aux(JUR_OVERLAY)? {
        let overlay: Overlay = serde_json::from_value(value)?;
        if overlay.state == OverlayState::RepairProposed(plan_id) {
            txn.delete_aux(JUR_OVERLAY, &key)?;
            resolve_covering_item(store, &mut txn, overlay.id)?;
            any = true;
        }
    }
    if any {
        txn.commit(save_meta())?;
    }
    Ok(())
}

/// Mark every `Pending`/`Blocked` reconciliation item that lists `overlay` as
/// `Resolved` (an accepted repair or holder-return resolves its debt).
fn resolve_covering_item(
    store: &GraphStore,
    txn: &mut liminal_graph::GraphTxn<'_>,
    overlay: OverlayId,
) -> anyhow::Result<()> {
    let queue = ReconciliationQueue { store };
    for mut item in queue.items()? {
        if item.overlays.contains(&overlay) && item.status != ReconciliationStatus::Resolved {
            item.status = ReconciliationStatus::Resolved;
            txn.put_aux(
                liminal_jurisdiction::reconcile::RECONCILE_NS,
                &item.id.to_string(),
                serde_json::to_value(&item)?,
            )?;
        }
    }
    Ok(())
}

/// The workspace-relative path a repair operation targets, when it is a
/// file-holder step (`WriteFile` / `InsertSourceId`); `None` for Graph steps.
pub fn overlay_operation_path(op: &RepairOperation) -> Option<PathId> {
    match op {
        RepairOperation::WriteFile { path, .. } | RepairOperation::InsertSourceId { path, .. } => {
            Some(path.clone())
        }
        RepairOperation::Graph(_) | RepairOperation::External { .. } => None,
    }
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
fn accept_repair(
    store: &GraphStore,
    root: &Utf8Path,
    actor: liminal_id::ActorId,
) -> anyhow::Result<()> {
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
        actor,
        at: Timestamp::now(),
    };
    let id = driver.prepare(plan.clone(), evidence.clone())?;
    driver.run(id)?;
    // DG-8.3: `InsertSourceId` mutates the file on disk without carrying the
    // landed bytes in-plan, so the save-time mirror (`refresh_file_blobs`)
    // never sees it — re-read the landed files here or the `file/<path>` blob
    // is a stale pre-acceptance echo.
    refresh_inserted_file_blobs(store, root, &plan)?;
    record_repair(store, &plan, "id-insert-then-reattach", &evidence)?;
    // D05.6: acceptance is a departure path — retire the proposal's overlays.
    retire_proposed_overlays(store, plan.id)?;
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
    profiles: &ProfileSet,
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

    // D05.2: the write route may be unavailable even though the file is
    // still readable (read-only remount). Capture never blocks (Law 3B) —
    // persist the plan and coalesce a durable Overlay instead of driving
    // ILRP; no decision, no reconciliation item.
    if !holder_available(store, path)? {
        return offline_save(store, profiles, &plan, file_node, merged.as_bytes());
    }

    apply_or_review(driver, profiles, &plan)
}

/// Whether `path`'s Holder currently accepts writes (D05.1): true iff
/// `SYS_UNAVAILABLE[path]` is absent.
pub fn holder_available(store: &GraphStore, path: &str) -> anyhow::Result<bool> {
    Ok(store.get_aux(SYS_UNAVAILABLE, path)?.is_none())
}

/// Offline save (D05.2/D05.3): one txn writes the plan (`JUR_PLAN`), the
/// draft blob (`SYS_BLOB[ours]`), and a coalesced `Active` Overlay. A second
/// offline save to the same subject updates the EXISTING Active overlay in
/// place (same id/created_at, new operation/last_activity) and appends the
/// prior operation to `JUR_OVERLAY_LOG` before overwriting it (D05.3: coalesce
/// without erasing history).
fn offline_save(
    store: &GraphStore,
    profiles: &ProfileSet,
    plan: &RepairPlan,
    file_node: NodeId,
    ours: &[u8],
) -> anyhow::Result<RepairId> {
    let subject = JurisdictionSubject::Node(file_node);
    let operation = plan
        .steps
        .values()
        .next()
        .map(|s| s.operation.clone())
        .ok_or_else(|| anyhow::anyhow!("offline save plan has no steps"))?;
    let now = now_with_offset(store)?;
    let profile = profiles
        .for_subject(subject, store)
        .ok_or_else(|| anyhow::anyhow!("no profile governs {subject}"))?;
    let lifecycle = profile.contract_for(subject, store).lifecycle;

    let mut txn = store.begin()?;
    txn.put_aux(JUR_PLAN, &plan.id.to_string(), serde_json::to_value(plan)?)?;
    blob::put(&mut txn, &String::from_utf8_lossy(ours))?;

    let overlay = match active_overlay_for(store, subject)? {
        Some(mut existing) => {
            let seq = next_overlay_log_seq(store, existing.id)?;
            txn.put_aux(
                JUR_OVERLAY_LOG,
                &format!("{}/{seq:04}", existing.id),
                serde_json::json!({"at": existing.last_activity, "operation": existing.operation}),
            )?;
            existing.operation = operation;
            existing.last_activity = now;
            existing
        }
        None => Overlay {
            id: OverlayId::new(),
            subject,
            base: plan.basis.clone(),
            operation,
            created_at: now,
            last_activity: now,
            profile: profile.id(),
            lifecycle,
            state: OverlayState::Active,
        },
    };
    txn.put_aux(
        JUR_OVERLAY,
        &overlay.id.to_string(),
        serde_json::to_value(&overlay)?,
    )?;
    txn.commit(save_meta())?;
    Ok(plan.id)
}

/// The current `Active` overlay for `subject`, if one exists (coalescing
/// lookup for D05.3).
fn active_overlay_for(
    store: &GraphStore,
    subject: JurisdictionSubject,
) -> anyhow::Result<Option<Overlay>> {
    for (_key, value) in store.scan_aux(JUR_OVERLAY)? {
        let overlay: Overlay = serde_json::from_value(value)?;
        if overlay.subject == subject && overlay.state == OverlayState::Active {
            return Ok(Some(overlay));
        }
    }
    Ok(None)
}

/// The next 4-digit zero-padded `JUR_OVERLAY_LOG` sequence number for
/// `overlay` (D05.3: `overlay:<uuid>/0001`, incrementing).
fn next_overlay_log_seq(store: &GraphStore, overlay: OverlayId) -> anyhow::Result<u32> {
    let prefix = format!("{overlay}/");
    let count = store
        .scan_aux(JUR_OVERLAY_LOG)?
        .into_iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .count();
    Ok(u32::try_from(count + 1).unwrap_or(u32::MAX))
}

/// Evaluate a save plan through the checker conjunction, persist the plan +
/// decision, and either auto-apply through the ONE ILRP interpreter (recording
/// the R4 §6 decision log) or leave a NeedsReview proposal (M04 Algorithm C
/// steps 5–7).
fn apply_or_review<X: ExternalExecutor, C: CrashInjector>(
    driver: &IlrpDriver<'_, X, C>,
    profiles: &ProfileSet,
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
            refresh_file_blobs(store, plan)?;
            record_repair(store, plan, "save-promotion", &evidence)?;
            Ok(plan.id)
        }
        RepairDecision::NeedsReview { .. } => {
            // Creation site (2), M04 Algorithm C step 7 (D05.2/D05.5).
            propose_review(store, profiles, plan)?;
            Ok(plan.id)
        }
    }
}

/// M08.3: standardize `file/<path>` in `SYS_BLOB` the same way
/// `persist_buffer_blob` standardizes `buf/<client>/<buffer>/<generation>` —
/// refresh it at the moment content actually becomes durable. Buffer blobs
/// refresh on every edit (D06.4); this is the save-time counterpart: once a
/// `save` step's `WriteFile` steps land on disk via ILRP, mirror the same
/// bytes into `SYS_BLOB["file/<path>"]` so a fresh process's file-blob scan
/// (`seed_durable_inputs`, `file_paths`) is never reading a stale ingest-time
/// echo. A no-op when `plan` carries no `WriteFile` step (e.g. the
/// id-then-reattach DAG, which is graph + `InsertSourceId` only).
fn refresh_file_blobs(store: &GraphStore, plan: &RepairPlan) -> anyhow::Result<()> {
    let writes: Vec<(&PathId, &[u8])> = plan
        .steps
        .values()
        .filter_map(|step| match &step.operation {
            RepairOperation::WriteFile { path, contents } => Some((path, contents.as_slice())),
            _ => None,
        })
        .collect();
    if writes.is_empty() {
        return Ok(());
    }
    let mut txn = store.begin()?;
    for (path, contents) in writes {
        txn.put_aux(
            SYS_BLOB,
            &format!("file/{path}"),
            serde_json::Value::String(String::from_utf8_lossy(contents).into_owned()),
        )?;
    }
    txn.commit(save_meta())?;
    Ok(())
}

/// The accepted-repair counterpart of [`refresh_file_blobs`] (DG-8.3):
/// `InsertSourceId` steps land bytes the plan does not carry, so the mirror
/// must read the landed file back from disk AFTER the driver commits. A no-op
/// for plans with no `InsertSourceId` step.
fn refresh_inserted_file_blobs(
    store: &GraphStore,
    root: &Utf8Path,
    plan: &RepairPlan,
) -> anyhow::Result<()> {
    let paths: std::collections::BTreeSet<&PathId> = plan
        .steps
        .values()
        .filter_map(|step| match &step.operation {
            RepairOperation::InsertSourceId { path, .. } => Some(path),
            _ => None,
        })
        .collect();
    if paths.is_empty() {
        return Ok(());
    }
    let mut txn = store.begin()?;
    for path in paths {
        let bytes = std::fs::read(root.join(&path.0))?;
        txn.put_aux(
            SYS_BLOB,
            &format!("file/{path}"),
            serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned()),
        )?;
    }
    txn.commit(save_meta())?;
    Ok(())
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
        at: Timestamp::now(),
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

/// D05.4: the store-resident fake clock. `now_with_offset` = real wall time
/// plus the accumulated `SYS_CLOCK["offset_ms"]` offset — used by ALL aging
/// math (Algorithm B) and by `lim overlays`, so a separate process observes
/// the same advanced clock. `created_at` timestamps stay real wall time;
/// determinism comes from day-scale offsets, not from replacing the clock.
pub fn now_with_offset(store: &GraphStore) -> anyhow::Result<Timestamp> {
    let offset_ms = store
        .get_aux(SYS_CLOCK, "offset_ms")?
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    Ok(Timestamp(Timestamp::now().0 + offset_ms))
}

/// `advance_clock { by_secs }`: accumulate `by_secs * 1000` into
/// `SYS_CLOCK["offset_ms"]` (D05.4).
fn advance_clock(store: &GraphStore, by_secs: i64) -> anyhow::Result<()> {
    let current = store
        .get_aux(SYS_CLOCK, "offset_ms")?
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let mut txn = store.begin()?;
    txn.put_aux(
        SYS_CLOCK,
        "offset_ms",
        serde_json::json!(current + by_secs * 1000),
    )?;
    txn.commit(save_meta())?;
    Ok(())
}

/// The `WorkspaceBasis` file-content hash an Overlay's `base` captured for
/// `path` (its `Path` component), if any.
fn overlay_basis_file_hash(base: &WorkspaceBasis, path: &PathId) -> Option<ContentHash> {
    base.components
        .get(&JurisdictionKey::Path(path.clone()))
        .and_then(|c| match c {
            BasisComponent::FileContent { hash, .. } => Some(*hash),
            _ => None,
        })
}

/// Algorithm B, aging half: an `Active` overlay past its profile's threshold
/// surfaces one coalesced `offline:path:<p>` item (declared-transient
/// profiles escalate at `escalate_after`; others surface at `surface_after`).
fn age_overlay(
    store: &GraphStore,
    overlay: &Overlay,
    now: Timestamp,
    path: &PathId,
) -> anyhow::Result<()> {
    let age_ms = now.0 - overlay.created_at.0;
    let escalate_ms =
        i64::try_from(overlay.lifecycle.escalate_after.as_millis()).unwrap_or(i64::MAX);
    let surface_ms = i64::try_from(overlay.lifecycle.surface_after.as_millis()).unwrap_or(i64::MAX);
    let should_surface = if overlay.lifecycle.declared_transient {
        age_ms > escalate_ms
    } else {
        age_ms > surface_ms
    };
    if !should_surface {
        return Ok(());
    }
    let item = ReconciliationItem {
        id: ReconciliationItemId::new(),
        root_cause: format!("offline:path:{path}"),
        subjects: vec![overlay.subject],
        overlays: vec![overlay.id],
        repairs: vec![],
        created_at: now,
        status: ReconciliationStatus::Pending,
    };
    let mut txn = store.begin()?;
    ReconciliationQueue { store }.upsert_coalesced(&mut txn, item)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// The durable-state inputs `holder_return` re-verifies against: the file's
/// current observed hash/bytes, and the overlay's captured basis hash.
struct HolderReturnState {
    current_hash: Option<ContentHash>,
    current_bytes: Option<String>,
    captured_hash: Option<ContentHash>,
    merged: String,
}

/// Re-observe the durable file and rebuild the merge candidate: unchanged
/// since capture → the draft applies as-is; drifted → redo `three_way` at a
/// fresh basis (base = the overlay's captured durable bytes, ours = the
/// draft, theirs = the current durable bytes).
fn rebuild_holder_return_draft(
    store: &GraphStore,
    root: &Utf8Path,
    overlay: &Overlay,
    path: &PathId,
    draft_text: &str,
) -> anyhow::Result<HolderReturnState> {
    let abs = root.join(&path.0);
    let current = liminal_source::observe(&abs).map_err(|e| anyhow::anyhow!("{e}"))?;
    let current_hash = current.as_ref().map(|o| o.hash);
    let current_bytes = if abs.exists() {
        Some(std::fs::read_to_string(&abs)?)
    } else {
        None
    };
    let captured_hash = overlay_basis_file_hash(&overlay.base, path);

    let merged = if captured_hash == current_hash {
        draft_text.to_owned()
    } else {
        match (
            captured_hash.and_then(|h| blob::get(store, h).ok().flatten()),
            &current_bytes,
        ) {
            (Some(base_text), Some(current_text)) => {
                match merge::three_way(&base_text, draft_text, current_text) {
                    MergeOutcome::Disjoint { merged }
                    | MergeOutcome::UniqueOverlap { merged, .. } => merged,
                    MergeOutcome::Conflict => draft_text.to_owned(),
                }
            }
            _ => draft_text.to_owned(),
        }
    };

    Ok(HolderReturnState {
        current_hash,
        current_bytes,
        captured_hash,
        merged,
    })
}

/// `AutoApply` half of holder-return: commit through ILRP and retire the
/// overlay (D05.6 — accepted repair is a departure path).
fn commit_holder_return(
    store: &GraphStore,
    root: &Utf8Path,
    plan: &RepairPlan,
    evidence: &SafetyEvidence,
    overlay: OverlayId,
) -> anyhow::Result<()> {
    let executor = crate::executor::FsExecutor::with_store(root.to_owned(), store)?;
    let crash = crate::crash::EnvCrashInjector::from_env();
    let driver = IlrpDriver {
        store,
        executor: &executor,
        crash,
    };
    let id = driver.prepare(plan.clone(), evidence.clone())?;
    driver.run(id)?;
    record_repair(store, plan, "holder-return", evidence)?;

    let mut txn = store.begin()?;
    txn.delete_aux(JUR_OVERLAY, &overlay.to_string())?;
    resolve_covering_item(store, &mut txn, overlay)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// `NeedsReview` half of holder-return: leave the overlay `RepairProposed`
/// alongside one coalesced `conflict:path:<p>` item.
fn requeue_holder_return(
    store: &GraphStore,
    overlay: &Overlay,
    path: &PathId,
    plan_id: RepairId,
) -> anyhow::Result<()> {
    let now = now_with_offset(store)?;
    let mut updated = overlay.clone();
    updated.state = OverlayState::RepairProposed(plan_id);
    updated.last_activity = now;
    let item = ReconciliationItem {
        id: ReconciliationItemId::new(),
        root_cause: format!("conflict:path:{path}"),
        subjects: vec![overlay.subject],
        overlays: vec![overlay.id],
        repairs: vec![plan_id],
        created_at: now,
        status: ReconciliationStatus::Pending,
    };
    let mut txn = store.begin()?;
    txn.put_aux(
        JUR_OVERLAY,
        &overlay.id.to_string(),
        serde_json::to_value(&updated)?,
    )?;
    ReconciliationQueue { store }.upsert_coalesced(&mut txn, item)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// Algorithm B, holder-return half: re-verify (and re-merge if the durable
/// file drifted since the draft was captured) an `Active` overlay whose write
/// route just became available, then evaluate it exactly like any other save.
/// `AutoApply` commits through ILRP and retires the overlay (D05.6);
/// `NeedsReview` leaves it `RepairProposed` alongside one coalesced item.
fn holder_return(
    store: &GraphStore,
    root: &Utf8Path,
    profiles: &ProfileSet,
    overlay: &Overlay,
    path: &PathId,
) -> anyhow::Result<()> {
    let file_node = match overlay.subject {
        JurisdictionSubject::Node(n) => n,
        JurisdictionSubject::Relation(_) => anyhow::bail!("holder-return expects a file subject"),
    };
    let RepairOperation::WriteFile {
        contents: draft, ..
    } = &overlay.operation
    else {
        anyhow::bail!("holder-return overlay must carry a WriteFile draft");
    };
    let draft_text = String::from_utf8_lossy(draft).into_owned();
    let state = rebuild_holder_return_draft(store, root, overlay, path, &draft_text)?;

    // Persist blobs so the safety predicate can recompute disjointness.
    {
        let mut txn = store.begin()?;
        if let Some(c) = &state.current_bytes {
            blob::put(&mut txn, c)?;
        }
        blob::put(&mut txn, &state.merged)?;
        txn.commit(save_meta())?;
    }

    let plan = build_save_plan(
        file_node,
        path,
        state.captured_hash,
        state.current_hash,
        state.current_bytes.as_deref().map(str::as_bytes),
        state.merged.as_bytes(),
    );

    let checker = Checker { store, profiles };
    let decision = checker
        .evaluate_repair(&plan)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    persist_plan(store, &plan)?;
    persist_decision(store, plan.id, &decision)?;

    match decision {
        RepairDecision::AutoApply { evidence } => {
            commit_holder_return(store, root, &plan, &evidence, overlay.id)
        }
        RepairDecision::NeedsReview { .. } => requeue_holder_return(store, overlay, path, plan.id),
    }
}

/// Algorithm B: age every `Active` overlay and re-verify any whose write
/// route has returned. Runs at workspace open (after recovery), after every
/// scenario step, and before `lim overlays` / `lim check` render.
pub fn sweep_overlays(store: &GraphStore, root: &Utf8Path) -> anyhow::Result<()> {
    let profiles = ProfileSet::phase_minus_1();
    let now = now_with_offset(store)?;

    for (_key, value) in store.scan_aux(JUR_OVERLAY)? {
        let overlay: Overlay = serde_json::from_value(value)?;
        if overlay.state != OverlayState::Active {
            continue;
        }
        let Some(path) = overlay_operation_path(&overlay.operation) else {
            continue;
        };
        if holder_available(store, path.0.as_str())? {
            holder_return(store, root, &profiles, &overlay, &path)?;
        } else {
            age_overlay(store, &overlay, now, &path)?;
        }
    }
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

/// `holder_unavailable`: the durable Holder loses its write route (D05.1).
/// The value is fixed per the schema — `{"mode":"read-only"}`, independent of
/// the step's own descriptive `mode` field — the file stays readable.
fn mark_holder_unavailable(store: &GraphStore, step: &crate::scenario::Step) -> anyhow::Result<()> {
    let path = field(step, "path")?;
    let mut txn = store.begin()?;
    txn.put_aux(
        SYS_UNAVAILABLE,
        path,
        serde_json::json!({"mode": "read-only"}),
    )?;
    txn.commit(save_meta())?;
    Ok(())
}

/// `holder_available`: the durable Holder's write route returns (D05.1).
/// Deletes the `SYS_UNAVAILABLE` key; the generic post-step sweep (Algorithm
/// B) then re-verifies any Active overlay waiting on this path.
fn mark_holder_available(store: &GraphStore, step: &crate::scenario::Step) -> anyhow::Result<()> {
    let path = field(step, "path")?;
    let mut txn = store.begin()?;
    txn.delete_aux(SYS_UNAVAILABLE, path)?;
    txn.commit(save_meta())?;
    Ok(())
}

/// `advance_clock { by_secs }` (D05.4; AM-5.1).
fn apply_advance_clock(store: &GraphStore, step: &crate::scenario::Step) -> anyhow::Result<()> {
    let by_secs = step
        .extra
        .get("by_secs")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| anyhow::anyhow!("advance_clock missing by_secs"))?;
    advance_clock(store, by_secs)
}

/// `resolver_observe { source, price, at }` (M08.6, AM-8.1): script one
/// effectful observation through `ScriptedStockResolver::observe` (D08.5's
/// demo `ReplayableResolver`), then inject it as a graph transaction
/// (`ToyWorkspace::inject_observation`, M08 Algorithm B).
fn apply_resolver_observe(
    ws: &mut ToyWorkspace,
    step: &crate::scenario::Step,
) -> anyhow::Result<()> {
    let source_name = field(step, "source")?;
    let price = step
        .extra
        .get("price")
        .and_then(toml::Value::as_float)
        .ok_or_else(|| anyhow::anyhow!("resolver_observe missing price"))?;
    let at = field(step, "at")?;

    let source = SourceId::from_name(source_name);
    let observed_at = parse_utc_timestamp(at)?;
    let mut resolver = ScriptedStockResolver::scripted(vec![Observation {
        source,
        observed_at,
        payload: serde_json::json!({ "price": price }),
    }]);
    let observed = resolver.observe(&source);
    ws.inject_observation(observed)?;
    Ok(())
}

/// `query { query, perspective, node?, client?, path? }` (M08.7, AM-8.1):
/// execute one of the four M08.4 queries against the CURRENT workspace state
/// and persist its canonical output at
/// `SYS_BLOB["query/<query>:<perspective-label>"]` — the m08 milestone test
/// reads it back (and, for `workspace_export`, compares it against an
/// independently re-executed durable export).
///
/// `perspective = "client"` opens a FRESH `ClientSession` buffer over the
/// runner's own tracked scratch bytes (`buffers` — the same bytes `save`
/// reads as "ours") so the query sees precisely what the scenario's
/// `buffer_edit` steps have produced so far, without perturbing the
/// save-flow's own bookkeeping. `perspective = "durable"` needs no client.
fn apply_query(
    ws: &mut ToyWorkspace,
    buffers: &BTreeMap<(String, String), Vec<u8>>,
    step: &crate::scenario::Step,
) -> anyhow::Result<()> {
    let query_name = field(step, "query")?.to_owned();
    let node_alias = step.extra.get("node").and_then(|v| v.as_str());

    let (perspective, label) = match field(step, "perspective")? {
        "durable" => (
            BasisPerspective::DurableOnly,
            crate::queries::perspective_label(&BasisPerspective::DurableOnly, None),
        ),
        "client" => {
            let client_name = field(step, "client")?;
            let path = field(step, "path")?;
            let bytes = buffers
                .get(&(client_name.to_owned(), path.to_owned()))
                .ok_or_else(|| {
                    anyhow::anyhow!("query: no tracked buffer for {client_name}/{path}")
                })?;
            let text = String::from_utf8_lossy(bytes).into_owned();
            let client = ClientId::new();
            {
                let mut session = ws.client(client);
                let buffer = session.open_buffer(PathId(path.into()));
                session.edit(buffer, &text);
            }
            let perspective = BasisPerspective::ClientScoped { client };
            let label = crate::queries::perspective_label(&perspective, Some(client_name));
            (perspective, label)
        }
        other => anyhow::bail!("query: unknown perspective {other:?}"),
    };

    let basis = ws.basis(perspective)?;
    let store = ws.store();
    let world = crate::queries::LiveWorld::new(store, ws.root());
    let mut deps = liminal_revision::ComponentDeps::default();
    let output = match query_name.as_str() {
        "render_block" => {
            let node = node_for_alias(
                store,
                node_alias.ok_or_else(|| anyhow::anyhow!("render_block query needs a node"))?,
            )?;
            crate::queries::RenderBlock {
                world: &world,
                node,
                perspective_label: label.clone(),
            }
            .execute(&basis, &mut deps)
        }
        "backlinks" => {
            let node = node_for_alias(
                store,
                node_alias.ok_or_else(|| anyhow::anyhow!("backlinks query needs a node"))?,
            )?;
            crate::queries::Backlinks {
                world: &world,
                node,
                perspective_label: label.clone(),
            }
            .execute(&basis, &mut deps)
        }
        "workspace_export" => crate::queries::WorkspaceExport {
            world: &world,
            perspective_label: label.clone(),
        }
        .execute(&basis, &mut deps),
        "ai_context_stub" => {
            let node = node_for_alias(
                store,
                node_alias.ok_or_else(|| anyhow::anyhow!("ai_context_stub query needs a node"))?,
            )?;
            crate::queries::AiContextStub {
                world: &world,
                focus: node,
                perspective_label: label.clone(),
            }
            .execute(&basis, &mut deps)
        }
        other => anyhow::bail!("query: unknown query kind {other:?}"),
    };

    let key = format!("query/{query_name}:{label}");
    let mut txn = store.begin()?;
    txn.put_aux(SYS_BLOB, &key, serde_json::Value::String(output))?;
    txn.commit(save_meta())?;
    Ok(())
}

/// The node an `{#alias}` durable id names (`JUR_ALIAS[alias]["node"]`).
fn node_for_alias(store: &GraphStore, alias: &str) -> anyhow::Result<NodeId> {
    let value = store
        .get_aux(JUR_ALIAS, alias)?
        .ok_or_else(|| anyhow::anyhow!("no alias {alias:?}"))?;
    value["node"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("alias {alias:?} has no node"))?
        .parse()
        .map_err(|e| anyhow::anyhow!("bad node id for alias {alias:?}: {e}"))
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

/// The per-step scenario runner seam (AM-11.4): the SAME setup + step
/// machinery `exec` has always run, factored so a caller (the M11 trace
/// pipeline) can apply steps ONE AT A TIME and observe the workspace
/// between them — checker findings, reconciliation items, overlay
/// dispositions — per event, as v4 §-1.5's scorecard requires. `exec`
/// delegates here; its behavior is unchanged.
#[derive(Debug)]
pub struct StepRunner<X: ExternalExecutor, C: CrashInjector> {
    root: Utf8PathBuf,
    /// Always `Some` between public calls; `Option` only so `daemon_restart`
    /// can DROP the old handle (releasing the store's exclusive advisory
    /// lock) strictly before reopening — the same drop-then-open order
    /// `exec` has always used (AM-5.1).
    ws: Option<ToyWorkspace>,
    profiles: ProfileSet,
    buffers: BTreeMap<(String, String), Vec<u8>>,
    /// Base text per path, captured BEFORE any foreign edit (the buffer's
    /// base for three-way merges).
    bases: BTreeMap<String, String>,
    executor: X,
    crash: C,
}

impl<X: ExternalExecutor, C: CrashInjector> StepRunner<X, C> {
    /// Set up a workspace exactly as `exec` always has: write setup files,
    /// seed setup buffers from disk, capture bases from setup text, open,
    /// ingest files + graph, and reopen once (AM-8.11).
    ///
    /// # Errors
    /// IO/store failures during setup or ingest.
    pub fn open(
        root: &Utf8Path,
        setup: &crate::scenario::Setup,
        executor: X,
        crash: C,
    ) -> anyhow::Result<Self> {
        // Write setup files to disk.
        for file in &setup.files {
            let path = root.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, file.text.as_bytes())?;
        }

        let mut buffers: BTreeMap<(String, String), Vec<u8>> = BTreeMap::new();
        for buf in &setup.buffers {
            let path = root.join(&buf.path);
            buffers.insert(
                (buf.client.clone(), buf.path.clone()),
                std::fs::read(&path)?,
            );
        }

        // Base text per path, captured BEFORE any foreign edit (the buffer's base).
        let mut bases: BTreeMap<String, String> = BTreeMap::new();
        for file in &setup.files {
            bases.insert(file.path.clone(), file.text.clone());
        }

        let ws = ToyWorkspace::open(root)?;
        let profiles = ProfileSet::phase_minus_1();

        // Ingest files and graph entries (M03).
        ingest_files(ws.store(), &setup.files)?;
        ingest_graph(ws.store(), &setup.graph)?;

        // M08.7: `seed_durable_inputs` (D06.5) runs at `open`, BEFORE the ingest
        // above — so this session's own `AvailableInputs` still lacks the
        // just-ingested file component. Reopen once, mirroring a fresh process
        // over the persisted store (the same gotcha `queries.rs`'s own unit
        // tests route around by opening twice, and the same reopen the
        // `daemon_restart` step already exercises, M05 AM-5.1). Inert for any
        // scenario that never calls `ws.basis()`/`AvailableInputs` — recovery and
        // the overlay sweep are no-ops with nothing pending yet.
        drop(ws);
        let ws = ToyWorkspace::open(root)?;

        Ok(Self {
            root: root.to_owned(),
            ws: Some(ws),
            profiles,
            buffers,
            bases,
            executor,
            crash,
        })
    }

    /// The always-present workspace handle (see the `ws` field doc).
    fn ws(&self) -> &ToyWorkspace {
        self.ws
            .as_ref()
            .expect("StepRunner workspace is always present")
    }

    /// Register a buffer mid-stream (M11 pipeline: `buffer_open→open_buffer`)
    /// over the file's CURRENT durable bytes — exactly what `open`'s
    /// setup-buffer seeding does at time zero. The path's merge base is
    /// captured on FIRST registration only (matching `exec`, where bases come
    /// from setup and are never overwritten).
    ///
    /// # Errors
    /// IO failure reading the file.
    pub fn open_buffer(&mut self, client: &str, path: &str) -> anyhow::Result<()> {
        let abs = self.root.join(path);
        let bytes = std::fs::read(&abs)?;
        self.bases
            .entry(path.to_owned())
            .or_insert_with(|| String::from_utf8_lossy(&bytes).into_owned());
        self.buffers
            .insert((client.to_owned(), path.to_owned()), bytes);
        Ok(())
    }

    /// Apply ONE scenario step, then run the post-step overlay sweep — the
    /// body of `exec`'s loop, verbatim.
    ///
    /// # Errors
    /// Unknown step kinds and any step/store/IO failure.
    pub fn step(&mut self, step: &crate::scenario::Step) -> anyhow::Result<()> {
        match step.kind.as_str() {
            "buffer_edit" => apply_buffer_edit(step, &mut self.buffers)?,
            "foreign_edit" => {
                apply_foreign_edit(step, &self.root)?;
                // Reconcile immediately: any comment Relation whose target's
                // `{#id}` vanished gets a two-step repair DAG proposed (M04.3
                // ingest::foreign_change). Refused for AUTOMATIC acceptance
                // (heuristic reattachment), so it persists as NeedsReview.
                plan_foreign_changes(&self.profiles, &self.root, self.ws().store())?;
            }
            "save" => {
                let client = field(step, "client")?;
                let path = field(step, "path")?;
                let store = self.ws().store();
                let driver = IlrpDriver {
                    store,
                    executor: &self.executor,
                    crash: &self.crash,
                };
                perform_save(
                    &driver,
                    &self.profiles,
                    &self.root,
                    &self.bases,
                    &self.buffers,
                    client,
                    path,
                )?;
            }
            "resolver_observe" => {
                let ws = self
                    .ws
                    .as_mut()
                    .expect("StepRunner workspace is always present");
                apply_resolver_observe(ws, step)?;
            }
            "query" => {
                let ws = self
                    .ws
                    .as_mut()
                    .expect("StepRunner workspace is always present");
                apply_query(ws, &self.buffers, step)?;
            }
            "accept_repair" => {
                // AM-17.12: a caller identity is required before any acceptance
                // effect. This records the caller's claim, not authentication.
                let actor = field(step, "actor")?.parse::<liminal_id::ActorId>()?;
                accept_repair(self.ws().store(), &self.root, actor)?;
            }
            "holder_unavailable" => mark_holder_unavailable(self.ws().store(), step)?,
            "holder_available" => mark_holder_available(self.ws().store(), step)?,
            "advance_clock" => apply_advance_clock(self.ws().store(), step)?,
            // AM-5.1: drop + reopen the workspace — the store's advisory lock
            // is exclusive, so the old handle must release it first. Exercises
            // open-time sweep/recovery in-process (D05.1: SYS_UNAVAILABLE
            // survives; Algorithm B).
            "daemon_restart" => {
                self.ws = None; // drop first: release the advisory lock
                self.ws = Some(ToyWorkspace::open(&self.root)?);
            }
            other => anyhow::bail!("step kind {other:?} not implemented until M3/M4"),
        }
        sweep_overlays(self.ws().store(), &self.root)?;
        Ok(())
    }

    /// The current workspace (replaced across `daemon_restart` steps).
    #[must_use]
    pub fn workspace(&self) -> &ToyWorkspace {
        self.ws()
    }

    /// Mutable workspace access (`resolver_observe→inject_observation`).
    pub fn workspace_mut(&mut self) -> &mut ToyWorkspace {
        self.ws
            .as_mut()
            .expect("StepRunner workspace is always present")
    }

    /// The workspace root.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }
}

/// Run one scripted scenario to completion (or to an armed crash point).
pub fn exec<X: ExternalExecutor, C: CrashInjector>(
    root: &Utf8Path,
    scenario: &ScenarioScript,
    executor: X,
    crash: C,
) -> anyhow::Result<()> {
    let mut runner = StepRunner::open(root, &scenario.setup, executor, crash)?;
    for step in &scenario.steps {
        runner.step(step)?;
    }
    let ws = runner
        .ws
        .take()
        .expect("StepRunner workspace is always present");

    let store = ws.store();
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
    /// Canonical digest of the durable Basis observed at recovery.
    pub basis_digest: String,
    /// Independent duplicate-effect ledger digest: terminal intents, Basis,
    /// and world digest are hashed together rather than relying on one world
    /// projection to prove idempotence.
    pub effect_digest: String,
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

    // NORMALIZED, not raw (M17.5 F-40). The raw world digest hashes
    // `intent:<uuid>:<json>` with `StepAck.at` wall-clock timestamps and minted
    // ids inside, so every crash-evidence run produced a different
    // recovery/effect digest for EVERY boundary while first == second still
    // held within a run. The committed fault matrix could not be reproduced
    // and verify_crash_replay's equality was unsatisfiable by construction.
    // normalized_world_digest (M04 Algorithm F) exists for exactly this: worlds
    // differing only by minted ids and clocks hash equally, which is the
    // reproducibility ADR-0020 §1 asks of committed evidence. Within-run
    // idempotence is preserved: raw-equal implies normalized-equal.
    let world_digest =
        crate::digest::normalized_world_digest(root, store).map_err(|e| anyhow::anyhow!("{e}"))?;
    let basis = ws
        .basis(BasisPerspective::DurableOnly)
        .map_err(|e| anyhow::anyhow!("basis capture during recovery failed: {e}"))?;
    // TransactionId is an allocation identity, not Basis content; it changes
    // on every open. Exclude it so duplicate recovery compares semantic Basis
    // components and perspective rather than a fresh UUID.
    let basis_bytes = serde_json::to_vec(&(&basis.perspective, &basis.components))?;
    let basis_digest = blake3::hash(&basis_bytes).to_hex().to_string();
    let terminals_bytes = serde_json::to_vec(&terminals)?;
    let effect_digest = blake3::hash(
        [
            terminals_bytes.as_slice(),
            basis_bytes.as_slice(),
            world_digest.as_bytes(),
        ]
        .concat()
        .as_slice(),
    )
    .to_hex()
    .to_string();

    Ok(RecoverOutcome {
        terminals,
        world_digest,
        basis_digest,
        effect_digest,
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
    let executor = crate::executor::FsExecutor::new(dir.path().to_owned());
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

/// A fresh scratch store dir that removes itself on drop (M17.5 F-12).
fn tempdir_for(label: &str) -> anyhow::Result<liminal_scratch::ScratchDir> {
    Ok(liminal_scratch::ScratchDir::new(&format!("d044-{label}"))?)
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
