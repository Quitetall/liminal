//! The R4 §2.3 SLO gate: each stable profile's held-out scorecard must clear
//! the hard-coded thresholds in `liminal_conformance::Scorecard`. Profile
//! graduation = these pass on `corpora/heldout/v1`.
//!
//! The threshold LOGIC is active today (unit-tested in `src/slo.rs`); these
//! tests wire it to the real pipeline at M11.

/// The external-file profile clears every R4 §2.3 threshold on the locked
/// held-out corpus: ≥99% auto-resolution, ≥99% intervention-free sessions,
/// zero sound-session diagnostics, byte-empty sound checker output, zero
/// unexpected reconciliation items, ≤1 visible incident per root cause, zero
/// manual Contract authoring.
#[test]
#[ignore = "Phase -1 M11: scorecard pipeline over heldout/v1 (external-file)"]
fn external_file_profile_graduates_on_heldout_corpus() {
    unimplemented!("run pipeline; assert Scorecard::violations().is_empty()")
}

/// The graph-native profile clears the same thresholds. Measured PER PROFILE
/// — an aggregate must never hide a weak domain (v4 §7.4).
#[test]
#[ignore = "Phase -1 M11: scorecard pipeline over heldout/v1 (graph-native)"]
fn graph_native_profile_graduates_on_heldout_corpus() {
    unimplemented!("run pipeline; assert Scorecard::violations().is_empty()")
}

/// Profile-declared transient drafts (offline work awaiting sync) are
/// reported separately and become DEBT past their policy window — they never
/// silently count as resolved (R4 §2.3).
#[test]
#[ignore = "Phase -1 M11: transient-draft accounting"]
fn transient_drafts_are_reported_separately_and_age_into_debt() {
    unimplemented!("replay offline/online trace; assert separate accounting + aging")
}
