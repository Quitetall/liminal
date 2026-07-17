//! §113 golden-render class: deterministic HTML output plus
//! full-vs-incremental patch equivalence (v4 §27, §112).

/// Full-document HTML for each fixture matches its insta snapshot
/// byte-exactly (deterministic builds, v4 §110).
#[test]
#[ignore = "Phase 1: native HTML backend"]
fn full_document_html_matches_golden() {
    unimplemented!("render fixtures; insta::assert_snapshot! per document")
}

/// Applying an edit incrementally patches the DOM/HTML to EXACTLY the output
/// of a from-scratch render at the new Basis (v4 §112 incremental ≡ full,
/// specialized to the render path; a paragraph edit must not recompile the
/// document).
#[test]
#[ignore = "Phase 1: incremental HTML patching"]
fn incremental_patch_equals_full_render() {
    unimplemented!("edit fixture; compare patched output to fresh render")
}
