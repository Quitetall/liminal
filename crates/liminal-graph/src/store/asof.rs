//! As-of reads (M08.1, v4 §-1.3, §24): materialize a read-only [`StateView`] at
//! any historical revision by replaying the retained segments from genesis. The
//! toy never garbage-collects segments, so genesis replay is always available;
//! O(n) is toy-fine (prohibition #8: no production store work).

use std::collections::BTreeMap;

use liminal_id::{GraphRevisionId, NodeId, RelationId};

use super::{GraphStore, State, StoreError, log};
use crate::node::Node;
use crate::relation::Relation;

/// A read-only materialized view of graph state at one revision. Owns the
/// `State` reconstructed at that revision; every accessor mirrors the head-read
/// API of [`GraphStore`] but is served from the frozen view.
#[derive(Debug)]
pub struct StateView {
    state: State,
}

impl StateView {
    /// The Node with `id` at this revision, if present.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.state.nodes.get(&id)
    }

    /// The Relation with `id` at this revision, if present.
    #[must_use]
    pub fn relation(&self, id: RelationId) -> Option<&Relation> {
        self.state.relations.get(&id)
    }

    /// Every Relation whose source is `source` at this revision (id order).
    #[must_use]
    pub fn relations_from(&self, source: NodeId) -> Vec<&Relation> {
        self.state
            .relations
            .values()
            .filter(|r| r.source == source)
            .collect()
    }

    /// The ordered children of `parent` at this revision.
    #[must_use]
    pub fn children(&self, parent: NodeId) -> &[NodeId] {
        self.state.children.get(&parent).map_or(&[], Vec::as_slice)
    }

    /// Every Node at this revision, in id order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.state.nodes.values()
    }

    /// Every Relation at this revision, in id order.
    pub fn relations(&self) -> impl Iterator<Item = &Relation> {
        self.state.relations.values()
    }

    /// Every auxiliary record in namespace `ns` at this revision, in key order.
    pub fn scan_aux(&self, ns: &str) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.state.aux.get(ns).into_iter().flat_map(|m| m.iter())
    }

    /// An auxiliary record at this revision.
    #[must_use]
    pub fn get_aux(&self, ns: &str, key: &str) -> Option<&serde_json::Value> {
        self.state.aux.get(ns).and_then(|m| m.get(key))
    }

    /// The revision this view was materialized at.
    #[must_use]
    pub fn head(&self) -> GraphRevisionId {
        self.state.head
    }

    /// Build a `StateView` from raw parts (M08.8, AM-8.12): the frozen-replay
    /// counterpart to [`GraphStore::state_at`], which materializes one by
    /// replaying the log. A frozen-basis replay world has no live store and
    /// no log to replay against, so it builds its `StateView` synthetically
    /// from already-deserialized parts instead. `transactions` is always
    /// empty — no `StateView` accessor reads it.
    #[must_use]
    pub fn synthetic(
        head: GraphRevisionId,
        nodes: BTreeMap<NodeId, Node>,
        relations: BTreeMap<RelationId, Relation>,
        children: BTreeMap<NodeId, Vec<NodeId>>,
        aux: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    ) -> Self {
        Self {
            state: State {
                head,
                nodes,
                relations,
                children,
                transactions: BTreeMap::new(),
                aux,
            },
        }
    }
}

impl GraphStore {
    /// Materialize state as of `rev` (M08.1). Replays retained segments from
    /// genesis into a scratch `State`, applying records up to and including
    /// `rev`. `rev == head` clones the in-memory state directly.
    ///
    /// # Errors
    /// [`StoreError::UnsupportedRevision`] iff `rev` is ahead of head; I/O or
    /// corruption errors propagate from the segment replay.
    pub fn state_at(&self, rev: GraphRevisionId) -> Result<StateView, StoreError> {
        let guard = self.lock()?;
        let head = guard.state.head;
        if rev.0 > head.0 {
            return Err(StoreError::UnsupportedRevision {
                requested: rev,
                head,
            });
        }
        if rev == head {
            return Ok(StateView {
                state: guard.state.clone(),
            });
        }

        // Replay from genesis, stopping after the record whose revision == rev.
        let mut state = State::default();
        log::replay_from_genesis(self.dir(), |record| {
            if record.revision.0 <= rev.0 {
                state.apply_commit(record).map_err(|e| e.to_string())?;
            }
            Ok(())
        })?;
        Ok(StateView { state })
    }
}
