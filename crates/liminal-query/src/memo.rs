//! The tiny pure memo layer (M08.4, D08.4) — this crate's promotion from a
//! reserved seam to a working component. A memoized query result is keyed by
//! `(query, perspective)` and carries the exact component VALUES it read
//! (D06.6): an entry is stale iff re-resolving the Basis yields, for some key it
//! read, a component different from the value captured at compute time. Editing
//! buffer A must not invalidate a computation that depends only on buffer B
//! (v4 §7.5; R4 §10).
//!
//! No Salsa, no parallel scheduling, no incremental dependency graph — just an
//! explicit table with equality-based staleness and hit/miss counters, enough
//! to demonstrate component-granular invalidation (Law 14).

use std::collections::{BTreeMap, HashMap};

use liminal_id::JurisdictionKey;
use liminal_revision::{BasisComponent, ComponentDeps, WorkspaceBasis};

use crate::Query;

/// One memoized computation result plus the exact component values it read.
#[derive(Debug, Clone)]
pub struct MemoEntry {
    /// The computed value (canonical string form — what freeze/replay compares).
    pub value: String,
    /// The keys this computation read.
    pub deps: ComponentDeps,
    /// The component VALUES captured for those keys at compute time (D06.6).
    pub snapshot: BTreeMap<JurisdictionKey, BasisComponent>,
}

impl MemoEntry {
    /// Whether this entry is stale against a freshly resolved Basis (D06.6): for
    /// some read key, the current component differs from the captured one (an
    /// absent-now component also counts as changed).
    #[must_use]
    pub fn is_stale(&self, current: &WorkspaceBasis) -> bool {
        self.deps
            .read
            .iter()
            .any(|key| self.snapshot.get(key) != current.components.get(key))
    }
}

/// A memo table keyed by `(query, perspective-label)`, tracking hit/miss counts
/// so component-granular invalidation is observable (the four-consumers gate
/// asserts that a phone edit invalidates only the phone-scoped entry).
#[derive(Debug, Default)]
pub struct MemoTable {
    entries: HashMap<(String, String), MemoEntry>,
    hits: u64,
    misses: u64,
}

impl MemoTable {
    /// A fresh empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a memoized result directly (used by the M06 invalidation gate,
    /// which computes the value out-of-band).
    pub fn insert(&mut self, query: &str, perspective_label: &str, entry: MemoEntry) {
        self.entries
            .insert((query.to_owned(), perspective_label.to_owned()), entry);
    }

    /// The memoized entry for `(query, perspective-label)`, if any.
    #[must_use]
    pub fn get(&self, query: &str, perspective_label: &str) -> Option<&MemoEntry> {
        self.entries
            .get(&(query.to_owned(), perspective_label.to_owned()))
    }

    /// Whether the entry is stale against a freshly resolved Basis (D06.6).
    /// An uncomputed entry is trivially stale (it must be computed).
    #[must_use]
    pub fn is_stale(&self, query: &str, perspective_label: &str, current: &WorkspaceBasis) -> bool {
        self.get(query, perspective_label)
            .is_none_or(|entry| entry.is_stale(current))
    }

    /// Execute `query` under `basis` with memoization (D08.4). Serves the cached
    /// value on a fresh hit (bumping [`Self::hits`]); otherwise runs the query,
    /// records its precise component deps, snapshots their values, caches, and
    /// bumps [`Self::misses`]. The query's output IS its canonical string.
    pub fn execute<Q>(
        &mut self,
        query_name: &str,
        perspective_label: &str,
        query: &Q,
        basis: &WorkspaceBasis,
    ) -> String
    where
        Q: Query<Output = String>,
    {
        let fresh = self
            .get(query_name, perspective_label)
            .filter(|entry| !entry.is_stale(basis))
            .map(|entry| entry.value.clone());
        if let Some(value) = fresh {
            self.hits += 1;
            return value;
        }
        self.misses += 1;
        let mut deps = ComponentDeps::default();
        let value = query.execute(basis, &mut deps);
        let snapshot = deps
            .read
            .iter()
            .filter_map(|k| basis.components.get(k).map(|c| (k.clone(), c.clone())))
            .collect();
        self.entries.insert(
            (query_name.to_owned(), perspective_label.to_owned()),
            MemoEntry {
                value: value.clone(),
                deps,
                snapshot,
            },
        );
        value
    }

    /// Cache hits observed so far.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Cache misses (recomputations) observed so far.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Number of stored entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Build a `MemoEntry` by recording every key of the resolved basis as a
/// dependency and snapshotting their values, alongside a caller-computed value.
/// (The M06 `render_paragraphs` query reads exactly the components in `basis`.)
#[must_use]
pub fn entry_over_basis(value: String, basis: &WorkspaceBasis) -> MemoEntry {
    let mut deps = ComponentDeps::default();
    let mut snapshot = BTreeMap::new();
    for (key, comp) in &basis.components {
        deps.record(key.clone());
        snapshot.insert(key.clone(), comp.clone());
    }
    MemoEntry {
        value,
        deps,
        snapshot,
    }
}
