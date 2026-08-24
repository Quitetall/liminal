//! M21 exit-gate tests: incremental recompilation.
//!
//! **All `#[ignore]`d under AM-17.2** — see the note in `m18.rs`.
//!
//! The §112 law itself (`incremental(x, edits) == full(apply(x, edits))`) is
//! checked generically by `laws::incremental_equals_full_compile_law_holds`.
//! DG21.1: that law is VACUOUS against the Phase 0 `ParagraphCompiler`, which
//! implements `incremental` as `full(apply(x, edits))` and satisfies it by
//! construction. These tests inherit that vacuity and must be re-examined when
//! subtree reuse lands.

use std::collections::BTreeMap;

use liminal_id::TransactionId;
use liminal_query::{IncrementalCompiler, ParagraphCompiler, SourceEdit};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

fn basis() -> WorkspaceBasis {
    WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    }
}

fn source() -> String {
    "alpha {#a}\n\nbeta {#b}\n\ngamma {#c}".to_owned()
}

/// D21: the same source and edits at the same Basis must compile identically
/// every time.
///
/// Determinism is what makes an incremental compiler auditable at all: a
/// nondeterministic one can satisfy the §112 law on the run that is observed
/// and violate it on the run that matters.
#[test]
#[ignore = "Phase 1: M21 incremental compilation (AM-17.2 quarantine)"]
fn incremental_recompilation_is_deterministic_across_runs() {
    let compiler = ParagraphCompiler;
    let basis = basis();
    let edits = [SourceEdit {
        start: 0,
        end: 5,
        replacement: "ALPHA".to_owned(),
    }];
    let first = compiler.incremental(&source(), &edits, &basis);
    for run in 1..8 {
        assert!(
            compiler.incremental(&source(), &edits, &basis) == first,
            "incremental compilation differed on run {run}"
        );
    }
}

/// D21: an incremental result must be attributable to the Basis that governed
/// it — the same edits at a DIFFERENT basis are a different compilation.
///
/// The check is expressed as agreement with the from-scratch compile at that
/// same basis, because that is the only statement about the output that does
/// not route through the implementation's own bookkeeping (ADR-0020 §5).
#[test]
#[ignore = "Phase 1: M21 incremental compilation (AM-17.2 quarantine)"]
fn incremental_outputs_name_their_governing_basis() {
    let compiler = ParagraphCompiler;
    let edits = [SourceEdit {
        start: 0,
        end: 5,
        replacement: "ALPHA".to_owned(),
    }];
    for _ in 0..4 {
        let governing = basis();
        let incremental = compiler.incremental(&source(), &edits, &governing);
        let full = compiler.full(&compiler.apply(&source(), &edits), &governing);
        assert!(
            incremental == full,
            "the incremental output must equal the full compile AT ITS OWN BASIS"
        );
    }
}

/// D21: edits addressing bytes the source does not have must not silently
/// produce a plausible result.
///
/// Clamping an out-of-range edit is the dangerous behaviour: the caller's stale
/// offset becomes a successful compile of the wrong document. Whatever the
/// compiler does, `incremental` and `full` must agree — a divergence here is a
/// §112 violation that the happy-path law would never sample.
#[test]
#[ignore = "Phase 1: M21 incremental compilation (AM-17.2 quarantine)"]
fn incremental_rejects_edits_against_a_stale_basis() {
    let compiler = ParagraphCompiler;
    let basis = basis();
    let stale = [SourceEdit {
        // Offsets from a longer, older revision of the document.
        start: 9_000,
        end: 9_100,
        replacement: "ghost".to_owned(),
    }];
    let incremental = compiler.incremental(&source(), &stale, &basis);
    let full = compiler.full(&compiler.apply(&source(), &stale), &basis);
    assert!(
        incremental == full,
        "an edit against a stale basis must not make the two paths diverge"
    );
}

/// D21: malformed, overlapping and inverted edit sequences must be total.
///
/// These are the shapes an editor actually emits when its own state is wrong,
/// so the compiler sees them in production, not only under fuzzing.
#[test]
#[ignore = "Phase 1: M21 incremental compilation (AM-17.2 quarantine)"]
fn incremental_survives_malformed_edit_sequences() {
    let compiler = ParagraphCompiler;
    let basis = basis();
    let cases: [Vec<SourceEdit>; 4] = [
        // Inverted range.
        vec![SourceEdit {
            start: 8,
            end: 2,
            replacement: "x".to_owned(),
        }],
        // Overlapping edits.
        vec![
            SourceEdit {
                start: 0,
                end: 6,
                replacement: "one".to_owned(),
            },
            SourceEdit {
                start: 3,
                end: 9,
                replacement: "two".to_owned(),
            },
        ],
        // Empty replacement at a boundary.
        vec![SourceEdit {
            start: 0,
            end: 0,
            replacement: String::new(),
        }],
        // Replacement containing the durable-marker syntax itself.
        vec![SourceEdit {
            start: 0,
            end: 5,
            replacement: "{#injected}".to_owned(),
        }],
    ];
    for (index, edits) in cases.iter().enumerate() {
        let incremental = compiler.incremental(&source(), edits, &basis);
        let full = compiler.full(&compiler.apply(&source(), edits), &basis);
        assert!(
            incremental == full,
            "case {index}: malformed edits made incremental diverge from full"
        );
    }
}
