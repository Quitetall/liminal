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
use camino::Utf8PathBuf;

/// Run the pipeline over the LOCKED heldout/v1 for one profile and return
/// (the JSON value in the scorecard bin's shape, the violations list).
fn heldout_run(profile: &str) -> (serde_json::Value, Vec<String>) {
    let corpus = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpora/heldout/v1");
    let (scorecard, drafts) =
        liminal_conformance::pipeline::run(&corpus, profile).expect("heldout replay must succeed");
    let violations = scorecard.violations();
    let value = serde_json::json!({
        "scorecard": scorecard,
        "transient_drafts": drafts,
        "violations": violations,
    });
    (value, violations)
}

/// The recorded one-shot scores (M11.8 ceremony; v1 frozen forever).
fn recorded_golden() -> serde_json::Value {
    let path =
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/scorecard-heldout-v1.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("recorded scorecard golden exists"))
        .expect("golden parses")
}

/// R4 §2.3 graduation on the LOCKED held-out corpus: auto-resolution and
/// intervention-free rates at threshold, zero sound-session diagnostics,
/// byte-empty sound checker output, zero unexpected reconciliation items,
/// ≤1 visible incident per root cause, zero manual Contract authoring.
#[test]
fn external_file_profile_graduates_on_heldout_corpus() {
    let (value, violations) = heldout_run("external-file");
    assert!(violations.is_empty(), "graduation violated: {violations:?}");
    assert_eq!(
        value,
        recorded_golden()["external-file"],
        "replay must reproduce the recorded M11.8 one-shot score exactly \
         (v1 is frozen; drift here is a harness regression, never a corpus edit)"
    );
}

/// The graph-native profile clears the same thresholds. Measured PER PROFILE
/// — an aggregate must never hide a weak domain (v4 §7.4).
#[test]
fn graph_native_profile_graduates_on_heldout_corpus() {
    let (value, violations) = heldout_run("graph-native");
    assert!(violations.is_empty(), "graduation violated: {violations:?}");
    assert_eq!(
        value,
        recorded_golden()["graph-native"],
        "replay must reproduce the recorded M11.8 one-shot score exactly"
    );
}

/// Profile-declared transient drafts (offline work awaiting sync) are
/// reported separately and become DEBT past their policy window — they never
/// silently count as resolved (R4 §2.3).
#[test]
fn transient_drafts_are_reported_separately_and_age_into_debt() {
    let (value, _violations) = heldout_run("external-file");
    let drafts = &value["transient_drafts"];
    assert!(
        drafts["declared"].as_u64().unwrap_or(0) > 0,
        "the offline-window trace must declare a transient draft: {drafts}"
    );
    assert!(
        drafts["aged_into_debt"].as_u64().unwrap_or(0) > 0,
        "past the policy window the draft must become visible DEBT: {drafts}"
    );
    // Separate accounting: the aged draft never dents the auto-resolution
    // rate (excluded from numerator AND denominator, R4 §2.3).
    let rate = value["scorecard"]["auto_resolution_rate"].as_f64().unwrap();
    assert!(
        (rate - 1.0).abs() < f64::EPSILON,
        "drafts must not dent the rate: {rate}"
    );
}
