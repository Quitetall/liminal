//! TEMPORARY HAQP probe (M17.5 adversarial pass, not a permanent test).
//!
//! Question: do the three flipped Phase 1 law gates actually constrain an
//! implementation, or would a degenerate implementation satisfy them?
//! ADR-0020 §5: "A relation is not proved by calling the implementation's own
//! equality or normalization path on both sides."

/// A deliberately useless formatter: `parse` throws the source away, `format`
/// returns a constant. If the idempotence and canonical-round-trip laws pass
/// against THIS, they constrain nothing.
#[derive(Debug, Default)]
struct DegenerateFormatter;

#[derive(Debug, PartialEq, Eq)]
struct EmptyDoc;

impl liminal_format::Formatter for DegenerateFormatter {
    type Doc = EmptyDoc;
    type Error = std::convert::Infallible;

    fn parse(&self, _source: &str) -> Result<Self::Doc, Self::Error> {
        Ok(EmptyDoc) // every source parses to the same nothing
    }

    fn emit(&self, _doc: &Self::Doc) -> Result<String, Self::Error> {
        Ok(String::new())
    }

    fn format(&self, _source: &str) -> Result<String, Self::Error> {
        Ok(String::new()) // every source formats to the empty string
    }
}

#[test]
fn probe_degenerate_formatter_against_idempotence_law() {
    let sources = [
        "alpha {#a}\n\nbeta {#b}",
        "leading blanks\n\nsecond block",
        "literal {#malformed id!}",
    ];
    liminal_conformance::laws::check_formatter_idempotence(&DegenerateFormatter, &sources);
    // Reaching here means: a formatter that DELETES ALL CONTENT satisfies the
    // formatter-idempotence law. The law is vacuous.
}

#[test]
fn probe_degenerate_formatter_against_canonical_round_trip_law() {
    let sources = [
        "alpha {#a}\n\nbeta {#b}",
        "unsuffixed paragraph\n\nthird block {#c}",
    ];
    liminal_conformance::laws::check_canonical_round_trip(&DegenerateFormatter, &sources);
    // Reaching here means the canonical-round-trip law is vacuous too.
}
