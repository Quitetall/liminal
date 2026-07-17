//! Scenario runner: `liminald exec` and `liminald recover` semantics (M02
//! Algorithm F).
//!
//! `exec` loads a scenario, sets up files + buffers, runs steps (buffer_edit,
//! save), and exits. `recover` opens the workspace (which runs ILRP recovery)
//! and prints terminal states.

use camino::Utf8Path;
use liminal_graph::ns::ILRP_INTENT;
use liminal_id::{
    ClientId, ContentHash, IdempotencyKey, JurisdictionSubject, NodeId, PathId, RepairId,
    RepairStepId, TransactionId,
};
use liminal_jurisdiction::{
    CrashInjector, ExternalExecutor, IlrpDriver, IntentState, ProposedMutation, RepairOperation,
    RepairPlan, SafetyEvidence, StatePredicate,
};
use liminal_revision::{BasisPerspective, WorkspaceBasis};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::scenario::ScenarioScript;
use crate::workspace::ToyWorkspace;

/// Build a save-as-Promotion `RepairPlan` for a buffer save step.
fn build_save_plan(
    root: &Utf8Path,
    buffers: &BTreeMap<(String, String), Vec<u8>>,
    client: &str,
    path: &str,
) -> anyhow::Result<(RepairPlan, SafetyEvidence)> {
    let key = (client.to_owned(), path.to_owned());
    let new_bytes = buffers
        .get(&key)
        .ok_or_else(|| anyhow::anyhow!("buffer not found: {client}/{path}"))?
        .clone();

    let rel = PathId(path.into());
    let abs = root.join(path);
    let base = liminal_source::observe(&abs).map_err(|e| anyhow::anyhow!("{e}"))?;

    let expected_prestate = match &base {
        Some(obs) => StatePredicate::FileContent {
            path: rel.clone(),
            hash: obs.hash,
        },
        None => StatePredicate::FileAbsent { path: rel.clone() },
    };
    let expected_poststate = StatePredicate::FileContent {
        path: rel.clone(),
        hash: ContentHash::of(&new_bytes),
    };

    let step_id = RepairStepId::new();
    let plan = RepairPlan {
        id: RepairId::new(),
        basis: WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::ClientScoped {
                client: ClientId::new(),
            },
            components: match &base {
                Some(obs) => BTreeMap::from([(
                    liminal_id::JurisdictionKey::Path(rel.clone()),
                    liminal_revision::BasisComponent::FileContent {
                        path: rel.clone(),
                        hash: obs.hash,
                    },
                )]),
                None => BTreeMap::new(),
            },
        },
        steps: BTreeMap::from([(
            step_id,
            ProposedMutation {
                id: step_id,
                subject: JurisdictionSubject::Node(NodeId::new()),
                operation: RepairOperation::WriteFile {
                    path: rel,
                    contents: new_bytes,
                },
                expected_prestate,
                expected_poststate,
                idempotency_key: IdempotencyKey::new(),
            },
        )]),
        dependencies: vec![],
        inverse: None,
    };
    let evidence = SafetyEvidence::StructurallyDisjoint {
        description: "single-step file promotion; durable file unchanged since buffer base \
                      (verified as ILRP prestate)"
            .into(),
    };
    Ok((plan, evidence))
}

/// Run one scripted scenario to completion (or to an armed crash point).
pub fn exec<X: ExternalExecutor, C: CrashInjector>(
    root: &Utf8Path,
    scenario: &ScenarioScript,
    executor: X,
    crash: C,
) -> anyhow::Result<()> {
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

    if !scenario.setup.graph.is_empty() {
        anyhow::bail!("setup.graph is M4");
    }

    let ws = ToyWorkspace::open(root)?;
    let store = ws.store();
    let driver = IlrpDriver {
        store,
        executor: &executor,
        crash,
    };

    for step in &scenario.steps {
        match step.kind.as_str() {
            "buffer_edit" => {
                let client = step
                    .extra
                    .get("client")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("buffer_edit missing client"))?;
                let path = step
                    .extra
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("buffer_edit missing path"))?;
                let find = step
                    .extra
                    .get("find")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("buffer_edit missing find"))?;
                let replace = step
                    .extra
                    .get("replace")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("buffer_edit missing replace"))?;

                let key = (client.to_owned(), path.to_owned());
                let buf = buffers
                    .get_mut(&key)
                    .ok_or_else(|| anyhow::anyhow!("buffer not found: {client}/{path}"))?;
                let text = String::from_utf8_lossy(buf).to_string();
                let pos = text
                    .find(find)
                    .ok_or_else(|| anyhow::anyhow!("find text not found in buffer: {find:?}"))?;
                *buf = format!("{}{}{}", &text[..pos], replace, &text[pos + find.len()..])
                    .into_bytes();
            }
            "save" => {
                let client = step
                    .extra
                    .get("client")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("save missing client"))?;
                let path = step
                    .extra
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("save missing path"))?;
                let (plan, evidence) = build_save_plan(root, &buffers, client, path)?;
                let id = driver.prepare(plan, evidence)?;
                driver.run(id)?;
            }
            other => anyhow::bail!("step kind {other:?} not implemented until M3/M4"),
        }
    }

    if let Some(ref expected) = scenario.expect.terminal {
        let intents = store.scan_aux(ILRP_INTENT)?;
        let normalize = |s: &str| s.to_lowercase().replace('-', "");
        let expected_norm = normalize(expected);
        for (_, value) in &intents {
            let intent: liminal_jurisdiction::RepairIntent = serde_json::from_value(value.clone())?;
            let actual_norm = normalize(&format!("{:?}", intent.state));
            if expected_norm != actual_norm {
                anyhow::bail!(
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
