//! The R4 §10 toy workspace: one file-held paragraph, one graph-native
//! comment Relation, two dirty clients, foreign edits, an unavailable Holder,
//! and every repair flowing through the ONE interpreter.

use camino::Utf8Path;
use liminal_graph::GraphStore;
use liminal_id::ClientId;
use liminal_jurisdiction::{Checker, ProfileSet, ReconciliationQueue, RepairRecord};
use liminal_revision::{AvailableInputs, BasisPerspective, PerspectiveError, WorkspaceBasis};

use crate::session::ClientSession;

/// The assembled toy workspace: store + profiles + inputs.
#[derive(Debug)]
pub struct ToyWorkspace {
    store: GraphStore,
    profiles: ProfileSet,
    #[allow(dead_code, reason = "wired up by session plumbing at M6")]
    inputs: AvailableInputs,
}

impl ToyWorkspace {
    /// Open the workspace rooted at `root`: state lives at `<root>/state/`;
    /// opening sweeps abandoned staged files (`liminal_source::scan_staged`)
    /// and runs ILRP recovery over every nonterminal intent
    /// (v4 §7.8 step 5 — recovery is the FIRST thing that happens).
    pub fn open(root: &Utf8Path) -> Result<Self, WorkspaceError> {
        let store_dir = root.join("state");
        let store = GraphStore::open(&store_dir)?;

        // Sweep abandoned staged files (D02.4: delete all).
        for staged in liminal_source::scan_staged(root)? {
            staged.abandon()?;
        }

        // Run ILRP recovery over every nonterminal intent. The executor is
        // store-aware so a resumed InsertSourceId step can resolve its alias.
        let executor = crate::executor::FsExecutor::with_store(root.to_owned(), &store)?;
        let driver = liminal_jurisdiction::IlrpDriver {
            store: &store,
            executor: &executor,
            crash: liminal_jurisdiction::NoCrash,
        };
        let _recovery_outcomes = driver.recover_all()?;

        // Algorithm B (M05): age Active overlays and re-verify any whose
        // write route returned while the process was down, BEFORE the
        // workspace is handed to a caller.
        crate::runner::sweep_overlays(&store, root)
            .map_err(|e| WorkspaceError::Sweep(e.to_string()))?;

        Ok(Self {
            store,
            profiles: ProfileSet::phase_minus_1(),
            inputs: AvailableInputs::default(),
        })
    }

    /// The interpretive checker over this workspace.
    #[must_use]
    pub fn checker(&self) -> Checker<'_> {
        Checker {
            store: &self.store,
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
        for (alias, value) in self.store.scan_aux(liminal_graph::ns::JUR_ALIAS).ok()? {
            if value.get("node").and_then(|v| v.as_str()) == Some(node_str.as_str()) {
                return Some(alias);
            }
        }
        None
    }

    /// The core Reconciliation Queue (R4 §9; no ticket-list subsystem underlies it).
    #[must_use]
    pub fn reconciliation(&self) -> ReconciliationQueue<'_> {
        ReconciliationQueue { store: &self.store }
    }

    /// Capture one immutable Basis under a perspective (Law 3D/3J).
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the M6 implementation moves the perspective into the captured WorkspaceBasis"
    )]
    pub fn basis(&self, perspective: BasisPerspective) -> Result<WorkspaceBasis, PerspectiveError> {
        let _ = perspective;
        todo!("Phase -1 M6: perspective capture via liminal_revision::resolve")
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
        for (_key, value) in self.store.scan_aux(liminal_graph::ns::JUR_REPAIR)? {
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
        &self.store
    }

    /// Mutable input map (session plumbing).
    #[allow(dead_code, reason = "wired up by session plumbing at M6")]
    pub(crate) fn inputs_mut(&mut self) -> &mut AvailableInputs {
        &mut self.inputs
    }
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
