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

        // Run ILRP recovery over every nonterminal intent.
        let executor = crate::executor::FsExecutor::new(root.to_owned());
        let driver = liminal_jurisdiction::IlrpDriver {
            store: &store,
            executor: &executor,
            crash: liminal_jurisdiction::NoCrash,
        };
        let _recovery_outcomes = driver.recover_all()?;

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

    /// The core Reconciliation Queue (R4 §9; agenda-free by construction).
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

    /// All recorded repair decisions (backs `lim repairs`).
    pub fn repairs(&self) -> Result<Vec<RepairRecord>, WorkspaceError> {
        todo!("Phase -1 M4: repair record listing (R4 §6)")
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
