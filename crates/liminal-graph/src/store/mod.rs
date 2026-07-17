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

mod log;
pub mod ns;
mod snapshot;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Mutex;

use camino::{Utf8Path, Utf8PathBuf};
use fs4::fs_std::FileExt as _;
use liminal_id::{GraphRevisionId, NodeId, RelationId, TransactionId};
use serde::{Deserialize, Serialize};

use crate::node::{Node, PayloadRef};
use crate::op::{Operation, Transaction, TxnMeta};
use crate::relation::Relation;

pub(crate) use log::SegmentLog;

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
    _lock: fs::File,
}

/// The toy transactional graph store and ILRP coordinator (v4 §92; ADR-0007).
///
/// Single-writer (advisory file lock). All reads are head-revision reads;
/// as-of/history queries are the Phase -1.3 (M8) milestone.
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

impl GraphStore {
    /// Open (or create) a store at `dir`, recovering state per §92: load the
    /// snapshot if present, replay checksummed segments, truncate a torn tail.
    pub fn open(dir: &Utf8Path) -> Result<Self, StoreError> {
        fs::create_dir_all(dir)?;
        let lock_path = dir.join("lock");
        let lock = fs::File::create(&lock_path)?;
        if !lock.try_lock_exclusive()? {
            return Err(StoreError::Locked(dir.to_owned()));
        }

        let (mut state, base_segment) = snapshot::load(dir)?;
        let log = SegmentLog::recover(dir, base_segment, |record| {
            state.apply_commit(record).map_err(|e| e.to_string())
        })?;

        Ok(Self {
            dir: dir.to_owned(),
            inner: Mutex::new(Inner {
                state,
                log,
                _lock: lock,
            }),
        })
    }

    /// Current head revision.
    pub fn head(&self) -> Result<GraphRevisionId, StoreError> {
        Ok(self.lock()?.state.head)
    }

    /// Begin a transaction. Nothing is durable until [`GraphTxn::commit`].
    pub fn begin(&self) -> Result<GraphTxn<'_>, StoreError> {
        Ok(GraphTxn {
            store: self,
            ops: Vec::new(),
            aux: Vec::new(),
        })
    }

    /// Read a Node at a revision. Only head reads are supported until the
    /// as-of prototype (Phase -1.3 / M8).
    pub fn node_at(&self, rev: GraphRevisionId, id: NodeId) -> Result<Option<Node>, StoreError> {
        let inner = self.lock()?;
        Self::require_head(&inner.state, rev)?;
        Ok(inner.state.nodes.get(&id).cloned())
    }

    /// Read a Relation at a revision (head-only, see [`Self::node_at`]).
    pub fn relation_at(
        &self,
        rev: GraphRevisionId,
        id: RelationId,
    ) -> Result<Option<Relation>, StoreError> {
        let inner = self.lock()?;
        Self::require_head(&inner.state, rev)?;
        Ok(inner.state.relations.get(&id).cloned())
    }

    /// All Relations whose source is `source` (head-only, see [`Self::node_at`]).
    pub fn relations_from(
        &self,
        rev: GraphRevisionId,
        source: NodeId,
    ) -> Result<Vec<Relation>, StoreError> {
        let inner = self.lock()?;
        Self::require_head(&inner.state, rev)?;
        Ok(inner
            .state
            .relations
            .values()
            .filter(|r| r.source == source)
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
    pub fn snapshot(&self) -> Result<(), StoreError> {
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

    fn require_head(state: &State, rev: GraphRevisionId) -> Result<(), StoreError> {
        if rev == state.head {
            Ok(())
        } else {
            Err(StoreError::UnsupportedRevision {
                requested: rev,
                head: state.head,
            })
        }
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
    ) -> Result<(GraphRevisionId, TransactionId), StoreError> {
        let mut inner = self.lock()?;
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
}

impl GraphTxn<'_> {
    /// Queue a graph operation.
    pub fn apply(&mut self, op: Operation) -> Result<(), StoreError> {
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
        self.aux.push(AuxWrite {
            ns: ns.to_owned(),
            key: key.to_owned(),
            value: Some(value),
        });
        Ok(())
    }

    /// Queue an auxiliary deletion in the same transaction.
    pub fn delete_aux(&mut self, ns: &str, key: &str) -> Result<(), StoreError> {
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
        self.store.commit_txn(self.ops, self.aux, meta)
    }
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
    /// As-of reads are not implemented until Phase -1.3 (M8).
    #[error(
        "only head-revision reads are supported (requested {requested:?}, head {head:?}); as-of queries are Phase -1.3"
    )]
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
