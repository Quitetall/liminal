//! The Phase -1 toy durable store (ADR-0007) — hand-rolled to spec v4 §92:
//! append-only checksummed segments plus atomic snapshot replacement.
//!
//! Explicitly throwaway. Its two jobs:
//!
//! 1. Hold toy graph state durably with honest, *visible* fsync boundaries so
//!    the ILRP crash matrix tests Liminal's own durability discipline, not a
//!    vendor WAL.
//! 2. Act as the ILRP coordinator: one committed log record carries graph
//!    operations AND auxiliary writes (intent state, overlays, reconciliation
//!    items), which is what makes ILRP Prepare/Finalize single transactions
//!    (v4 §7.8, §92: "the graph store acts as coordinator because it can
//!    transactionally retain intent and progress").
//!
//! Everything on disk is NDJSON — `less`-able, `grep`-able, `git diff`-able.
//! A failed crash test can be debugged by reading the log with eyes.
//!
//! Deviations from the abstract sketch, documented: aux keys/values are
//! `String`/[`serde_json::Value`] rather than bytes, keeping records
//! human-inspectable (falsification > speed).

mod asof;
mod log;
pub mod ns;
mod snapshot;

pub use asof::StateView;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Mutex;

use camino::{Utf8Path, Utf8PathBuf};
use fs4::fs_std::FileExt as _;
use liminal_id::{GraphRevisionId, NodeId, RelationId, SourceId, TransactionId};
use serde::{Deserialize, Serialize};

use crate::node::{Node, PayloadRef};
use crate::op::{Operation, Transaction, TxnMeta};
use crate::relation::Relation;

pub(crate) use log::SegmentLog;

/// Non-serializable authority attached to a transaction at private
/// construction. A scoped transaction can never widen or remove this value.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TxnAuthority {
    Root,
    Coordinator,
    Bootstrap,
    Capture,
    Bookkeeping,
    HostControl,
    Reactor(SourceId),
    Reconciliation,
    Epoch,
}

/// One auxiliary durable write, committed atomically with graph operations.
/// `value: None` deletes the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuxWrite {
    /// Namespace (e.g. `ilrp.intent`, `jur.overlay`, `jur.reconcile`).
    pub ns: String,
    /// Key within the namespace.
    pub key: String,
    /// New value, or `None` to delete.
    pub value: Option<serde_json::Value>,
}

/// One accepted historical write to an auxiliary key. Read-only provenance;
/// possessing this DTO grants no write authority.
#[derive(Debug, Clone, PartialEq)]
pub struct CommittedAuxRecord {
    /// Graph revision containing the write.
    pub revision: GraphRevisionId,
    /// Accepted transaction recorded at that revision.
    pub transaction: Transaction,
    /// Written value, or `None` for a deletion.
    pub value: Option<serde_json::Value>,
}

/// One committed log record: a transaction's operations plus its auxiliary
/// writes, applied atomically. A record is either fully present with a valid
/// checksum or dropped at recovery — that line-atomicity is the §92 guarantee
/// every crash test rests on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommitRecord {
    pub revision: GraphRevisionId,
    pub txn: Transaction,
    pub aux: Vec<AuxWrite>,
}

/// Full materialized state — snapshot payload and in-memory shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct State {
    pub head: GraphRevisionId,
    pub nodes: BTreeMap<NodeId, Node>,
    pub relations: BTreeMap<RelationId, Relation>,
    /// Ordered containment (v4 §5.2) in its simplest toy form.
    pub children: BTreeMap<NodeId, Vec<NodeId>>,
    pub transactions: BTreeMap<TransactionId, Transaction>,
    pub aux: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

impl State {
    fn apply(&mut self, op: &Operation) -> Result<(), StoreError> {
        match op {
            Operation::CreateNode { node } => {
                if self.nodes.contains_key(&node.id) {
                    return Err(StoreError::Conflict(format!("node exists: {}", node.id)));
                }
                self.nodes.insert(node.id, node.clone());
            }
            Operation::DeleteNode { id } => {
                self.nodes
                    .remove(id)
                    .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
                self.children.remove(id);
            }
            Operation::SetPayload { id, payload } => {
                let node = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
                node.payload = payload.clone();
                node.revision.0 += 1;
            }
            Operation::AddRelation { relation } => {
                if self.relations.contains_key(&relation.id) {
                    return Err(StoreError::Conflict(format!(
                        "relation exists: {}",
                        relation.id
                    )));
                }
                self.relations.insert(relation.id, relation.clone());
            }
            Operation::RemoveRelation { id } => {
                self.relations
                    .remove(id)
                    .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
            }
            Operation::RetargetRelation { id, target } => {
                let rel = self
                    .relations
                    .get_mut(id)
                    .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
                rel.target = target.clone();
                rel.revision.0 += 1;
            }
            Operation::InsertChild {
                parent,
                child,
                index,
            } => {
                let children = self.children.entry(*parent).or_default();
                let at = usize::try_from(*index)
                    .unwrap_or(usize::MAX)
                    .min(children.len());
                children.insert(at, *child);
            }
            Operation::MoveChild {
                parent,
                child,
                index,
            } => {
                let children = self
                    .children
                    .get_mut(parent)
                    .ok_or_else(|| StoreError::NotFound(parent.to_string()))?;
                let pos = children
                    .iter()
                    .position(|c| c == child)
                    .ok_or_else(|| StoreError::NotFound(child.to_string()))?;
                children.remove(pos);
                let at = usize::try_from(*index)
                    .unwrap_or(usize::MAX)
                    .min(children.len());
                children.insert(at, *child);
            }
            Operation::AttachResource { node, object } => {
                let n = self
                    .nodes
                    .get_mut(node)
                    .ok_or_else(|| StoreError::NotFound(node.to_string()))?;
                n.payload = PayloadRef::Object(*object);
                n.revision.0 += 1;
            }
            Operation::MaterializeExternal { node, observed, .. } => {
                let n = self
                    .nodes
                    .get_mut(node)
                    .ok_or_else(|| StoreError::NotFound(node.to_string()))?;
                n.payload = PayloadRef::Object(*observed);
                n.revision.0 += 1;
            }
        }
        Ok(())
    }

    fn apply_commit(&mut self, record: &CommitRecord) -> Result<(), StoreError> {
        if record.revision.0 != self.head.0 + 1 {
            return Err(StoreError::Corrupt(format!(
                "revision gap: head {:?}, record {:?}",
                self.head, record.revision
            )));
        }
        for op in &record.txn.ops {
            self.apply(op)?;
        }
        for aux in &record.aux {
            let ns = self.aux.entry(aux.ns.clone()).or_default();
            match &aux.value {
                Some(v) => {
                    ns.insert(aux.key.clone(), v.clone());
                }
                None => {
                    ns.remove(&aux.key);
                }
            }
        }
        self.head = record.revision;
        self.transactions.insert(record.txn.id, record.txn.clone());
        Ok(())
    }
}

struct Inner {
    state: State,
    log: SegmentLog,
    /// Held for the store's lifetime; advisory, so it survives SIGABRT of the
    /// holder (an O_EXCL lockfile would deadlock crash recovery).
    lock_handle: same_file::Handle,
}

/// The toy transactional graph store and ILRP coordinator (v4 §92; ADR-0007).
///
/// Single-writer (advisory file lock). All reads are head-revision reads;
/// as-of/history queries are the Phase -1.3 (M8) milestone.
/// Ordinary store references cannot open transactions:
/// ```compile_fail
/// use liminal_graph::GraphStore;
/// fn raw_write(store: &GraphStore) { let _ = store.begin(); }
/// ```
#[derive(Debug)]
pub struct GraphStore {
    dir: Utf8PathBuf,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("head", &self.state.head)
            .finish_non_exhaustive()
    }
}

/// How long [`acquire_write_lock`] tolerates a transient lock holder. Sized to
/// dwarf a fork→exec window (microseconds, worst case low milliseconds under
/// load) while staying far below any human-visible open latency. A genuine
/// second writer holds its lock for its whole lifetime, so this delays the
/// honest `Locked` error by at most this budget — it never suppresses it.
const LOCK_ACQUIRE_BUDGET: std::time::Duration = std::time::Duration::from_millis(250);

/// Take the single-writer advisory lock, tolerating a *transient* holder
/// (M17.5 F-11).
///
/// `flock(2)` ownership belongs to the open file description, not to the
/// descriptor. So when this process spawns a subprocess, the child inherits a
/// duplicate of every open descriptor at `fork`, and `O_CLOEXEC` does not
/// discard them until `execve`. Inside that window the child co-owns our lock's
/// OFD — and therefore closing *our* descriptor does not release the lock.
///
/// That makes a plain `try_lock` unsound for any process that both spawns
/// children and reopens a store, which is exactly what the conformance harness
/// does: `StepRunner::open` deliberately drops the workspace and reopens the
/// same path (M08.7's `seed_durable_inputs` ordering), while sibling tests
/// spawn `lim-toy` and `git`. The reopen then failed with `EWOULDBLOCK` against
/// a lock no live writer held.
///
/// Measured in isolation with a control: a drop-then-reopen loop fails 0/3000
/// times with no subprocess spawning and 4/3000 with a sibling thread spawning
/// children. This is not a flaky test retried away — it is a lock acquisition
/// that was never correct in a process that forks, and the permanent-holder
/// canary in `tests/store.rs` pins that real contention still fails.
/// Only contention is retried. `try_lock_exclusive` reports contention as
/// `Ok(false)`; any `Err` is a real I/O fault (a closed descriptor, a
/// filesystem that cannot lock) and propagates immediately rather than being
/// retried into a timeout that would misreport the cause as `Locked`.
fn acquire_write_lock(lock: &fs::File, dir: &Utf8Path) -> Result<(), StoreError> {
    let deadline = std::time::Instant::now() + LOCK_ACQUIRE_BUDGET;
    loop {
        if lock.try_lock_exclusive()? {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(StoreError::Locked(dir.to_owned()));
        }
        // Sleeping rather than spinning is deliberate, not an unfinished
        // optimization: opening a store is a cold path, and the holder we are
        // waiting on is a subprocess reaching `execve`, which no amount of
        // spinning makes arrive sooner.
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

impl GraphStore {
    /// Open (or create) a store at `dir`, recovering state per §92: load the
    /// snapshot if present, replay checksummed segments, truncate a torn tail.
    pub(crate) fn open(dir: &Utf8Path) -> Result<Self, StoreError> {
        fs::create_dir_all(dir)?;
        let lock_path = dir.join("lock");
        let lock = fs::File::create(&lock_path)?;
        acquire_write_lock(&lock, dir)?;

        let (mut state, base_segment) = snapshot::load(dir)?;
        let log = SegmentLog::recover(dir, base_segment, |record| {
            state.apply_commit(record).map_err(|e| e.to_string())
        })?;

        Ok(Self {
            dir: dir.to_owned(),
            inner: Mutex::new(Inner {
                state,
                log,
                lock_handle: same_file::Handle::from_file(lock)?,
            }),
        })
    }

    /// Current head revision.
    pub fn head(&self) -> Result<GraphRevisionId, StoreError> {
        Ok(self.lock()?.state.head)
    }

    /// Transaction that produced the current head, or `None` at genesis.
    pub fn head_transaction(&self) -> Result<Option<Transaction>, StoreError> {
        let inner = self.lock()?;
        let head = inner.state.head;
        Ok(inner
            .state
            .transactions
            .values()
            .find(|transaction| transaction.parent.0.checked_add(1) == Some(head.0))
            .cloned())
    }

    /// Validate graph operations against a clone of current state without
    /// mutating memory, appending a record, or advancing the head.
    pub fn preview_ops(&self, ops: &[Operation]) -> Result<StateView, StoreError> {
        let mut state = self.lock()?.state.clone();
        for op in ops {
            state.apply(op)?;
        }
        Ok(StateView::from_state(state))
    }

    /// Replay retained checksummed history and return every accepted write to
    /// `namespace[key]`, including overwrites and deletions, in append order.
    pub fn committed_aux_history(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Vec<CommittedAuxRecord>, StoreError> {
        let inner = self.lock()?;
        let accepted_head = inner.state.head;
        let mut replayed = State::default();
        let mut history = Vec::new();
        log::replay_from_genesis(self.dir(), |record| {
            if record.revision.0 > accepted_head.0 {
                return Err(format!(
                    "history revision {:?} exceeds accepted head {accepted_head:?}",
                    record.revision
                ));
            }
            replayed
                .apply_commit(record)
                .map_err(|error| error.to_string())?;
            for write in &record.aux {
                if write.ns == namespace && write.key == key {
                    history.push(CommittedAuxRecord {
                        revision: record.revision,
                        transaction: record.txn.clone(),
                        value: write.value.clone(),
                    });
                }
            }
            Ok(())
        })?;
        let requested_replayed = replayed
            .aux
            .get(namespace)
            .and_then(|values| values.get(key));
        let requested_accepted = inner
            .state
            .aux
            .get(namespace)
            .and_then(|values| values.get(key));
        if replayed.head != inner.state.head
            || replayed.nodes != inner.state.nodes
            || replayed.relations != inner.state.relations
            || replayed.children != inner.state.children
            || replayed.transactions != inner.state.transactions
            || requested_replayed != requested_accepted
        {
            return Err(StoreError::Corrupt(
                "retained history does not reconstruct requested accepted state".into(),
            ));
        }
        Ok(history)
    }

    /// Begin a transaction. Nothing is durable until [`GraphTxn::commit`].
    pub(crate) fn begin(&self) -> Result<GraphTxn<'_>, StoreError> {
        self.begin_scoped(TxnAuthority::Root)
    }

    pub(crate) fn begin_scoped(&self, authority: TxnAuthority) -> Result<GraphTxn<'_>, StoreError> {
        Ok(GraphTxn {
            store: self,
            ops: Vec::new(),
            aux: Vec::new(),
            authority,
            expected_head: if matches!(authority, TxnAuthority::Root) {
                None
            } else {
                Some(self.head()?)
            },
            denied: false,
        })
    }

    /// Read a Node at a revision. Head reads take the in-memory fast path;
    /// `rev < head` delegates to [`Self::state_at`] (M08.1); `rev > head` errors.
    pub fn node_at(&self, rev: GraphRevisionId, id: NodeId) -> Result<Option<Node>, StoreError> {
        {
            let inner = self.lock()?;
            if rev == inner.state.head {
                return Ok(inner.state.nodes.get(&id).cloned());
            }
        }
        Ok(self.state_at(rev)?.node(id).cloned())
    }

    /// Read a Relation at a revision (head fast path, else as-of; see
    /// [`Self::node_at`]).
    pub fn relation_at(
        &self,
        rev: GraphRevisionId,
        id: RelationId,
    ) -> Result<Option<Relation>, StoreError> {
        {
            let inner = self.lock()?;
            if rev == inner.state.head {
                return Ok(inner.state.relations.get(&id).cloned());
            }
        }
        Ok(self.state_at(rev)?.relation(id).cloned())
    }

    /// All Relations whose source is `source` (head fast path, else as-of; see
    /// [`Self::node_at`]).
    pub fn relations_from(
        &self,
        rev: GraphRevisionId,
        source: NodeId,
    ) -> Result<Vec<Relation>, StoreError> {
        {
            let inner = self.lock()?;
            if rev == inner.state.head {
                return Ok(inner
                    .state
                    .relations
                    .values()
                    .filter(|r| r.source == source)
                    .cloned()
                    .collect());
            }
        }
        Ok(self
            .state_at(rev)?
            .relations_from(source)
            .into_iter()
            .cloned()
            .collect())
    }

    /// All Relations at head, in id order. Toy-scale full scan.
    pub fn relations(&self) -> Result<Vec<Relation>, StoreError> {
        Ok(self.lock()?.state.relations.values().cloned().collect())
    }

    /// All Nodes at head, in id order. Toy-scale full scan.
    pub fn nodes(&self) -> Result<Vec<Node>, StoreError> {
        Ok(self.lock()?.state.nodes.values().cloned().collect())
    }

    /// Look up an accepted transaction.
    pub fn transaction(&self, id: TransactionId) -> Result<Option<Transaction>, StoreError> {
        Ok(self.lock()?.state.transactions.get(&id).cloned())
    }

    /// Write an EPHEMERAL auxiliary value — editor working state (buffer blobs)
    /// — into the in-memory aux map WITHOUT appending a log record or advancing
    /// the head revision (M08). An editor keystroke is not a durable graph
    /// transaction: it must not bump the graph revision that `GraphSnapshot`
    /// pins, or a buffer edit would spuriously invalidate every graph-reading
    /// query (component-granular invalidation, v4 §7.5). Lost on reopen —
    /// buffers are re-sent by their editor.
    pub(crate) fn put_working_aux(
        &self,
        ns: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        let mut inner = self.lock()?;
        if let Some(previous) = inner.state.aux.get(ns).and_then(|values| values.get(key)) {
            if previous != &value {
                return Err(StoreError::Conflict(
                    "captured buffer generation is immutable".into(),
                ));
            }
            return Ok(());
        }
        inner
            .state
            .aux
            .entry(ns.to_owned())
            .or_default()
            .insert(key.to_owned(), value);
        Ok(())
    }

    /// Explicit test fault injection bypasses working-generation immutability.
    /// Only the owner-issued fault grant can reach this path.
    pub(crate) fn inject_aux_fault(
        &self,
        ns: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        self.lock()?
            .state
            .aux
            .entry(ns.to_owned())
            .or_default()
            .insert(key.to_owned(), value);
        Ok(())
    }

    /// Read an auxiliary record committed atomically with graph state
    /// (ILRP intents, overlays, reconciliation items).
    pub fn get_aux(&self, ns: &str, key: &str) -> Result<Option<serde_json::Value>, StoreError> {
        Ok(self
            .lock()?
            .state
            .aux
            .get(ns)
            .and_then(|m| m.get(key))
            .cloned())
    }

    /// All auxiliary records in a namespace, in key order.
    pub fn scan_aux(&self, ns: &str) -> Result<Vec<(String, serde_json::Value)>, StoreError> {
        Ok(self
            .lock()?
            .state
            .aux
            .get(ns)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default())
    }

    /// Write a snapshot with atomic replacement and rotate to a fresh segment
    /// (v4 §92). Old segments are retained — the toy never garbage-collects.
    pub(crate) fn snapshot(&self) -> Result<(), StoreError> {
        let mut inner = self.lock()?;
        let next_segment = inner.log.current_segment() + 1;
        snapshot::write(&self.dir, &inner.state, next_segment)?;
        inner.log = SegmentLog::create(&self.dir, next_segment)?;
        Ok(())
    }

    /// The store directory.
    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Corrupt("store mutex poisoned".to_owned()))
    }

    fn commit_txn(
        &self,
        ops: Vec<Operation>,
        aux: Vec<AuxWrite>,
        meta: TxnMeta,
        expected_head: Option<GraphRevisionId>,
    ) -> Result<(GraphRevisionId, TransactionId), StoreError> {
        let mut inner = self.lock()?;
        if let Some(expected) = expected_head
            && inner.state.head != expected
        {
            return Err(StoreError::Conflict(format!(
                "transaction expected head {expected:?}, found {:?}",
                inner.state.head
            )));
        }
        let revision = GraphRevisionId(inner.state.head.0 + 1);
        let txn = Transaction {
            id: TransactionId::new(),
            parent: inner.state.head,
            meta,
            ops,
        };
        let record = CommitRecord { revision, txn, aux };

        // Validate against a scratch copy first: an invalid transaction must
        // leave both memory and disk untouched (toy state is tiny; clone is fine).
        let mut next = inner.state.clone();
        next.apply_commit(&record)?;

        // Durable boundary: the record is fsynced before the commit returns.
        // ILRP fault points sit immediately after calls into this function.
        inner.log.append(&record)?;
        let id = record.txn.id;
        inner.state = next;
        Ok((revision, id))
    }
}

/// An open transaction: pending graph operations plus auxiliary writes that
/// will commit atomically in one log record.
#[derive(Debug)]
pub struct GraphTxn<'s> {
    store: &'s GraphStore,
    ops: Vec<Operation>,
    aux: Vec<AuxWrite>,
    authority: TxnAuthority,
    expected_head: Option<GraphRevisionId>,
    denied: bool,
}

impl GraphTxn<'_> {
    /// Queue a graph operation.
    pub fn apply(&mut self, op: Operation) -> Result<(), StoreError> {
        let allowed = match self.operation_allowed(&op) {
            Ok(allowed) => allowed,
            Err(error) => {
                self.denied = true;
                return Err(error);
            }
        };
        if !allowed {
            self.denied = true;
            return Err(StoreError::Conflict(
                "writer cannot apply graph operations".into(),
            ));
        }
        self.ops.push(op);
        Ok(())
    }

    /// Queue an auxiliary durable record in the SAME transaction — this is
    /// what makes the graph store the ILRP coordinator (v4 §7.8: Prepare and
    /// Finalize are each one graph transaction).
    pub fn put_aux(
        &mut self,
        ns: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        self.require_aux_scope(ns, key)?;
        self.aux.push(AuxWrite {
            ns: ns.to_owned(),
            key: key.to_owned(),
            value: Some(value),
        });
        Ok(())
    }

    /// Queue an auxiliary deletion in the same transaction.
    pub fn delete_aux(&mut self, ns: &str, key: &str) -> Result<(), StoreError> {
        self.require_aux_scope(ns, key)?;
        self.aux.push(AuxWrite {
            ns: ns.to_owned(),
            key: key.to_owned(),
            value: None,
        });
        Ok(())
    }

    /// Validate, append one fsynced checksummed record, and advance head.
    /// On error nothing is written and state is unchanged.
    pub fn commit(self, meta: TxnMeta) -> Result<(GraphRevisionId, TransactionId), StoreError> {
        if self.denied {
            return Err(StoreError::Conflict(
                "transaction contains a denied write".into(),
            ));
        }
        self.store
            .commit_txn(self.ops, self.aux, meta, self.expected_head)
    }

    /// Require the store to remain at `head` until this transaction commits.
    /// The comparison occurs under the same mutex as validation and append.
    #[must_use]
    pub fn expect_head(mut self, head: GraphRevisionId) -> Self {
        // A caller may add a guard, never retarget a guard under which writer
        // scope was already checked. Ignoring this refusal cannot revive it.
        if self.expected_head.is_some_and(|expected| expected != head) {
            self.denied = true;
        }
        self.expected_head = Some(head);
        self
    }

    fn require_aux_scope(&mut self, namespace: &str, key: &str) -> Result<(), StoreError> {
        if !self.aux_allowed(namespace, key) {
            self.denied = true;
            return Err(StoreError::Conflict(
                "writer cannot mutate this namespace".into(),
            ));
        }
        Ok(())
    }

    fn aux_allowed(&self, namespace: &str, key: &str) -> bool {
        match self.authority {
            TxnAuthority::Root => true,
            TxnAuthority::Coordinator => namespace == ns::ILRP_INTENT,
            TxnAuthority::Bootstrap => {
                namespace == ns::JUR_ALIAS
                    || (namespace == ns::SYS_BLOB && key.starts_with("file/"))
            }
            TxnAuthority::Capture => {
                matches!(
                    namespace,
                    ns::JUR_PLAN
                        | ns::JUR_DECISION
                        | ns::JUR_OVERLAY
                        | ns::JUR_OVERLAY_LOG
                        | ns::JUR_RECONCILE
                ) || (namespace == ns::SYS_BLOB && is_content_hash_key(key))
            }
            TxnAuthority::Bookkeeping => {
                matches!(
                    namespace,
                    ns::JUR_REPAIR | ns::JUR_OVERLAY | ns::JUR_RECONCILE
                ) || (namespace == ns::SYS_BLOB
                    && (key.starts_with("file/") || key.starts_with("query/")))
            }
            TxnAuthority::HostControl => {
                matches!(namespace, ns::SYS_CLOCK | ns::SYS_UNAVAILABLE)
            }
            TxnAuthority::Reactor(source) => {
                let observation_prefix = format!("obs/{source}/");
                namespace == ns::SYS_BLOB
                    && (key
                        .strip_prefix(&observation_prefix)
                        .is_some_and(is_content_hash_key)
                        || key == format!("obs-current/{source}"))
            }
            TxnAuthority::Reconciliation => namespace == ns::JUR_RECONCILE,
            TxnAuthority::Epoch => namespace == ns::SYS_EPOCH && key == "epoch",
        }
    }

    fn operation_allowed(&self, op: &Operation) -> Result<bool, StoreError> {
        Ok(match self.authority {
            TxnAuthority::Root | TxnAuthority::Coordinator => true,
            TxnAuthority::Bootstrap => matches!(
                op,
                Operation::CreateNode { .. }
                    | Operation::InsertChild { .. }
                    | Operation::AddRelation { .. }
            ),
            TxnAuthority::Reactor(source) => match op {
                Operation::CreateNode { node } => node.kind == crate::kind::EXTERNAL_VALUE,
                Operation::MaterializeExternal {
                    node,
                    source: observed_source,
                    ..
                } => {
                    *observed_source == source
                        && (self.ops.iter().any(|queued| {
                            matches!(
                                queued,
                                Operation::CreateNode { node: queued_node }
                                    if queued_node.id == *node
                                        && queued_node.kind == crate::kind::EXTERNAL_VALUE
                            )
                        }) || self
                            .store
                            .node_at(self.store.head()?, *node)?
                            .is_some_and(|existing| existing.kind == crate::kind::EXTERNAL_VALUE))
                }
                _ => false,
            },
            TxnAuthority::Capture
            | TxnAuthority::Bookkeeping
            | TxnAuthority::HostControl
            | TxnAuthority::Reconciliation
            | TxnAuthority::Epoch => false,
        })
    }
}

fn is_content_hash_key(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Store failure.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Another process holds the store lock.
    #[error("store is locked by another process: {0}")]
    Locked(Utf8PathBuf),
    /// The transaction conflicts with current state.
    #[error("conflict: {0}")]
    Conflict(String),
    /// A referenced subject does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// Non-tail corruption or an internal invariant failure. Detected, never
    /// silently served (v4 §92).
    #[error("store corrupt: {0}")]
    Corrupt(String),
    /// A revision ahead of head was requested. As-of reads (M08.1) serve any
    /// `rev <= head`; only a future revision is unsupported.
    #[error("revision {requested:?} is ahead of head {head:?}")]
    UnsupportedRevision {
        /// The requested revision.
        requested: GraphRevisionId,
        /// The current head.
        head: GraphRevisionId,
    },
    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl GraphStore {
    /// Compare the actual held lock with the currently named lock, not two
    /// re-resolved path strings. A check at assembly time, not a filesystem
    /// lease against arbitrary later same-user directory replacement.
    pub(crate) fn matches_directory(&self, dir: &Utf8Path) -> Result<bool, StoreError> {
        if self.dir != dir {
            return Ok(false);
        }
        let inner = self.lock()?;
        // Match the permissions required by the original write-lock open. Do
        // not create, truncate, or write the named file while checking identity.
        let named_file = match fs::OpenOptions::new().write(true).open(dir.join("lock")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let named = same_file::Handle::from_file(named_file)?;
        Ok(inner.lock_handle == named)
    }
}
