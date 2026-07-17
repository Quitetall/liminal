//! The core Reconciliation Queue (R4 §9) — present from Phase -1, long before
//! any agenda subsystem exists. When the agenda domain arrives (Phase 5) it
//! PROJECTS these items as tasks; it never copies them and is never their
//! storage authority.

use liminal_graph::{GraphStore, GraphTxn, StoreError};
use liminal_id::{JurisdictionSubject, OverlayId, ReconciliationItemId, RepairId, Timestamp};
use serde::{Deserialize, Serialize};

/// Aux namespace in the coordinating store.
pub const RECONCILE_NS: &str = "jur.reconcile";

/// One coalesced reconciliation entry. Coalescing key is `root_cause`:
/// visible incidents per root cause per session <= 1 (R4 §2.3) — repeated
/// downstream invalidations from one root event never inflate the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationItem {
    /// Identity.
    pub id: ReconciliationItemId,
    /// The coalescing key: one root event, one visible item.
    pub root_cause: String,
    /// Affected subjects.
    pub subjects: Vec<JurisdictionSubject>,
    /// Linked Overlays awaiting acceptance.
    pub overlays: Vec<OverlayId>,
    /// Linked repair proposals.
    pub repairs: Vec<RepairId>,
    /// Creation time.
    pub created_at: Timestamp,
    /// Status.
    pub status: ReconciliationStatus,
}

/// Reconciliation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReconciliationStatus {
    /// Awaiting resolution.
    Pending,
    /// Blocked on a prerequisite repair step (e.g. ID insertion before
    /// reattachment, v4 §7.7).
    Blocked {
        /// The blocking step.
        on: liminal_id::RepairStepId,
    },
    /// Resolved by accepted repair or explicit discard.
    Resolved,
    /// Explicitly archived; remains searchable (v4 §7.10.7).
    Archived,
}

/// Queue access over the coordinating store's `jur.reconcile` namespace.
/// Surfaced by `lim overlays` / `lim repairs` and contextually at
/// open/sync/publication/destructive-conversion boundaries (R4 §9).
#[derive(Debug)]
pub struct ReconciliationQueue<'s> {
    /// The coordinating store.
    pub store: &'s GraphStore,
}

impl ReconciliationQueue<'_> {
    /// All items, in key order.
    pub fn items(&self) -> Result<Vec<ReconciliationItem>, StoreError> {
        let mut out = Vec::new();
        for (key, value) in self.store.scan_aux(RECONCILE_NS)? {
            let item: ReconciliationItem = serde_json::from_value(value)
                .map_err(|e| StoreError::Corrupt(format!("reconcile item {key}: {e}")))?;
            out.push(item);
        }
        Ok(out)
    }

    /// Insert or update an item, coalescing on `root_cause`, inside a caller
    /// transaction (so queue changes commit atomically with the graph/intent
    /// changes that caused them).
    pub fn upsert_coalesced(
        &self,
        txn: &mut GraphTxn<'_>,
        item: ReconciliationItem,
    ) -> Result<(), StoreError> {
        let _ = (txn, item);
        todo!("Phase -1 M5: root-cause coalescing upsert (R4 §2.3, §9)")
    }
}
