//! Scenario runner: `liminald exec` and `liminald recover` semantics (M02
//! Algorithm F).
//!
//! `exec` loads a scenario, sets up files + buffers, runs steps (buffer_edit,
//! save), and exits. `recover` opens the workspace (which runs ILRP recovery)
//! and prints terminal states.

use camino::Utf8Path;
use liminal_graph::ns::{ILRP_INTENT, JUR_ALIAS, JUR_DECISION, JUR_PLAN, JUR_REPAIR, SYS_BLOB};
use liminal_graph::{
    GraphStore, IdentityRequirement, Node, NodeFlags, Operation, PayloadRef, Relation,
    RelationFlags, Target,
};
use liminal_id::{
    ClientId, ContentHash, EntityId, IdempotencyKey, IdentityGrade, JurisdictionKey,
    JurisdictionSubject, NodeId, PathId, RepairId, RepairStepId, RevisionId, TransactionId,
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
                        id: liminal_id::RelationId::new(),
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
    // poststate = old hash). Only constructible when we know the old bytes,
    // i.e. current_hash is Some.
    let inverse = current_hash.map(|old_hash| {
        let inv_step_id = RepairStepId::new();
        let inv_step = ProposedMutation {
            id: inv_step_id,
            subject: JurisdictionSubject::Node(file_node),
            // contents filled by the executor from SYS_BLOB[old_hash] is not
            // how WriteFile works; the inverse carries the actual old bytes,
            // which the save path passes in via `base`/current blob. Here we
            // leave contents empty and rely on plan_undo (M04.7) to hydrate.
            operation: RepairOperation::WriteFile {
                path: rel.clone(),
                contents: Vec::new(),
            },
            expected_prestate: expected_poststate,
            expected_poststate: StatePredicate::FileContent {
                path: rel.clone(),
                hash: old_hash,
            },
            idempotency_key: IdempotencyKey::new(),
        };
        InverseRepairPlan(Box::new(RepairPlan {
            id: RepairId::new(),
            basis: WorkspaceBasis {
                transaction: TransactionId::new(),
                perspective: BasisPerspective::DurableOnly,
                components: BTreeMap::new(),
            },
            steps: BTreeMap::from([(inv_step_id, inv_step)]),
            dependencies: vec![],
            inverse: None,
        }))
    });

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
                let plan =
                    build_save_plan(file_node, &rel, base_hash, current_hash, ours.as_bytes());
                persist_plan(store, &plan)?;
                return Ok(plan.id);
            }
        },
        _ => ours.clone(),
    };

    let plan = build_save_plan(file_node, &rel, base_hash, current_hash, merged.as_bytes());
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
            "foreign_edit" => apply_foreign_edit(step, root)?,
            "save" => {
                let client = field(step, "client")?;
                let path = field(step, "path")?;
                perform_save(&driver, &profiles, root, &bases, &buffers, client, path)?;
            }
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
