//! M20 exit-gate tests: the canonical formatter.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs`.
//!
//! ## Three declared tests are deliberately absent
//!
//! The packet also names `lim_fmt_interrupted_mid_write_leaves_no_partial_file`,
//! `lim_fmt_rerun_after_interruption_is_idempotent` and
//! `lim_fmt_survives_malformed_input_without_data_loss`. All three are about the
//! `lim fmt` COMMAND, and no such command exists — `crates/liminal-cli/src/cmd/`
//! has check, jurisdiction, overlays, repair_undo and repairs, and nothing else.
//!
//! They are not written here, and writing them would have been the worse
//! choice. A test whose subject does not exist can only be a stub that fails
//! unconditionally, and an unconditionally-failing test is a *vacuity vector*
//! under F-28's new gate: once un-ignored it would "witness" the kill of any
//! mutant that named it, without the mutation having anything to do with the
//! failure. An honest absence is stronger than a dishonest witness.
//!
//! M20 writes them when M20 writes `lim fmt`.

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
