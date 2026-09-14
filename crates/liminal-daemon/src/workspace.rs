//! The R4 §10 toy workspace: one file-held paragraph, one graph-native
//! comment Relation, two dirty clients, foreign edits, an unavailable Holder,
//! and every repair flowing through the ONE interpreter.

use camino::Utf8Path;
use liminal_graph::{GraphStore, StoreOwner};
use liminal_id::{
    BufferId, ClientId, JurisdictionKey, PathId, SessionEpoch, SourceId, TransactionId,
};
use liminal_jurisdiction::{Checker, ProfileSet, ReconciliationQueue, RepairRecord};
use liminal_revision::{AvailableInputs, BasisPerspective, PerspectiveError, WorkspaceBasis};

use crate::session::ClientSession;

/// Per-open buffer bookkeeping (D06.3/D06.4): which path a buffer covers, its
/// monotonic generation, and the durable base hash it was opened against.
#[derive(Debug, Clone)]
pub(crate) struct BufferState {
    /// The client that owns this buffer (used by the save flow, M06.4).
    #[allow(dead_code, reason = "read by the ClientSession::save fold-in at M06.4")]
    pub(crate) client: ClientId,
    pub(crate) path: PathId,
    pub(crate) generation: u64,
    pub(crate) base_file_hash: liminal_id::ContentHash,
}

/// The assembled toy workspace: store + profiles + inputs + buffer bookkeeping.
#[derive(Debug)]
pub struct ToyWorkspace {
    owner: StoreOwner,
    profiles: ProfileSet,
    inputs: AvailableInputs,
    /// The workspace root (buffers read/write files under it).
    root: camino::Utf8PathBuf,
    /// This process's session epoch (D06.3 — prevents generation collisions
    /// across editor sessions over one durable subject). Computed read-only at
    /// open; persisted lazily on the first buffer.
    epoch: SessionEpoch,
    /// Whether this session's epoch has been persisted to `SYS_EPOCH` yet.
    epoch_persisted: bool,
    /// Open buffers, keyed by id.
    buffers: BTreeMap<BufferId, BufferState>,
}

use std::collections::BTreeMap;

const CURRENT_OBSERVATION_PREFIX: &str = "obs-current/";

/// Canonical restart-metadata key for one external observation (v4 §7.5;
/// M08 Algorithm B). Reactor writes and workspace reopen reads this one grammar.
pub(crate) fn current_observation_key(source: SourceId) -> String {
    format!("{CURRENT_OBSERVATION_PREFIX}{source}")
}

impl ToyWorkspace {
    /// Open the workspace rooted at `root`: state lives at `<root>/state/`;
    /// opening sweeps abandoned staged files (`liminal_source::scan_staged`)
    /// and runs ILRP recovery over every nonterminal intent
    /// (v4 §7.8 step 5 — recovery is the FIRST thing that happens).
    pub fn open(root: &Utf8Path) -> Result<Self, WorkspaceError> {
        let store_dir = root.join("state");
        let owner = StoreOwner::open(&store_dir)?;
        let store = owner.store();

        // Sweep abandoned staged files (D02.4: delete all).
        for staged in liminal_source::scan_staged(root)? {
            staged.abandon()?;
        }

        // Run ILRP recovery over every nonterminal intent. The executor is
        // store-aware so a resumed InsertSourceId step can resolve its alias.
        let executor = crate::executor::FsExecutor::with_store(root.to_owned(), store)?;
        let driver = liminal_jurisdiction::IlrpDriver {
            store,
            executor: &executor,
            crash: liminal_jurisdiction::NoCrash,
        };
        let _recovery_outcomes = driver.recover_all()?;

        // Algorithm B (M05): age Active overlays and re-verify any whose
        // write route returned while the process was down, BEFORE the
        // workspace is handed to a caller.
        crate::runner::sweep_overlays(store, root)
            .map_err(|e| WorkspaceError::Sweep(e.to_string()))?;

        // D06.3: this session's epoch is one past the durable counter. It is
        // computed read-only here and only PERSISTED when the session first
        // opens a buffer (`register_buffer`) — so recovery-only and CLI-read
        // opens are side-effect-free and world-digest idempotence holds.
        let epoch = SessionEpoch(peek_epoch(store)? + 1);

        // Seed the durable input map from every ingested file (D06.5: the
        // single source of truth for perspective resolution).
        let inputs = seed_durable_inputs(store, root)?;

        Ok(Self {
            owner,
            profiles: ProfileSet::phase_minus_1(),
            inputs,
            root: root.to_owned(),
            epoch,
            epoch_persisted: false,
            buffers: BTreeMap::new(),
        })
    }

    /// The interpretive checker over this workspace.
    #[must_use]
    pub fn checker(&self) -> Checker<'_> {
        Checker {
            store: self.store(),
            profiles: &self.profiles,
        }
    }

    /// The durable-id alias for a subject, if one is recorded in `JUR_ALIAS`.
    /// Renders subjects as their everyday `{#id}` in CLI output (R4 §3).
    #[must_use]
    pub fn alias_for(&self, subject: liminal_id::JurisdictionSubject) -> Option<String> {
        let liminal_id::JurisdictionSubject::Node(node) = subject else {
            return None;
        };
        let node_str = node.to_string();
        for (alias, value) in self.store().scan_aux(liminal_graph::ns::JUR_ALIAS).ok()? {
            if value.get("node").and_then(|v| v.as_str()) == Some(node_str.as_str()) {
                return Some(alias);
            }
        }
        None
    }

    /// The core Reconciliation Queue (R4 §9; no ticket-list subsystem underlies it).
    #[must_use]
    pub fn reconciliation(&self) -> ReconciliationQueue<'_> {
        ReconciliationQueue {
            store: self.store(),
        }
    }

    /// Capture one immutable Basis under a perspective (Law 3D/3J). Resolves
    /// against the current input map via `liminal_revision::resolve` (M06
    /// Algorithm A) at the store's current transaction frontier.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "frozen surface: basis(perspective: BasisPerspective) (M06 §Frozen surfaces)"
    )]
    pub fn basis(&self, perspective: BasisPerspective) -> Result<WorkspaceBasis, PerspectiveError> {
        let at = TransactionId::new();
        let mut basis = liminal_revision::resolve(&self.inputs, &perspective, at)?;
        // M08.2: every Basis pins the graph-store revision at the reserved
        // graph_key() so graph-reading queries (backlinks, export, ai-context)
        // record a dependency on it and can replay from a frozen Basis.
        if let Ok(revision) = self.store().head() {
            basis.components.insert(
                liminal_revision::graph_key(),
                liminal_revision::BasisComponent::GraphSnapshot { revision },
            );
        }
        Ok(basis)
    }

    /// The session epoch of this open (D06.3).
    #[must_use]
    pub(crate) fn epoch(&self) -> SessionEpoch {
        self.epoch
    }

    /// The workspace root (M08.7 AM-8.10: widened to `pub` — like `store()`,
    /// callers outside `liminal-daemon` need it to build a `LiveWorld` over an
    /// opened workspace, e.g. the `four_consumers` milestone test).
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// Register a freshly opened buffer's state (session plumbing). The first
    /// buffer of a session persists the epoch (D06.3), so only buffer-producing
    /// sessions bump the durable counter.
    pub(crate) fn register_buffer(&mut self, id: BufferId, state: BufferState) {
        if !self.epoch_persisted {
            let _ = persist_epoch(&self.owner.epoch_writer(), self.epoch);
            self.epoch_persisted = true;
        }
        self.buffers.insert(id, state);
    }

    /// A buffer's mutable state.
    pub(crate) fn buffer_mut(&mut self, id: BufferId) -> Option<&mut BufferState> {
        self.buffers.get_mut(&id)
    }

    /// A buffer's state (read-only).
    #[must_use]
    pub(crate) fn buffer(&self, id: BufferId) -> Option<&BufferState> {
        self.buffers.get(&id)
    }

    /// Publish a working component for a client over a subject (D06.2: the
    /// working map holds at most one component per key; the publisher
    /// guarantees uniqueness).
    pub(crate) fn publish_working(
        &mut self,
        client: ClientId,
        key: JurisdictionKey,
        component: liminal_revision::BasisComponent,
    ) {
        self.inputs
            .working
            .entry(client)
            .or_default()
            .insert(key, component);
    }

    /// Remove a client's working claim over a subject (D06.2 divergence
    /// fallback: the subject reverts to durable state).
    pub(crate) fn unpublish_working(&mut self, client: ClientId, key: &JurisdictionKey) {
        if let Some(map) = self.inputs.working.get_mut(&client) {
            map.remove(key);
        }
    }

    /// Record a working-holder selection for a client over a subject (AM-6.2).
    pub(crate) fn select_working(
        &mut self,
        client: ClientId,
        key: JurisdictionKey,
        buffer: BufferId,
    ) {
        self.inputs
            .working_selection
            .entry(client)
            .or_default()
            .insert(key, buffer);
    }

    /// Whether a client already has a published working claim over a subject.
    #[must_use]
    pub(crate) fn has_working(&self, client: ClientId, key: &JurisdictionKey) -> bool {
        self.inputs
            .working
            .get(&client)
            .is_some_and(|m| m.contains_key(key))
    }

    /// A scripted client session ("neovim", "phone" — R4 §10). No real
    /// editors: clients are `ClientId`s emitting `BufferGeneration` components.
    pub fn client(&mut self, id: ClientId) -> ClientSession<'_> {
        ClientSession::new(self, id)
    }

    /// All recorded repair decisions (backs `lim repairs`). Accepted
    /// `RepairRecord`s only, sorted by repair id (UUIDv7 ⇒ chronological).
    pub fn repairs(&self) -> Result<Vec<RepairRecord>, WorkspaceError> {
        let mut out = Vec::new();
        for (_key, value) in self.store().scan_aux(liminal_graph::ns::JUR_REPAIR)? {
            let record: RepairRecord = serde_json::from_value(value)
                .map_err(|e| liminal_graph::StoreError::Corrupt(e.to_string()))?;
            out.push(record);
        }
        out.sort_by_key(|r| r.repair);
        Ok(out)
    }

    /// The underlying store (harness access).
    #[must_use]
    pub fn store(&self) -> &GraphStore {
        self.owner.store()
    }

    /// Session capture receives no epoch or accepted-transaction capability.
    pub(crate) fn working_capture(&self) -> liminal_graph::WorkingCapture<'_> {
        self.owner.working_capture()
    }

    /// Mutable input map (session plumbing).
    #[allow(dead_code, reason = "wired up by session plumbing at M6")]
    pub(crate) fn inputs_mut(&mut self) -> &mut AvailableInputs {
        &mut self.inputs
    }
}

/// Read the durable `SYS_EPOCH["epoch"]` counter WITHOUT mutating it (D06.3).
fn peek_epoch(store: &GraphStore) -> Result<u64, WorkspaceError> {
    Ok(store
        .get_aux(liminal_graph::ns::SYS_EPOCH, "epoch")?
        .and_then(|v| v.as_u64())
        .unwrap_or(0))
}

/// Persist this session's epoch to `SYS_EPOCH["epoch"]` (its own txn). Called
/// once, lazily, when a session first opens a buffer.
fn persist_epoch(
    writer: &liminal_graph::EpochWriter<'_>,
    epoch: SessionEpoch,
) -> Result<(), WorkspaceError> {
    writer.persist(
        epoch,
        liminal_graph::TxnMeta {
            actor: None,
            origin: liminal_graph::Origin::Human,
            at: liminal_id::Timestamp::now(),
            provenance: Some("epoch:persist".into()),
            inverse: None,
        },
    )?;
    Ok(())
}

/// Reconstruct `AvailableInputs.durable` from committed file mirrors and
/// current-observation metadata (D06.5; M08 Algorithm B). Durable state is what
/// every perspective starts from, including after process restart.
fn seed_durable_inputs(
    store: &GraphStore,
    root: &Utf8Path,
) -> Result<AvailableInputs, WorkspaceError> {
    let mut inputs = AvailableInputs::default();
    for (key, value) in store.scan_aux(liminal_graph::ns::SYS_BLOB)? {
        if let Some(rel) = key.strip_prefix("file/") {
            let abs = root.join(rel);
            let Ok(bytes) = std::fs::read(&abs) else {
                continue;
            };
            let path = PathId(rel.into());
            inputs.durable.insert(
                JurisdictionKey::Path(path.clone()),
                liminal_revision::BasisComponent::FileContent {
                    path,
                    hash: liminal_id::ContentHash::of(&bytes),
                },
            );
        } else if let Some(key_source) = key.strip_prefix(CURRENT_OBSERVATION_PREFIX) {
            let component: liminal_revision::BasisComponent = serde_json::from_value(value)
                .map_err(|error| liminal_graph::StoreError::Corrupt(error.to_string()))?;
            let liminal_revision::BasisComponent::Observation { source, .. } = &component else {
                return Err(liminal_graph::StoreError::Corrupt(format!(
                    "{key}: current observation metadata has wrong component kind"
                ))
                .into());
            };
            if source.to_string() != key_source {
                return Err(liminal_graph::StoreError::Corrupt(format!(
                    "{key}: observation source does not match its key"
                ))
                .into());
            }
            inputs
                .durable
                .insert(JurisdictionKey::Source(*source), component);
        }
    }
    Ok(inputs)
}

/// Workspace failure.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Store failure.
    #[error(transparent)]
    Store(#[from] liminal_graph::StoreError),
    /// ILRP failure during open-time recovery.
    #[error(transparent)]
    Ilrp(#[from] liminal_jurisdiction::IlrpError),
    /// I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Open-time overlay sweep (Algorithm B) failed.
    #[error("overlay sweep: {0}")]
    Sweep(String),
}

/// Save failure. Note what is ABSENT: "Holder unavailable" is not an error —
/// capture never blocks; an unavailable write route produces a durable
/// Overlay and `save` still succeeds (Law 3B).
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// Workspace-level failure (I/O, store).
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}
