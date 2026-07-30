//! M10 exit-gate born-passing tests: the D10.3 pandoc version pin and (at
//! M10.4) the declared-level recomputation.

use liminal_conformance::pandoc;

/// D10.3: the pinned toolchain is present and exact — `pandoc --version`
/// first line equals `pandoc 3.6.1`. A missing or mismatched pandoc PANICS
/// (no silent skip, no `#[ignore]`): a skipped loss report would fake Law 8.
#[test]
fn pandoc_version_pinned() {
    pandoc::assert_pandoc_pinned();
}

/// M10.4: recompute the capability level from the live measurements via the
/// D10.4 rule and assert it equals the adapter's `DECLARED_LEVEL` constant —
/// the declaration can never drift from the measurement (mirrors M09's
/// `declared_level_matches_report`).
#[test]
fn declared_level_matches_loss_report() {
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixtures/conversion-loss/pandoc");
    let workdir = liminal_scratch::ScratchDir::new("m10-level").expect("scratch dir");
    let measurement = pandoc::measure(&fixture, &workdir).expect("measurement runs");
    assert_eq!(
        measurement.declared_level(),
        pandoc::DECLARED_LEVEL,
        "computed level != DECLARED_LEVEL; the measurement changed — \
         re-examine before touching the constant (a level is spec)"
    );
}
