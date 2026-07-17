//! Scripted per-client sessions with first-class `epoch + generation` buffer
//! Basis components (v4 §7.5). "neovim" and "phone" in the toy are exactly
//! these — no real editors exist in Phase -1.

use liminal_id::{BufferId, ClientId, PathId, RepairId};
use liminal_revision::BasisComponent;

use crate::workspace::{SaveError, ToyWorkspace};

/// One client's dirty-buffer session.
#[derive(Debug)]
pub struct ClientSession<'w> {
    workspace: &'w mut ToyWorkspace,
    client: ClientId,
}

impl<'w> ClientSession<'w> {
    pub(crate) fn new(workspace: &'w mut ToyWorkspace, client: ClientId) -> Self {
        Self { workspace, client }
    }

    /// The client this session belongs to.
    #[must_use]
    pub fn client(&self) -> ClientId {
        self.client
    }

    /// Open a buffer over a durable file subject.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the M6 implementation stores the PathId in AvailableInputs"
    )]
    pub fn open_buffer(&mut self, path: PathId) -> BufferId {
        let _ = path;
        todo!("Phase -1 M6: buffer registration in AvailableInputs (v4 §7.5)")
    }

    /// Apply an edit, producing a new `BufferGeneration` component (epoch +
    /// generation + content hash). Never rejected (Law 3B).
    pub fn edit(&mut self, buffer: BufferId, contents: &str) -> BasisComponent {
        let _ = (buffer, contents);
        todo!("Phase -1 M6: generation bump + component publication (v4 §7.5)")
    }

    /// Save the buffer. Save IS Promotion IS a `RepairPlan` through the one
    /// interpreter (R4 §4): operationally a repair from the buffer Overlay
    /// into the file Holder, executed via ILRP. An unavailable Holder still
    /// succeeds — the edit becomes a durable Overlay (Law 3B).
    pub fn save(&mut self, buffer: BufferId) -> Result<RepairId, SaveError> {
        let _ = buffer;
        let _ = &self.workspace;
        todo!("Phase -1 M2/M4: save-as-Promotion through IlrpDriver (R4 §4)")
    }
}
