//! The core Reconciliation Queue (R4 §9) — present from Phase -1, long before
//! any task-list subsystem exists. When that subsystem arrives (Phase 5) it
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
    ///
    /// Algorithm A: scan for the first `Pending | Blocked` item sharing
    /// `item.root_cause`; found → merge `subjects`/`overlays`/`repairs` (union,
    /// sorted, deduped) and adopt `item.status`, written under the EXISTING
    /// id (visible incidents per root cause <= 1, R4 §2.3); none → insert
    /// `item` as given.
    pub fn upsert_coalesced(
        &self,
        txn: &mut GraphTxn<'_>,
        item: ReconciliationItem,
    ) -> Result<(), StoreError> {
        let mut existing: Option<ReconciliationItem> = None;
        for (key, value) in self.store.scan_aux(RECONCILE_NS)? {
            let candidate: ReconciliationItem = serde_json::from_value(value)
                .map_err(|e| StoreError::Corrupt(format!("reconcile item {key}: {e}")))?;
            if candidate.root_cause == item.root_cause
                && matches!(
                    candidate.status,
                    ReconciliationStatus::Pending | ReconciliationStatus::Blocked { .. }
                )
            {
                existing = Some(candidate);
                break;
            }
        }

        let merged = match existing {
            Some(mut e) => {
                merge_sorted(&mut e.subjects, item.subjects);
                merge_sorted(&mut e.overlays, item.overlays);
                merge_sorted(&mut e.repairs, item.repairs);
                e.status = item.status;
                e
            }
            None => item,
        };
        txn.put_aux(
            RECONCILE_NS,
            &merged.id.to_string(),
            serde_json::to_value(&merged)
                .map_err(|e| StoreError::Corrupt(format!("reconcile item encode: {e}")))?,
        )
    }
}

/// Union `incoming` into `existing`, then sort and dedup (Algorithm A step 2).
fn merge_sorted<T: Ord>(existing: &mut Vec<T>, incoming: Vec<T>) {
    existing.extend(incoming);
    existing.sort();
    existing.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use liminal_id::NodeId;

    /// A fresh scratch store that removes its directory on drop (M17.5 F-12).
    /// The guard is returned alongside the store because dropping it deletes the
    /// directory the store is reading.
    fn scratch_store(label: &str) -> (GraphStore, liminal_scratch::ScratchDir) {
        let dir =
            liminal_scratch::ScratchDir::new(&format!("reconcile-{label}")).expect("scratch dir");
        let store = GraphStore::open(&dir).expect("open store");
        (store, dir)
    }

    fn item(root_cause: &str, subject: JurisdictionSubject) -> ReconciliationItem {
        ReconciliationItem {
            id: ReconciliationItemId::new(),
            root_cause: root_cause.to_owned(),
            subjects: vec![subject],
            overlays: vec![OverlayId::new()],
            repairs: vec![RepairId::new()],
            created_at: Timestamp::now(),
            status: ReconciliationStatus::Pending,
        }
    }

    /// Algorithm A: a second upsert sharing `root_cause` coalesces into the
    /// FIRST item's id, unioning subjects/overlays/repairs rather than
    /// inflating the queue with a second visible incident (R4 §2.3).
    #[test]
    fn same_root_cause_coalesces_under_the_first_id() {
        let (store, _scratch) = scratch_store("coalesce");
        let queue = ReconciliationQueue { store: &store };

        let s1 = JurisdictionSubject::Node(NodeId::new());
        let s2 = JurisdictionSubject::Node(NodeId::new());
        let first = item("conflict:path:notes.md", s1);
        let first_id = first.id;
        let second = item("conflict:path:notes.md", s2);

        let meta = liminal_graph::TxnMeta {
            actor: None,
            origin: liminal_graph::Origin::Human,
            at: Timestamp::now(),
            provenance: None,
            inverse: None,
        };

        let mut txn = store.begin().unwrap();
        queue.upsert_coalesced(&mut txn, first).unwrap();
        txn.commit(meta.clone()).unwrap();

        let mut txn = store.begin().unwrap();
        queue.upsert_coalesced(&mut txn, second).unwrap();
        txn.commit(meta).unwrap();

        let items = queue.items().unwrap();
        assert_eq!(items.len(), 1, "coalesced under one visible item");
        let merged = &items[0];
        assert_eq!(merged.id, first_id, "kept the EXISTING id");
        let mut subjects = merged.subjects.clone();
        subjects.sort();
        let mut expected = vec![s1, s2];
        expected.sort();
        assert_eq!(subjects, expected, "subjects unioned");
        assert_eq!(merged.overlays.len(), 2, "overlays unioned");
        assert_eq!(merged.repairs.len(), 2, "repairs unioned");
    }

    /// A different `root_cause` never coalesces — two distinct root events
    /// remain two distinct visible items.
    #[test]
    fn different_root_cause_stays_separate() {
        let (store, _scratch) = scratch_store("separate");
        let queue = ReconciliationQueue { store: &store };

        let meta = liminal_graph::TxnMeta {
            actor: None,
            origin: liminal_graph::Origin::Human,
            at: Timestamp::now(),
            provenance: None,
            inverse: None,
        };

        let mut txn = store.begin().unwrap();
        queue
            .upsert_coalesced(
                &mut txn,
                item(
                    "conflict:path:a.md",
                    JurisdictionSubject::Node(NodeId::new()),
                ),
            )
            .unwrap();
        txn.commit(meta.clone()).unwrap();

        let mut txn = store.begin().unwrap();
        queue
            .upsert_coalesced(
                &mut txn,
                item(
                    "conflict:path:b.md",
                    JurisdictionSubject::Node(NodeId::new()),
                ),
            )
            .unwrap();
        txn.commit(meta).unwrap();

        assert_eq!(queue.items().unwrap().len(), 2);
    }
}
