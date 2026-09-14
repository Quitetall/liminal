//! Store ownership and narrowly scoped internal writers (AM-17.12).
//!
//! The owner is assembled at workspace open, never deserialized from a DTO.
//! The owner lends read views and purpose-scoped writers (v4 Law 3). Raw root
//! transaction authority remains confined to trusted workspace assembly; the
//! scoped types make write routes explicit without claiming host isolation.

use camino::Utf8Path;
use liminal_id::{
    BufferId, ClientId, ContentHash, GraphRevisionId, SessionEpoch, SourceId, TransactionId,
};
use std::{ops::Deref, sync::Arc};

use crate::{GraphStore, StoreError, TxnMeta, ns, store::TxnAuthority};

/// Root capability held by trusted workspace assembly, not by query callers (v4 Law 3; v4 §7.3).
/// Not cloneable or serializable. Its borrowed capabilities cannot outlive it.
#[derive(Debug)]
pub struct StoreOwner {
    store: Arc<GraphStore>,
}

impl StoreOwner {
    /// Open or create the owned store using the existing recovery/locking path (v4 §92).
    pub fn open(dir: &Utf8Path) -> Result<Self, StoreError> {
        Ok(Self {
            store: Arc::new(GraphStore::open(dir)?),
        })
    }

    /// Borrow the store for reads without write authority (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn store(&self) -> &GraphStore {
        &self.store
    }

    /// Start an unrestricted transaction at the trusted root boundary (v4 §92).
    /// Ordinary daemon code uses a named scoped writer instead.
    pub fn begin(&self) -> Result<crate::GraphTxn<'_>, StoreError> {
        self.store.begin()
    }

    /// Persist a snapshot at the trusted root boundary (v4 §92).
    pub fn snapshot(&self) -> Result<(), StoreError> {
        self.store.snapshot()
    }

    /// Check path and actual held-lock identity before workspace assembly.
    /// This is a point-in-time host check, not a lease against later path edits.
    pub fn matches_directory(&self, dir: &Utf8Path) -> Result<bool, StoreError> {
        self.store.matches_directory(dir)
    }

    /// Lend only workspace-epoch persistence (v4 §7.5).
    #[must_use]
    pub fn epoch_writer(&self) -> EpochWriter<'_> {
        EpochWriter { store: &self.store }
    }

    /// Lend only volatile, buffer-keyed working capture (v4 §7.5).
    #[must_use]
    pub fn working_capture(&self) -> WorkingCapture<'_> {
        WorkingCapture { store: &self.store }
    }

    /// Lend graph-plus-intent authority only to the ILRP coordinator (v4 §7.8).
    #[must_use]
    pub fn coordinator_writer(&self) -> CoordinatorWriter<'_> {
        CoordinatorWriter { store: &self.store }
    }

    /// Lend setup-ingest authority under the declared write route (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn bootstrap_writer(&self) -> BootstrapWriter<'_> {
        BootstrapWriter { store: &self.store }
    }

    /// Lend proposal/capture authority under the declared write route (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn capture_writer(&self) -> CaptureWriter<'_> {
        CaptureWriter { store: &self.store }
    }

    /// Lend committed-result bookkeeping authority (v4 Law 3G; v4 §7.7).
    #[must_use]
    pub fn bookkeeping_writer(&self) -> BookkeepingWriter<'_> {
        BookkeepingWriter { store: &self.store }
    }

    /// Lend external-input host-control authority (v4 §8.6).
    #[must_use]
    pub fn host_control_writer(&self) -> HostControlWriter<'_> {
        HostControlWriter { store: &self.store }
    }

    /// Lend reactor authority bound to one external source (v4 §8.6).
    #[must_use]
    pub fn reactor_writer(&self, source: SourceId) -> ReactorWriter<'_> {
        ReactorWriter {
            store: &self.store,
            source,
        }
    }

    /// Lend only reconciliation-debt writes; no graph or accepted ILRP effects (v4 Law 3I; v4 §7.10).
    #[must_use]
    pub fn reconciliation_writer(&self) -> ReconciliationWriter<'_> {
        ReconciliationWriter { store: &self.store }
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

impl Deref for StoreOwner {
    type Target = GraphStore;

    fn deref(&self) -> &Self::Target {
        self.store()
    }
}

macro_rules! aux_writer {
    ($name:ident, $authority:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug)]
        pub struct $name<'s> {
            store: &'s GraphStore,
        }

        impl<'s> $name<'s> {
            /// Borrow the same store for reads (v4 Law 3; v4 §7.3).
            #[must_use]
            pub fn store(&self) -> &'s GraphStore {
                self.store
            }

            /// Start a transaction permanently bound to this writer's scope (v4 §7.3; v4 §92).
            pub fn begin(&self) -> Result<crate::GraphTxn<'s>, StoreError> {
                self.store.begin_scoped($authority)
            }
        }
    };
}

aux_writer!(
    CaptureWriter,
    TxnAuthority::Capture,
    "Owner-issued writer for proposal, decision, overlay, debt, and content-addressed capture (v4 §7.10)."
);
aux_writer!(
    BookkeepingWriter,
    TxnAuthority::Bookkeeping,
    "Owner-issued writer for committed repair bookkeeping and durable mirrors (v4 §7.7)."
);
aux_writer!(
    HostControlWriter,
    TxnAuthority::HostControl,
    "Owner-issued writer for the fake clock and unavailable-holder state (v4 §8.6)."
);

/// Owner-issued ILRP coordinator writer (v4 §7.8). Graph operations and `ILRP_INTENT`
/// may share one durable transaction; no other auxiliary namespace is allowed.
#[derive(Debug)]
pub struct CoordinatorWriter<'s> {
    store: &'s GraphStore,
}

impl<'s> CoordinatorWriter<'s> {
    /// Borrow the same store for reads with the owner's lifetime (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn store(&self) -> &'s GraphStore {
        self.store
    }

    /// Start a coordinator-scoped transaction (v4 §7.8; v4 §92).
    pub fn begin(&self) -> Result<crate::GraphTxn<'s>, StoreError> {
        self.store.begin_scoped(TxnAuthority::Coordinator)
    }
}

/// Owner-issued setup-ingest writer (v4 Law 3; v4 §7.3).
#[derive(Debug)]
pub struct BootstrapWriter<'s> {
    store: &'s GraphStore,
}

impl<'s> BootstrapWriter<'s> {
    /// Borrow the same store for reads (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn store(&self) -> &'s GraphStore {
        self.store
    }

    /// Start a setup-scoped transaction (v4 §7.3; v4 §92).
    pub fn begin(&self) -> Result<crate::GraphTxn<'s>, StoreError> {
        self.store.begin_scoped(TxnAuthority::Bootstrap)
    }
}

/// Owner-issued reactor writer bound to one external source (v4 §8.6).
#[derive(Debug)]
pub struct ReactorWriter<'s> {
    store: &'s GraphStore,
    source: SourceId,
}

impl<'s> ReactorWriter<'s> {
    /// Borrow the same store for reads (v4 Law 3; v4 §7.3).
    #[must_use]
    pub fn store(&self) -> &'s GraphStore {
        self.store
    }

    /// The only source this writer may materialize (v4 §8.6).
    #[must_use]
    pub fn source(&self) -> SourceId {
        self.source
    }

    /// Start a transaction bound to this source (v4 §8.6; v4 §92).
    pub fn begin(&self) -> Result<crate::GraphTxn<'s>, StoreError> {
        self.store.begin_scoped(TxnAuthority::Reactor(self.source))
    }
}

/// Owner-issued writer for reconciliation debt only (v4 Law 3I; v4 §7.10). Failed out-of-scope
/// operations poison the entire transaction, even when a caller ignores errors.
///
/// ```compile_fail
/// use liminal_graph::{GraphStore, ReconciliationWriter};
/// fn forge(store: &GraphStore) { let _ = ReconciliationWriter { store }; }
/// ```
#[derive(Debug)]
pub struct ReconciliationWriter<'s> {
    store: &'s GraphStore,
}

impl ReconciliationWriter<'_> {
    /// Start a `JUR_RECONCILE`-only transaction (v4 Law 3I).
    pub fn begin(&self) -> Result<crate::GraphTxn<'_>, StoreError> {
        self.store.begin_scoped(TxnAuthority::Reconciliation)
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
        self.store.inject_aux_fault(
            ns::SYS_BLOB,
            &format!("buf/{client}/{buffer}/{generation}"),
            serde_json::Value::String(text.to_owned()),
        )
    }

    /// Replace a pinned observation blob without changing its key/hash metadata.
    pub fn corrupt_observation(
        &self,
        source: SourceId,
        hash: ContentHash,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        self.store
            .inject_aux_fault(ns::SYS_BLOB, &format!("obs/{source}/{hash}"), value)
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
        let mut txn = self.store.begin_scoped(TxnAuthority::Epoch)?;
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
