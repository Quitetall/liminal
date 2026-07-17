//! Overlays: the durable internal form of an unaccepted draft (v4 §7.10;
//! Law 3B: capture is never rejected; Law 3I: Overlay debt remains visible).

use liminal_id::{JurisdictionSubject, OverlayId, RepairId, Timestamp};
use liminal_revision::WorkspaceBasis;
use serde::{Deserialize, Serialize};

use crate::contract::LifecyclePolicy;
use crate::profile::ProfileId;
use crate::repair::RepairOperation;

/// A durable unaccepted draft (v4 §7.10). Durable IMMEDIATELY — an Overlay
/// exists before any reconciliation is attempted, and survives process death.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    /// Identity.
    pub id: OverlayId,
    /// The governed subject this draft targets.
    pub subject: JurisdictionSubject,
    /// The Basis the draft was made against.
    pub base: WorkspaceBasis,
    /// The captured edit or operation.
    pub operation: RepairOperation,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last local activity (edits coalesce without erasing history, §7.10.2).
    pub last_activity: Timestamp,
    /// The governing profile.
    pub profile: ProfileId,
    /// Aging thresholds (§7.10.3).
    pub lifecycle: LifecyclePolicy,
    /// Lifecycle state.
    pub state: OverlayState,
}

/// Overlay lifecycle state. An Overlay leaves the active queue ONLY through
/// accepted repair, explicit discard, or an explicit archival action that
/// remains searchable — never silent collection (v4 §7.10.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayState {
    /// Awaiting a Holder or a safe automatic repair.
    Active,
    /// A repair plan has been proposed and awaits acceptance.
    RepairProposed(RepairId),
    /// ILRP recovery observed contested state; human review required
    /// (v4 §7.8 step 5 — never guess).
    Contested,
    /// Explicitly archived; remains searchable.
    Archived,
    /// Explicitly discarded by the user.
    Discarded,
}
