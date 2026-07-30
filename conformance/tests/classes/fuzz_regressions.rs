//! Named regression fixtures recovered from HAQP-1 fuzz campaigns
//! (ADR-0020 §4: "Minimized artifacts become named regression fixtures before
//! the qualification lane is rerun").
//!
//! Each fixture is a byte-exact input that once broke a §112 law. They are
//! replayed here so a fix cannot silently regress, and so the campaign never
//! has to rediscover the same defect.

use camino::Utf8PathBuf;

fn corpus_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpora/regression/phase1/canonical_round_trip")
}

fn fixture(name: &str) -> Vec<u8> {
    let path = corpus_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every `.bin` in the corpus, enumerated rather than listed.
///
/// M17.5: the campaign had left 23 minimized artifacts in `fuzz/artifacts/`
/// while only 2 were promoted, which ADR-0020 §4 forbids ("minimized artifacts
/// become named regression fixtures before the qualification lane is rerun").
/// Promoting them is only half the fix — a hardcoded list means the next
/// promotion is replayed by nobody, so the guard now reads the directory.
fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = corpus_dir();
    let mut found: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .flatten()
        .filter_map(|entry| {
            let path = Utf8PathBuf::from_path_buf(entry.path()).ok()?;
            if path.extension() != Some("bin") {
                return None;
            }
            let name = path.file_name()?.to_owned();
            let bytes = std::fs::read(&path).ok()?;
            Some((name, bytes))
        })
        .collect();
    found.sort();
    assert!(
        found.len() >= 24,
        "the F-09 regression corpus has shrunk to {} fixtures; a corpus that \
         quietly empties reports clean",
        found.len()
    );
    found
}

/// F-09 (M17.5 fuzz campaign, 2026-07-25): `parse(emit(parse(x))) != parse(x)`
/// for an explicit-syntax document whose attribute region contains an
/// unbalanced quote alongside `=`, `\r`, and `,`.
///
/// **FIXED in M17.5** (ADR-0020 §1 required it before the qualification lane
/// could be rerun; the diagnostic half remains M19's).
///
/// Root cause, as diagnosed by the pass-1 review:
///
/// 1. `parse_attributes` accepted ANY text before the `=` as an attribute name.
///    M19's grammar says `assignment = ident, ...`, and `emit_attrs` writes
///    names bare, so a name containing `"` emitted source the parser could not
///    read back. Names outside the grammar are now dropped at parse time.
/// 2. `split_top_level` left `quoted` open forever after an unbalanced quote,
///    swallowing every later delimiter and changing the attribute COUNT across
///    the round trip. An unterminated quote is not a string, so the split now
///    rescans treating quotes as ordinary characters.
///
/// This test replays with `from_utf8_lossy`, NOT `from_utf8`. Both fixtures
/// contain invalid UTF-8 — that is what the fuzzer found — and the previous
/// `let Ok(source) = from_utf8(..) else { continue }` skipped both of them, so
/// the regression guard would have reported success against either fixture no
/// matter what the code did. `fuzz_targets/canonical_round_trip.rs` uses
/// `from_utf8_lossy`; a regression harness that does not replay what the fuzzer
/// ran is not a regression harness.
#[test]
fn f09_attribute_escape_round_trip_holds() {
    use liminal_format::Formatter;

    for (name, bytes) in fixtures() {
        let source = String::from_utf8_lossy(&bytes);
        let formatter = liminal_format::MarkdownFormatter::default();
        let document = formatter
            .parse(&source)
            .unwrap_or_else(|e| panic!("{name}: parser must be total: {e:?}"));
        let emitted = formatter.emit(&document).expect("emit must be total");
        let round = formatter.parse(&emitted).expect("emitted must reparse");
        assert_eq!(
            round, document,
            "{name}: parse(emit(parse(x))) must equal parse(x)"
        );
        // The fuzz target asserts emit stability too; so must the regression.
        assert_eq!(
            formatter.emit(&round).expect("emit must be total"),
            emitted,
            "{name}: emit(parse(emit(x))) must equal emit(x)"
        );
    }
}

/// The fixtures themselves must stay present and non-empty — a regression
/// corpus that quietly empties is worse than none, because the campaign would
/// report clean. This runs unconditionally.
#[test]
fn f09_regression_fixtures_are_present_and_non_empty() {
    // The two originally-named fixtures must keep their names; the rest are
    // enumerated.
    for name in ["f09-minimized.bin", "f09-attribute-escape-round-trip.bin"] {
        assert!(!fixture(name).is_empty(), "{name} is empty");
    }
    for (name, bytes) in fixtures() {
        assert!(!bytes.is_empty(), "{name} is empty");
        assert!(
            bytes.starts_with(b"#!liminal-explicit-v1"),
            "{name} no longer carries the explicit-syntax marker it was minimized from"
        );
    }
}
