//! §113 differential-conversion class: every conversion honors its DECLARED
//! loss contract (Law 8; v4 §14, §107) — loss is visible, never silent.

/// The -1.4 spike, made permanent: toy Resolved-Graph → Pandoc JSON AST →
/// back, with measured loss equal to the committed golden report
/// (`conformance/golden/pandoc_loss.md`) and ID survival recorded honestly.
/// The report carries no volatile lines (the pandoc version is pinned), so
/// the comparison is byte-exact; regenerate deliberately with
/// `BLESS_PANDOC_REPORT=1` and review the diff (a golden is spec).
#[test]
fn pandoc_roundtrip_loss_matches_golden() {
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixtures/conversion-loss/pandoc");
    let workdir = liminal_scratch::ScratchDir::new("m10-loss").expect("scratch dir");
    let measurement =
        liminal_conformance::pandoc::measure(&fixture, &workdir).expect("measurement runs");
    let rendered = liminal_conformance::pandoc::loss_report(&measurement);

    let golden_path = root.join("golden/pandoc_loss.md");
    if std::env::var("BLESS_PANDOC_REPORT").is_ok() || !golden_path.exists() {
        std::fs::write(&golden_path, &rendered).expect("write golden");
    }
    let golden = std::fs::read_to_string(&golden_path).expect("read pandoc_loss.md golden");
    assert_eq!(
        rendered, golden,
        "pandoc loss report drifted from the committed golden; if intended, \
         regenerate with BLESS_PANDOC_REPORT=1 and review the diff"
    );
}

/// Unknown constructs survive import as namespaced opaque Nodes with source
/// payload and declared loss status (v4 §107).
#[test]
#[ignore = "Phase 4: importer adapter framework"]
fn unknown_constructs_are_preserved_opaquely() {
    unimplemented!("import fixture with unsupported constructs; assert opaque preservation")
}

/// A destructive conversion warns before discarding (v4 §14: "the compiler
/// must warn before a destructive conversion unless the user explicitly
/// accepts the contract").
#[test]
#[ignore = "Phase 4: conversion-loss diagnostics"]
fn destructive_conversion_warns_first() {
    unimplemented!("convert fixture losing declared facets; assert warning precedes output")
}
