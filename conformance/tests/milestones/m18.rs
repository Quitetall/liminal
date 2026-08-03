//! M18 exit-gate tests: the lossless CST frontend over a declared Holder view.
//!
//! **All `#[ignore]`d under AM-17.2.** M18 is authored but unauthorized, and
//! ADR-0020 §1 plus finding F-02 make pre-greening a Phase 1 exit gate the
//! specific failure this milestone exists to prevent. They are written now
//! because F-28 found that 27 of the packet's 35 declared killing tests did not
//! exist at all, which left the mutation requirement unprovable and — worse —
//! unstated. A declared test that exists and is quarantined is honest; a
//! declared test that is a string in a JSON file is not.
//!
//! Each test exercises the real surface, so un-ignoring one at M18 is a
//! decision about authorization, not a rewrite.

use liminal_id::{ContentHash, SourceId};
use liminal_source::{SourceBasis, Utf8HolderView};

fn view(bytes: &[u8]) -> Utf8HolderView {
    let basis = SourceBasis {
        source: SourceId::from_name("liminal:m18:fixture"),
        content_hash: ContentHash::of(bytes),
    };
    Utf8HolderView::from_bytes(basis, bytes).expect("fixture must load")
}

/// D18: the parser is TOTAL. Truncated and hostile bytes produce diagnostics,
/// never a panic, and never a partial parse that loses source.
///
/// Totality is the property every later law leans on: `check_canonical_round_trip`
/// calls `parse` on fuzzer output and would report a crash as a test failure
/// rather than a diagnostic. A frontend that panics on hostile input cannot be
/// fuzzed, so it cannot be qualified.
#[test]
#[ignore = "Phase 1: M18 CST frontend (AM-17.2 quarantine)"]
fn cst_parses_truncated_and_hostile_bytes_without_panic() {
    let hostile: [&[u8]; 8] = [
        b"",
        b"{#",
        b"{#unterminated",
        b"#!liminal-explicit-v1\nnode {",
        b"node paragraph { literal \"unclosed",
        b"\xef\xbb\xbf{#bom}",
        b"a\r\n\r\n\r\nb",
        b"\t\t\t{#\t}",
    ];
    for bytes in hostile {
        let held = view(bytes);
        let parse = liminal_cst::parse(&held);
        // Losslessness is the totality proof that means something: a parser
        // that "does not panic" by discarding input has not parsed it.
        assert_eq!(
            parse.emit_lossless(),
            held.to_string(),
            "parse must be lossless for {bytes:?}"
        );
    }
}

/// D18: every diagnostic range must address the source basis it was produced
/// from, and must lie inside that source.
///
/// A span that outruns its source cannot be resolved by any later stage, and a
/// span carrying someone else's basis is worse — it points confidently at the
/// wrong document. Both are the class of defect §112's basis rules exist for.
#[test]
#[ignore = "Phase 1: M18 CST frontend (AM-17.2 quarantine)"]
fn cst_spans_resolve_against_the_declared_source_basis() {
    let source = b"alpha {#a}\n\nbeta {#malformed id!}\n";
    let held = view(source);
    let parse = liminal_cst::parse(&held);

    assert_eq!(
        parse.basis(),
        held.basis(),
        "the parse must carry the basis it was produced from"
    );
    let len = u64::try_from(source.len()).expect("fixture fits u64");
    for error in parse.errors() {
        assert!(
            error.range.start <= error.range.end,
            "{error:?} has an inverted range"
        );
        assert!(
            error.range.end <= len,
            "{error:?} addresses past the end of its own source ({len} bytes)"
        );
        // The range must be resolvable, not merely numerically plausible.
        let start = usize::try_from(error.range.start).expect("fits usize");
        let end = usize::try_from(error.range.end).expect("fits usize");
        held.byte_slice(start..end)
            .unwrap_or_else(|e| panic!("{error:?} does not resolve against its basis: {e:?}"));
    }
}

/// D18: a slice outside the view is REFUSED, not clamped.
///
/// Clamping is the dangerous failure: it returns a real-looking slice for a
/// range that was wrong, so the caller's bug becomes silent data corruption
/// instead of an error. The same argument applies at both ends and to an
/// inverted range, so all three are pinned.
#[test]
#[ignore = "Phase 1: M18 CST frontend (AM-17.2 quarantine)"]
fn rope_edit_outside_bounds_is_rejected() {
    let source = b"alpha beta";
    let held = view(source);
    let len = held.len_bytes();

    held.byte_slice(0..len)
        .expect("the whole view is in bounds");

    held.byte_slice(0..len + 1)
        .expect_err("a slice past the end must be refused, never clamped");
    held.byte_slice(len + 1..len + 2)
        .expect_err("a slice starting past the end must be refused");
    let (high, low) = (4, 2);
    held.byte_slice(high..low)
        .expect_err("an inverted range must be refused");

    // Refusal must not depend on the range being far outside: one byte past is
    // the case a clamping implementation gets wrong.
    assert!(
        held.byte_slice(len..len).is_ok(),
        "an empty slice at the end is in bounds"
    );
}
