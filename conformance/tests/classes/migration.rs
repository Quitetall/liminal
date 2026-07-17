//! §113 snapshot-migration class: persisted formats migrate forward without
//! loss (activation: the first persisted-format ADR, v4 §121).

/// A store/snapshot written by version N opens under version N+1 with
/// identical semantic content (v4 §113 "snapshot migration tests").
#[test]
#[ignore = "Phase 1+: first persisted-format ADR (v4 §121) freezes a format to migrate from"]
fn old_snapshots_open_after_format_evolution() {
    unimplemented!("open committed fixtures/migration/** stores; assert semantic equality")
}
