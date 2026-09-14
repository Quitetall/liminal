//! Store ownership and narrowly scoped internal writers (AM-17.12).
//!
//! The owner is assembled at workspace open, never deserialized from a DTO.
//! This first migration slice covers epoch and volatile buffer capture. The
//! legacy GraphStore transaction interface remains until the remaining writer
//! callers migrate; these types alone do not establish closed write authority.

use camino::Utf8Path;
use liminal_id::{
    BufferId, ClientId, ContentHash, GraphRevisionId, SessionEpoch, SourceId, TransactionId,
};
use std::sync::Arc;

use crate::{GraphStore, StoreError, TxnMeta, ns};

/// Root capability held by trusted workspace assembly, not by query callers.
/// Not clonable or serializable. Its borrowed capabilities cannot outlive it.
#[derive(Debug)]
pub struct StoreOwner {
    store: Arc<GraphStore>,
}

impl StoreOwner {
    /// Open or create the owned store using the existing recovery/locking path.
    pub fn open(dir: &Utf8Path) -> Result<Self, StoreError> {
        Ok(Self {
            store: Arc::new(GraphStore::open(dir)?),
        })
    }

    /// Borrow the store for reads. Legacy raw-write callers are being migrated
    /// separately; this accessor does not yet claim a read-only type boundary.
    #[must_use]
    pub fn store(&self) -> &GraphStore {
        &self.store
    }

    /// Lend only the right to persist the workspace epoch counter.
    #[must_use]
    pub fn epoch_writer(&self) -> EpochWriter<'_> {
        EpochWriter { store: &self.store }
    }

    /// Lend only volatile, buffer-keyed working capture.
    #[must_use]
    pub fn working_capture(&self) -> WorkingCapture<'_> {
        WorkingCapture { store: &self.store }
    }

    /// Explicitly grant the conformance adapter authority to corrupt only
    /// volatile buffer and observation blobs. Never provided by a read view or
    /// ordinary workspace request. The grant keeps this same store open until
    /// dropped; fixtures must drop it before testing reopen semantics.
    #[must_use]
    pub fn blob_fault_injector(&self) -> BlobFaultInjector {
        BlobFaultInjector {
            store: Arc::clone(&self.store),
        }
    }
}

/// Owner-granted corruption adapter for existing hash-pin rejection witnesses.
/// This is not normal capture, not durable, and cannot write ILRP or graph state.
/// Construction occurs explicitly at trusted fixture assembly before the owner
/// is handed to the workspace. No serialization or public fields mint a grant.
#[derive(Debug)]
pub struct BlobFaultInjector {
    store: Arc<GraphStore>,
}

impl BlobFaultInjector {
    /// Replace exactly one buffer-generation blob without changing its Basis.
    pub fn corrupt_buffer(
        &self,
        client: ClientId,
        buffer: BufferId,
        generation: u64,
        text: &str,
    ) -> Result<(), StoreError> {
        WorkingCapture { store: &self.store }.put_buffer(client, buffer, generation, text)
    }

    /// Replace a pinned observation blob without changing its key/hash metadata.
    pub fn corrupt_observation(
        &self,
        source: SourceId,
        hash: ContentHash,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        self.store
            .put_working_aux(ns::SYS_BLOB, &format!("obs/{source}/{hash}"), value)
    }
}

/// Capability for exactly `SYS_EPOCH["epoch"]`; no graph/ILRP/other aux access.
///
/// Ordinary callers cannot construct a writer from a store reference:
/// ```compile_fail
/// use liminal_graph::{EpochWriter, GraphStore};
/// fn forge(store: &GraphStore) { let _ = EpochWriter { store }; }
/// ```
#[derive(Debug)]
pub struct EpochWriter<'s> {
    store: &'s GraphStore,
}

impl EpochWriter<'_> {
    /// Persist one epoch through the existing durable transaction boundary.
    pub fn persist(
        &self,
        epoch: SessionEpoch,
        meta: TxnMeta,
    ) -> Result<(GraphRevisionId, TransactionId), StoreError> {
        let mut txn = self.store.begin()?;
        txn.put_aux(ns::SYS_EPOCH, "epoch", serde_json::Value::from(epoch.0))?;
        txn.commit(meta)
    }
}

/// Volatile capture of one identified buffer generation. No caller-supplied
/// namespace or arbitrary key; cannot overwrite an intent or durable file mirror.
///
/// ```compile_fail
/// use liminal_graph::{GraphStore, WorkingCapture};
/// fn forge(store: &GraphStore) { let _ = WorkingCapture { store }; }
/// ```
#[derive(Debug)]
pub struct WorkingCapture<'s> {
    store: &'s GraphStore,
}

impl WorkingCapture<'_> {
    /// Capture UTF-8 text without advancing the graph revision or claiming
    /// durability. Lost on reopen, as required by the existing buffer contract.
    pub fn put_buffer(
        &self,
        client: ClientId,
        buffer: BufferId,
        generation: u64,
        text: &str,
    ) -> Result<(), StoreError> {
        self.store.put_working_aux(
            ns::SYS_BLOB,
            &format!("buf/{client}/{buffer}/{generation}"),
            serde_json::Value::String(text.to_owned()),
        )
    }
}
