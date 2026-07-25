//! §113 golden-render class: deterministic HTML output plus
//! full-vs-incremental patch equivalence (v4 §27, §112).

/// Full-document HTML for each fixture matches its insta snapshot
/// byte-exactly (deterministic builds, v4 §110).
#[test]
#[ignore = "Phase 1: native HTML backend"]
fn full_document_html_matches_golden() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/phase1/render");
    let source = std::fs::read_to_string(root.join("basic.md")).expect("render source");
    let expected = std::fs::read_to_string(root.join("basic.html")).expect("render golden");
    let actual = liminal_format::MarkdownRenderer
        .render(&source)
        .expect("render");
    assert_eq!(actual, expected);
}

/// Applying an edit incrementally patches the DOM/HTML to EXACTLY the output
/// of a from-scratch render at the new Basis (v4 §112 incremental ≡ full,
/// specialized to the render path; a paragraph edit must not recompile the
/// document).
#[test]
#[ignore = "Phase 1: incremental HTML patching"]
fn incremental_patch_equals_full_render() {
    use std::collections::BTreeMap;

    use liminal_id::TransactionId;
    use liminal_query::{IncrementalCompiler, ParagraphCompiler, SourceEdit};
    use liminal_revision::{BasisPerspective, WorkspaceBasis};

    let source = "one {#a}\n\ntwo {#b}".to_owned();
    let edit_start = source.find("two").expect("edit anchor");
    let edit = SourceEdit {
        start: edit_start,
        end: edit_start + 3,
        replacement: "TWO".to_owned(),
    };
    let compiler = ParagraphCompiler;
    let basis = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    let edited = compiler.apply(&source, std::slice::from_ref(&edit));
    let incremental_blocks = compiler.incremental(&source, &[edit], &basis);
    let full = liminal_format::MarkdownRenderer
        .render(&edited)
        .expect("full render");
    let patched = liminal_format::MarkdownRenderer
        .render_blocks(&incremental_blocks)
        .expect("incremental patch render");
    assert_eq!(patched, full);
}
