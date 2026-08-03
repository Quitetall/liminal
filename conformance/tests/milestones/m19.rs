//! M19 exit-gate tests: HIR lowering and the source map.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs` for why these
//! exist while Phase 1 is unauthorized.

use liminal_hir::{HirDiagnosticCode, HirItemKind, SourceDialect, lower};
use liminal_id::{ContentHash, SourceId};
use liminal_source::{SourceBasis, Utf8HolderView};

fn lowered(source: &str) -> liminal_hir::LoweredHir {
    let bytes = source.as_bytes();
    let basis = SourceBasis {
        source: SourceId::from_name("liminal:m19:fixture"),
        content_hash: ContentHash::of(bytes),
    };
    let held = Utf8HolderView::from_bytes(basis, bytes).expect("fixture must load");
    let cst = liminal_cst::parse(&held);
    lower(&cst, SourceDialect::CompactOrExplicitV1).expect("lowering must be total")
}

/// D19: an unknown explicit form is reported with an EXACT diagnostic, not
/// swallowed and not guessed at.
///
/// Guessing is what makes a frontend unqualifiable: a form silently reinterpreted
/// as something else round-trips fine and means the wrong thing. M17.5's F-09,
/// F-21 and F-22 were all this defect in the emitter; this is the parser's half.
#[test]
#[ignore = "Phase 1: M19 HIR lowering (AM-17.2 quarantine)"]
fn explicit_syntax_rejects_unknown_forms_with_exact_diagnostics() {
    let result = lowered("#!liminal-explicit-v1\nsummon paragraph { literal \"x\"; }\n");
    assert!(
        result.diagnostics.iter().any(|d| matches!(
            d.code,
            HirDiagnosticCode::UnknownForm | HirDiagnosticCode::MalformedSyntax
        )),
        "an unknown form must produce a diagnostic, got {:?}",
        result.diagnostics
    );
    // ...and must not be silently promoted into a form the grammar DOES have.
    assert!(
        !result.document.items.iter().any(|item| matches!(
            &item.kind,
            HirItemKind::NodeConstruction { name } if name == "paragraph"
        )),
        "an unknown form must not be reinterpreted as a known one"
    );
}

/// D19: lowering is a pure function of source and dialect.
///
/// Nondeterminism here would make every downstream golden and every mutation
/// kill unreproducible — a suite that cannot reproduce cannot qualify
/// (ADR-0020 §1).
#[test]
#[ignore = "Phase 1: M19 HIR lowering (AM-17.2 quarantine)"]
fn hir_lowering_is_deterministic_across_runs() {
    let source = concat!(
        "#!liminal-explicit-v1\n",
        "node root (id = \"root\") {\n",
        "  literal \"value\";\n",
        "  attribute color = \"blue\";\n",
        "  reference @root;\n",
        "}\n",
        "relation @root -[links]-> @root;\n",
    );
    let first = lowered(source);
    for run in 1..8 {
        let again = lowered(source);
        assert_eq!(
            first.document, again.document,
            "lowering differed on run {run}"
        );
        assert_eq!(
            first.diagnostics, again.diagnostics,
            "diagnostics differed on run {run}"
        );
    }
}

/// D19: malformed durable markers degrade to diagnostics, never panics.
///
/// These inputs are drawn from the shapes M17.5's fuzz campaigns actually
/// produced — unbalanced quotes beside `=`, `\r` and `,` (F-09), and bare
/// identifiers outside the grammar (F-22).
#[test]
#[ignore = "Phase 1: M19 HIR lowering (AM-17.2 quarantine)"]
fn hir_lowering_survives_malformed_markers_without_panic() {
    let hostile = [
        "#!liminal-explicit-v1\nnode paragraph (a\"=\r,b = 1) { literal \"x\"; }\n",
        "#!liminal-explicit-v1\nnode 9bad { literal \"x\"; }\n",
        "#!liminal-explicit-v1\nrelation @a -[not an ident]-> @b;\n",
        "#!liminal-explicit-v1\nordered {}\n",
        "{#unterminated",
        "",
    ];
    for source in hostile {
        let result = lowered(source);
        // Totality is not enough: a lowering that "survives" by discarding the
        // document has not lowered it. Every item must still address its source.
        for item in &result.document.items {
            assert!(
                item.range.start <= item.range.end,
                "{source:?} produced item {:?} with an inverted range",
                item.id
            );
        }
    }
}

/// D19: every source-map entry names the basis it was lowered from.
///
/// An entry without its basis is a span pointing at "some document", which is
/// exactly what §112's provenance rules forbid.
#[test]
#[ignore = "Phase 1: M19 HIR lowering (AM-17.2 quarantine)"]
fn source_map_entries_carry_their_source_basis() {
    let source = "#!liminal-explicit-v1\nnode paragraph { literal \"x\"; }\n";
    let bytes = source.as_bytes();
    let expected = SourceBasis {
        source: SourceId::from_name("liminal:m19:fixture"),
        content_hash: ContentHash::of(bytes),
    };
    let result = lowered(source);
    assert_eq!(
        result.document.basis, expected,
        "the lowered document must carry the basis it was lowered from"
    );
    // The basis must be the one that HASHES the source, not merely a
    // well-formed basis: a mismatched hash is how a stale document masquerades
    // as a current one.
    assert_eq!(
        result.document.basis.content_hash,
        ContentHash::of(bytes),
        "the recorded content hash must be the hash of the recorded source"
    );
}
