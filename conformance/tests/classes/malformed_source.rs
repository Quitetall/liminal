//! §113 malformed-source recovery class: parsing never panics, always yields
//! an error-tolerant CST preserving the invalid regions (v4 §9, §22).

/// Every fixture under `fixtures/malformed-source/` parses without panic into
/// a CST that preserves invalid regions as opaque blocks with exact ranges,
/// and `emit` round-trips the damaged bytes losslessly (L0 is lossless).
#[test]
#[ignore = "Phase 1: error-tolerant CST (liminal-cst)"]
fn malformed_source_never_panics_and_round_trips() {
    unimplemented!("glob fixtures/malformed-source/**; parse; assert lossless emit")
}

/// Fuzz smoke lives in fuzz/ (created with liminal-cst, see
/// docs/implementation-plan.md defer list); this class holds its minimized
/// regression cases per §113.
#[test]
#[ignore = "Phase 1: fuzz-derived regression corpus replay"]
fn fuzz_regressions_stay_fixed() {
    unimplemented!("replay conformance/corpora/regression/parser/** minimized cases")
}
