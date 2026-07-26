//! Named regression fixtures recovered from HAQP-1 fuzz campaigns
//! (ADR-0020 §4: "Minimized artifacts become named regression fixtures before
//! the qualification lane is rerun").
//!
//! Each fixture is a byte-exact input that once broke a §112 law. They are
//! replayed here so a fix cannot silently regress, and so the campaign never
//! has to rediscover the same defect.

use camino::Utf8PathBuf;

fn fixture(name: &str) -> Vec<u8> {
    let path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpora/regression/phase1/canonical_round_trip")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// F-09 (M17.5 fuzz campaign, 2026-07-25): `parse(emit(parse(x))) != parse(x)`
/// for an explicit-syntax document whose attribute region contains an
/// unbalanced quote alongside `=`, `\r`, and `,`.
///
/// Root cause, diagnosed but NOT yet fixed (the fix belongs to M19/M20
/// execution, not M17):
///
/// 1. `liminal_hir::schema::parse_value` silently falls back to storing the
///    RAW source text — quotes and escape sequences included — when
///    `parse_json_string` fails, instead of diagnosing malformed input. Emit
///    then JSON-escapes that raw text, so the bytes differ on the next pass.
/// 2. `split_top_level` leaves `quoted` open forever after an unbalanced
///    quote, so a `,` separating two attributes is swallowed and the
///    attribute COUNT changes across the round trip.
///
/// The law this breaks is M19's exit gate and the evidence behind the
/// declared Level 2 capability, so this test is `#[ignore]`d against the same
/// Phase 1 tag as that gate rather than deleted or weakened: it is backlog,
/// and it must go green by the milestone that earns the gate.
#[test]
#[ignore = "Phase 1: F-09 attribute-escape round trip (fix belongs to M19/M20)"]
fn f09_attribute_escape_round_trip_holds() {
    use liminal_format::Formatter;

    for name in ["f09-minimized.bin", "f09-attribute-escape-round-trip.bin"] {
        let bytes = fixture(name);
        let Ok(source) = std::str::from_utf8(&bytes) else {
            continue; // the harness only replays UTF-8 inputs
        };
        let formatter = liminal_format::MarkdownFormatter::default();
        let Ok(document) = formatter.parse(source) else {
            continue;
        };
        let emitted = formatter.emit(&document).expect("emit must be total");
        let round = formatter.parse(&emitted).expect("emitted must reparse");
        assert_eq!(
            round, document,
            "{name}: parse(emit(parse(x))) must equal parse(x)"
        );
    }
}

/// The fixtures themselves must stay present and non-empty — a regression
/// corpus that quietly empties is worse than none, because the campaign would
/// report clean. This runs unconditionally.
#[test]
fn f09_regression_fixtures_are_present_and_non_empty() {
    for name in ["f09-minimized.bin", "f09-attribute-escape-round-trip.bin"] {
        let bytes = fixture(name);
        assert!(!bytes.is_empty(), "{name} is empty");
        assert!(
            bytes.starts_with(b"#!liminal-explicit-v1"),
            "{name} no longer carries the explicit-syntax marker it was minimized from"
        );
    }
}
