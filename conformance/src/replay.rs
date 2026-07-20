//! Freeze/replay harness (M08.8, v4 §42, §112, M08 Algorithm C): capture a
//! `WorkspaceBasis`'s EXACT pinned bytes into canonical JSON, then serve
//! reads purely from that JSON — no live store, no disk — so a query
//! re-executed against [`FrozenWorld`] after the workspace directory is
//! deleted reproduces the SAME output as the live run, byte-identically.

use std::collections::BTreeMap;

use liminal_daemon::ToyWorkspace;
use liminal_daemon::queries::World;
use liminal_graph::{Node, Relation, StateView, ns};
use liminal_id::{GraphRevisionId, JurisdictionKey, NodeId, PathId, RelationId};
use liminal_revision::{BasisComponent, WorkspaceBasis};
use serde::{Deserialize, Serialize};

/// Every aux namespace the toy defines (protocol §8) — frozen exhaustively
/// alongside nodes/relations/children so a `FrozenWorld` can answer any
/// query the live world could, not only the ones exercised today.
const ALL_NAMESPACES: &[&str] = &[
    ns::ILRP_INTENT,
    ns::JUR_ALIAS,
    ns::JUR_PLAN,
    ns::JUR_DECISION,
    ns::JUR_REPAIR,
    ns::JUR_OVERLAY,
    ns::JUR_OVERLAY_LOG,
    ns::JUR_RECONCILE,
    ns::SYS_BLOB,
    ns::SYS_UNAVAILABLE,
    ns::SYS_CLOCK,
    ns::SYS_EPOCH,
];

/// A frozen snapshot of one graph revision — every part of `StateView` a
/// query can read, captured through its public accessors (`State` itself is
/// `pub(crate)` to `liminal-graph`, AM-8.12).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrozenGraphSnapshot {
    head: GraphRevisionId,
    nodes: BTreeMap<NodeId, Node>,
    relations: BTreeMap<RelationId, Relation>,
    children: BTreeMap<NodeId, Vec<NodeId>>,
    aux: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

impl FrozenGraphSnapshot {
    fn capture(view: &StateView) -> Self {
        let nodes: BTreeMap<NodeId, Node> = view.nodes().map(|n| (n.id, n.clone())).collect();
        let relations: BTreeMap<RelationId, Relation> =
            view.relations().map(|r| (r.id, r.clone())).collect();
        let mut children = BTreeMap::new();
        for &id in nodes.keys() {
            let c = view.children(id);
            if !c.is_empty() {
                children.insert(id, c.to_vec());
            }
        }
        let mut aux = BTreeMap::new();
        for ns_name in ALL_NAMESPACES {
            let entries: BTreeMap<String, serde_json::Value> = view
                .scan_aux(ns_name)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !entries.is_empty() {
                aux.insert((*ns_name).to_owned(), entries);
            }
        }
        Self {
            head: view.head(),
            nodes,
            relations,
            children,
            aux,
        }
    }

    fn into_view(self) -> StateView {
        StateView::synthetic(
            self.head,
            self.nodes,
            self.relations,
            self.children,
            self.aux,
        )
    }
}

/// The frozen payload: a Basis plus the exact bytes/state it pins. `blobs` is
/// keyed by `blob_key` — the SAME `buf/…`/`file/…`/`obs/…` families
/// `SYS_BLOB` already uses, plus one label for the graph snapshot — NOT the
/// `JurisdictionKey` a component is filed under in `basis.components` (see
/// `blob_key`'s doc comment for why those must differ).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrozenPayload {
    basis: WorkspaceBasis,
    blobs: BTreeMap<String, serde_json::Value>,
}

/// The `blobs` map key for `component` (M08 Algorithm C). NOT the
/// `JurisdictionKey` a component is filed under in `basis.components` — a
/// `BufferGeneration` and a `FileContent` component can share the SAME
/// `JurisdictionKey` (`Path(path)`, differing only by WHICH perspective
/// selected them), so the blob key must depend on the component's OWN
/// identity. These are exactly the keys [`liminal_daemon::queries::LiveWorld`]
/// already reads/writes (`buf/…`, `obs/…`) or `runner::ingest_files`'s
/// `file/…` convention, plus one label for the graph snapshot (there is at
/// most one per Basis).
fn blob_key(component: &BasisComponent) -> anyhow::Result<String> {
    match component {
        BasisComponent::BufferGeneration {
            client,
            buffer,
            generation,
            ..
        } => Ok(format!("buf/{client}/{buffer}/{generation}")),
        BasisComponent::FileContent { path, .. } => Ok(format!("file/{path}")),
        BasisComponent::Observation { source, hash, .. } => Ok(format!("obs/{source}/{hash}")),
        BasisComponent::GraphSnapshot { .. } => Ok(liminal_revision::graph_key().to_string()),
        BasisComponent::ObjectContent { .. }
        | BasisComponent::GitCommit { .. }
        | BasisComponent::ExternalRevision { .. } => {
            anyhow::bail!("freeze: {component:?} component kind is not used in Phase -1")
        }
    }
}

/// Freeze `basis`'s exact pinned bytes out of `workspace` into canonical JSON
/// (M08 Algorithm C). Every component in `basis.components` contributes one
/// `blobs` entry, keyed by `blob_key`: the buffer-generation blob, the
/// durable file bytes, the observation payload, or the serialized graph
/// snapshot.
///
/// # Errors
/// Propagates store/IO failures reading any pinned component's bytes, or a
/// `BasisComponent` kind Phase -1 never produces (`ObjectContent`,
/// `GitCommit`, `ExternalRevision`).
pub fn freeze(basis: &WorkspaceBasis, workspace: &ToyWorkspace) -> anyhow::Result<String> {
    let mut blobs = BTreeMap::new();
    for component in basis.components.values() {
        let key = blob_key(component)?;
        let value = match component {
            BasisComponent::BufferGeneration { content_hash, .. } => {
                let value = workspace
                    .store()
                    .get_aux(ns::SYS_BLOB, &key)?
                    .ok_or_else(|| anyhow::anyhow!("freeze: missing basis-pinned blob {key}"))?;
                let bytes = value.as_str().ok_or_else(|| {
                    anyhow::anyhow!("freeze: basis-pinned buffer blob {key} is not text")
                })?;
                if let Some(expected) = content_hash {
                    let actual = liminal_id::ContentHash::of(bytes.as_bytes());
                    anyhow::ensure!(
                        actual == *expected,
                        "freeze: buffer blob {key} no longer matches the basis-pinned hash \
                         (pinned {expected}, found {actual})"
                    );
                }
                value
            }
            BasisComponent::Observation { hash, .. } => {
                let value = workspace
                    .store()
                    .get_aux(ns::SYS_BLOB, &key)?
                    .ok_or_else(|| anyhow::anyhow!("freeze: missing basis-pinned blob {key}"))?;
                let bytes = serde_json::to_vec(&value)?;
                let actual = liminal_id::ContentHash::of(&bytes);
                anyhow::ensure!(
                    actual == *hash,
                    "freeze: observation blob {key} no longer matches the basis-pinned hash \
                     (pinned {hash}, found {actual})"
                );
                value
            }
            BasisComponent::FileContent { path, hash } => {
                let bytes = std::fs::read(workspace.root().join(&path.0))?;
                let actual = liminal_id::ContentHash::of(&bytes);
                anyhow::ensure!(
                    actual == *hash,
                    "freeze: {path} on disk no longer matches the basis-pinned hash \
                     (pinned {hash}, found {actual}) — a freeze must capture exactly \
                     the bytes the Basis recorded, never whatever is there now"
                );
                serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned())
            }
            BasisComponent::GraphSnapshot { revision } => {
                let view = workspace.store().state_at(*revision)?;
                serde_json::to_value(FrozenGraphSnapshot::capture(&view))?
            }
            BasisComponent::ObjectContent { .. }
            | BasisComponent::GitCommit { .. }
            | BasisComponent::ExternalRevision { .. } => {
                unreachable!("blob_key already rejected this component kind")
            }
        };
        blobs.insert(key, value);
    }
    let payload = FrozenPayload {
        basis: basis.clone(),
        blobs,
    };
    Ok(serde_json::to_string(&payload)?)
}

/// A workspace served purely from a frozen JSON payload — no live store, no
/// disk. Implements [`World`] so the SAME query code that runs over
/// `LiveWorld` runs here byte-identically (v4 §24, §42).
#[derive(Debug)]
pub struct FrozenWorld {
    basis: WorkspaceBasis,
    blobs: BTreeMap<String, serde_json::Value>,
}

impl FrozenWorld {
    /// Load a `FrozenWorld` from a [`freeze`]-produced JSON string.
    ///
    /// # Errors
    /// Propagates JSON deserialization failures.
    pub fn load(json: &str) -> anyhow::Result<Self> {
        let payload: FrozenPayload = serde_json::from_str(json)?;
        Ok(Self {
            basis: payload.basis,
            blobs: payload.blobs,
        })
    }

    /// The frozen Basis this world serves.
    #[must_use]
    pub fn basis(&self) -> &WorkspaceBasis {
        &self.basis
    }
}

impl World for FrozenWorld {
    fn graph_at(&self, rev: GraphRevisionId) -> StateView {
        let key = liminal_revision::graph_key().to_string();
        let value = self
            .blobs
            .get(&key)
            .cloned()
            .expect("FrozenWorld: no frozen graph snapshot for graph_key()");
        let snapshot: FrozenGraphSnapshot =
            serde_json::from_value(value).expect("FrozenWorld: frozen graph snapshot is malformed");
        assert_eq!(
            snapshot.head, rev,
            "FrozenWorld: requested revision does not match the frozen snapshot"
        );
        snapshot.into_view()
    }

    fn resolved_text(&self, path: &PathId, component: Option<&BasisComponent>) -> String {
        let key = match component {
            Some(BasisComponent::BufferGeneration {
                client,
                buffer,
                generation,
                ..
            }) => format!("buf/{client}/{buffer}/{generation}"),
            _ => format!("file/{path}"),
        };
        self.blobs
            .get(&key)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .unwrap_or_default()
    }

    fn file_paths(&self) -> Vec<PathId> {
        let graph_key = liminal_revision::graph_key();
        self.basis
            .components
            .keys()
            .filter_map(|k| match k {
                JurisdictionKey::Path(p) if *k != graph_key => Some(p.clone()),
                _ => None,
            })
            .collect()
    }
}
