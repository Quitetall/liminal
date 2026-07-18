//! The component-granular memo table now lives in `liminal-query` (M08.4,
//! D08.4 — the crate's promotion). This module re-exports it so the M06
//! invalidation gate keeps its `liminal_conformance::memo::*` path.
//!
//! D06.6: staleness is defined on component VALUES, not keys. All three
//! perspectives read the same `path:notes.md` key, so key equality cannot tell
//! clients apart — an entry is stale iff re-resolving its perspective yields,
//! for some key it read, a component different from the value it captured.

pub use liminal_query::memo::{MemoEntry, MemoTable, entry_over_basis};

#[cfg(test)]
mod tests {
    use liminal_id::{ContentHash, JurisdictionKey, PathId, TransactionId};
    use liminal_revision::{AvailableInputs, BasisComponent, BasisPerspective, resolve};

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
