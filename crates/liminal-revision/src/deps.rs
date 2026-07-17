//! Component-granular dependency sets (v4 §7.5 last paragraph).

use std::collections::BTreeSet;

use liminal_id::JurisdictionKey;

/// The exact Basis components a computation read.
///
/// This is the invalidation contract of the Phase -1.0 exit gate: editing
/// buffer A must not invalidate computations that depend only on buffer B or
/// an unrelated object (v4 §7.5; R4 §10 "buffer generations invalidate only
/// dependent queries").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentDeps {
    /// Keys of every component this computation read.
    pub read: BTreeSet<JurisdictionKey>,
}

impl ComponentDeps {
    /// Record that a component was read.
    pub fn record(&mut self, key: JurisdictionKey) {
        self.read.insert(key);
    }

    /// Whether a change to `changed` invalidates this computation.
    #[must_use]
    pub fn invalidated_by(&self, changed: &JurisdictionKey) -> bool {
        self.read.contains(changed)
    }
}

#[cfg(test)]
mod tests {
    use liminal_id::PathId;

    use super::*;

    #[test]
    fn unrelated_component_does_not_invalidate() {
        let mut deps = ComponentDeps::default();
        let a = JurisdictionKey::Path(PathId("a.md".into()));
        let b = JurisdictionKey::Path(PathId("b.md".into()));
        deps.record(a.clone());
        assert!(deps.invalidated_by(&a));
        assert!(!deps.invalidated_by(&b));
    }
}
