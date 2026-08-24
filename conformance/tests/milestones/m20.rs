//! M20 exit-gate tests: the canonical formatter and `lim fmt`.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs`.

use std::fs;

use camino::Utf8Path;
use liminal_cli::{format_workspace, format_workspace_with_failure_after};
use liminal_scratch::ScratchDir;

fn fixture() -> ScratchDir {
    let root = ScratchDir::new("m20-cli").expect("scratch root");
    fs::write(
        root.join("a.md"),
        "#!liminal-explicit-v1\nnode a { literal \"alpha\"; }\n",
    )
    .expect("a");
    fs::write(
        root.join("b.md"),
        "#!liminal-explicit-v1\nnode b { literal \"beta\"; }\n",
    )
    .expect("b");
    root
}

fn bytes(root: &Utf8Path, name: &str) -> Vec<u8> {
    fs::read(root.join(name)).expect("fixture bytes")
}

use liminal_format::{Formatter, MarkdownFormatter};
use liminal_hir::SourceDialect;
use liminal_id::ContentHash;

/// D20: formatted output must be attributable to the exact source it was
/// produced from.
///
/// Checked by discrimination rather than by reading the formatter's own
/// bookkeeping (ADR-0020 §5): two sources that differ must not format to the
/// same bytes, or the output cannot name its input at all.
#[test]
#[ignore = "Phase 1: M20 canonical formatter (AM-17.2 quarantine)"]
fn formatted_output_records_the_basis_it_formatted_at() {
    let formatter = MarkdownFormatter::default();
    let left = "alpha {#a}";
    let right = "beta {#b}";
    assert_ne!(
        ContentHash::of(left.as_bytes()),
        ContentHash::of(right.as_bytes()),
        "the fixtures must differ, or this test proves nothing"
    );

    let formatted_left = formatter.format(left).expect("format must be total");
    let formatted_right = formatter.format(right).expect("format must be total");
    assert_ne!(
        formatted_left, formatted_right,
        "distinct sources must not collapse to identical formatted output"
    );

    // The parsed document must carry the basis its formatter was pinned to,
    // not a basis invented at emit time.
    let document = formatter.parse(left).expect("parse must be total");
    assert_eq!(
        document.hir.basis.content_hash,
        ContentHash::of(left.as_bytes()),
        "the document must record the hash of the source it parsed"
    );
}

/// D20: a document the requested dialect cannot express falls back to the
/// canonical surface rather than emitting a lossy approximation.
///
/// This is the property M17.5's F-21 violated in the other direction: an empty
/// ordered block was emitted as `ordered;`, a form the grammar does not have,
/// so the "compact" rendering was not merely lossy but unparsable. Falling
/// back is always available because the explicit surface can express every
/// form.
#[test]
#[ignore = "Phase 1: M20 canonical formatter (AM-17.2 quarantine)"]
fn formatter_refuses_unrepresentable_documents_instead_of_lossy_emission() {
    let formatter = MarkdownFormatter::default();
    // Richer than the compact surface can spell: a relation and an ordered
    // block have no compact form at all.
    let source = concat!(
        "#!liminal-explicit-v1\n",
        "node root (id = \"root\") {\n",
        "  literal \"value\";\n",
        "}\n",
        "relation @root -[links]-> @root;\n",
        "ordered {\n  literal \"tail\";\n}\n",
    );
    let document = formatter.parse(source).expect("parse must be total");
    let compact = formatter
        .emit_in_dialect(&document, SourceDialect::CompactOrExplicitV1)
        .expect("emission must be total");

    assert!(
        compact.starts_with("#!liminal-explicit-v1"),
        "an unrepresentable document must fall back to the explicit surface, got {compact:?}"
    );
    // The fallback must be lossless, which is the whole point of falling back.
    let round = formatter.parse(&compact).expect("fallback must reparse");
    assert_eq!(
        round, document,
        "the fallback surface must round-trip the document it could not compact"
    );
}

/// A failure after one replacement leaves no staged sibling and preserves the
/// already-completed file as durable partial progress.
#[test]
#[ignore = "Phase 1: M20 canonical formatter (AM-17.2 quarantine)"]
fn lim_fmt_interrupted_mid_write_leaves_no_partial_file() {
    let root = fixture();
    let before_b = bytes(&root, "b.md");
    let error = format_workspace_with_failure_after(&root, Some(1))
        .expect_err("the injected interruption must fail");
    assert!(
        error
            .to_string()
            .contains("injected formatter interruption")
    );
    assert_ne!(
        bytes(&root, "a.md"),
        b"#!liminal-explicit-v1\nnode a { literal \"alpha\"; }\n"
    );
    assert_eq!(bytes(&root, "b.md"), before_b);
    assert!(
        liminal_source::scan_staged(&root)
            .expect("scan staged")
            .is_empty()
    );
}

/// Rerunning after partial failure completes remaining files and a third run
/// is byte-idempotent.
#[test]
#[ignore = "Phase 1: M20 canonical formatter (AM-17.2 quarantine)"]
fn lim_fmt_rerun_after_interruption_is_idempotent() {
    let root = fixture();
    let _ = format_workspace_with_failure_after(&root, Some(1));
    format_workspace(&root).expect("rerun must complete");
    let a = bytes(&root, "a.md");
    let b = bytes(&root, "b.md");
    let outcome = format_workspace(&root).expect("third run");
    assert_eq!(outcome.changed_files, 0);
    assert_eq!(bytes(&root, "a.md"), a);
    assert_eq!(bytes(&root, "b.md"), b);
}

/// Malformed syntax remains recoverable and does not cause data loss during
/// formatting.
#[test]
#[ignore = "Phase 1: M20 canonical formatter (AM-17.2 quarantine)"]
fn lim_fmt_survives_malformed_input_without_data_loss() {
    let root = ScratchDir::new("m20-malformed").expect("scratch root");
    let source = "#!liminal-explicit-v1\nnode broken { literal \"unterminated;\n";
    fs::write(root.join("broken.md"), source).expect("source");
    format_workspace(&root).expect("malformed source must remain format-safe");
    let formatted = bytes(&root, "broken.md");
    assert!(!formatted.is_empty());
    let reparsed =
        MarkdownFormatter::default().parse(std::str::from_utf8(&formatted).expect("UTF-8"));
    assert!(reparsed.is_ok(), "formatted malformed source must reparse");
}
