//! A minimal memoization table proving component-granular invalidation
//! (v4 §7.5 last paragraph; R4 §10 "buffer generations invalidate only
//! dependent queries"). Reused by M08.
//!
//! D06.6: staleness is defined on component VALUES, not keys. All three
//! perspectives read the same `path:notes.md` key, so key equality cannot tell
//! clients apart — an entry is stale iff re-resolving its perspective yields,
//! for some key it read, a component different from the value it captured.

use std::collections::{BTreeMap, HashMap};

use liminal_id::JurisdictionKey;
use liminal_revision::{BasisComponent, ComponentDeps, WorkspaceBasis};

/// One memoized computation result plus the exact component values it read.
#[derive(Debug, Clone)]
pub struct MemoEntry {
    /// The computed value.
    pub value: String,
    /// The keys this computation read.
    pub deps: ComponentDeps,
    /// The component VALUES captured for those keys at compute time (D06.6).
    pub snapshot: BTreeMap<JurisdictionKey, BasisComponent>,
}

/// A memo table keyed by `(query, perspective-label)`.
#[derive(Debug, Default)]
pub struct MemoTable {
    entries: HashMap<(String, String), MemoEntry>,
}

impl MemoTable {
    /// A fresh empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a memoized result.
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

    /// Whether the entry is stale against a freshly resolved Basis (D06.6):
    /// for some read key, the current component differs from the captured one.
    /// A key that read a component now absent is also stale.
    #[must_use]
    pub fn is_stale(&self, query: &str, perspective_label: &str, current: &WorkspaceBasis) -> bool {
        let Some(entry) = self.get(query, perspective_label) else {
            // Never computed ⇒ trivially "stale" (must compute).
            return true;
        };
        for key in &entry.deps.read {
            let captured = entry.snapshot.get(key);
            let now = current.components.get(key);
            if captured != now {
                return true;
            }
        }
        false
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
///
/// The minimal M06 query `render_paragraphs(notes.md)` reads exactly the
/// components in `basis`, so its deps are all of `basis.components`' keys.
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

#[cfg(test)]
mod tests {
    use liminal_id::{ContentHash, PathId, TransactionId};
    use liminal_revision::{AvailableInputs, BasisPerspective, resolve};

    use super::*;

    fn durable_inputs(bytes: &[u8]) -> AvailableInputs {
        let mut inputs = AvailableInputs::default();
        let path = PathId("notes.md".into());
        inputs.durable.insert(
            JurisdictionKey::Path(path.clone()),
            BasisComponent::FileContent {
                path,
                hash: ContentHash::of(bytes),
            },
        );
        inputs
    }

    #[test]
    fn fresh_entry_is_not_stale_against_same_basis() {
        let inputs = durable_inputs(b"v0");
        let basis = resolve(
            &inputs,
            &BasisPerspective::DurableOnly,
            TransactionId::new(),
        )
        .unwrap();
        let mut table = MemoTable::new();
        table.insert("q", "durable", entry_over_basis("rendered".into(), &basis));
        assert!(!table.is_stale("q", "durable", &basis));
    }

    #[test]
    fn changed_component_value_makes_entry_stale() {
        let inputs0 = durable_inputs(b"v0");
        let basis0 = resolve(
            &inputs0,
            &BasisPerspective::DurableOnly,
            TransactionId::new(),
        )
        .unwrap();
        let mut table = MemoTable::new();
        table.insert("q", "durable", entry_over_basis("rendered".into(), &basis0));

        // Durable bytes change ⇒ the FileContent hash differs ⇒ stale.
        let inputs1 = durable_inputs(b"v1");
        let basis1 = resolve(
            &inputs1,
            &BasisPerspective::DurableOnly,
            TransactionId::new(),
        )
        .unwrap();
        assert!(table.is_stale("q", "durable", &basis1));
    }

    #[test]
    fn uncomputed_query_is_stale() {
        let inputs = durable_inputs(b"v0");
        let basis = resolve(
            &inputs,
            &BasisPerspective::DurableOnly,
            TransactionId::new(),
        )
        .unwrap();
        let table = MemoTable::new();
        assert!(table.is_stale("never", "durable", &basis));
    }
}
