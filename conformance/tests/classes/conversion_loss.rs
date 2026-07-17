//! §113 differential-conversion class: every conversion honors its DECLARED
//! loss contract (Law 8; v4 §14, §107) — loss is visible, never silent.

/// The -1.4 spike, made permanent: toy Resolved-Graph → Pandoc JSON AST →
/// back, with measured loss equal to the committed golden report
/// (`conformance/golden/pandoc_loss.md`) and ID survival recorded honestly.
#[test]
#[ignore = "Phase -1 M10: Pandoc adapter-boundary spike"]
fn pandoc_roundtrip_loss_matches_golden() {
    unimplemented!("drive `pandoc` CLI; compare loss report byte-exactly to golden")
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
