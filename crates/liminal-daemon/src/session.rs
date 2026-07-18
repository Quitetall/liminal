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
        // ── STUB (M04.6, T2). Spec = M04 Algorithm C. ──
        //
        // save IS Promotion IS a RepairPlan through the ONE IlrpDriver (R4 §4).
        // 1. (path, base_hash, ours) = buffer state (M6 moves this into
        //    AvailableInputs; until then read from the runner's buffer map).
        // 2. persist SYS_BLOB[blake3(ours)] (base blob persisted at open).
        // 3. if !holder_available(store, path) → M5 branch (stub: always avail).
        // 4. outcome = merge::three_way(SYS_BLOB[base], ours, current file bytes):
        //      Disjoint | UniqueOverlap → plan = save-promotion builder (merged);
        //      Conflict → persist JUR_PLAN[draft plan carrying ours] (+ M5
        //          Overlay + item); return Ok(plan.id) — capture never rejected.
        // 5. one txn: JUR_PLAN[plan] + JUR_DECISION[checker.evaluate_repair(&plan)].
        // 6. AutoApply{evidence} → driver.prepare(plan, evidence); driver.run
        //    → expect Committed; return Ok(plan.id).
        // 7. NeedsReview{reasons} → (M5 adds Overlay + reconciliation item);
        //    return Ok(plan.id).
        //
        // Save-promotion builder (Algorithm A): one WriteFile step; prestate =
        // observed base hash (or FileAbsent); poststate = merged-bytes hash;
        // inverse = Some(InverseRepairPlan(one WriteFile of the OLD bytes,
        // prestate = new hash, poststate = old hash)); subject = the FILE node.
        // The runner's inline save logic (runner::build_save_plan) is the M02
        // seed to fold into this path.
        todo!("Phase -1 M4: save-as-Promotion through IlrpDriver (Algorithm C; R4 §4)")
    }
}
