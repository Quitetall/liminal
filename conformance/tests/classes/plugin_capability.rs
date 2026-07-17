//! §98/§113 plugin-capability class: effects are capability-gated, logged,
//! and attributable (Law 5).

/// A Wasm plugin without a capability cannot perform its effect; the denial
/// is logged with the plugin's identity.
#[test]
#[ignore = "Phase 11: Wasm component host + capability runtime"]
fn plugin_without_capability_is_denied_and_logged() {
    unimplemented!("load test plugin lacking net capability; assert denial + audit log entry")
}

/// Traversing a Relation never performs an effect (Law 5: "traversing a
/// Relation must not unexpectedly perform an effect").
#[test]
#[ignore = "Phase 7: resolver runtime (effects live outside tracked queries, v4 §8.6)"]
fn relation_traversal_performs_no_effects() {
    unimplemented!("inspect external Relation; assert zero resolver invocations")
}
