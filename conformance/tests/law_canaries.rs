//! Gate canaries for the §112 constitutional laws (ADR-0020 §3; M17.5 F-01).
//!
//! ADR-0020 requires that "a deliberate violation must make the exact expected
//! gate fail." These are that proof for the three Phase 1 law gates.
//!
//! History: before M17.5 hardening, a formatter whose `parse` returned a
//! constant and whose `format`/`emit` returned `""` — one that DELETES THE
//! WHOLE DOCUMENT — passed `check_formatter_idempotence` and
//! `check_canonical_round_trip`, because every assertion compared the
//! implementation to itself. Each canary below pins one degenerate
//! implementation that the law MUST now reject. A canary that stops panicking
//! means the law went vacuous again.

use std::collections::BTreeMap;

use liminal_format::Formatter;
use liminal_id::TransactionId;
use liminal_query::IncrementalCompiler;
use liminal_revision::{BasisPerspective, WorkspaceBasis};

const SOURCES: [&str; 3] = [
    "alpha {#a}\n\nbeta {#b}",
    "leading blanks\n\nsecond block",
    "literal {#malformed id!}",
];

/// Deletes everything: one constant `Doc`, empty output.
#[derive(Debug, Default)]
struct ContentDeletingFormatter;

#[derive(Debug, PartialEq, Eq)]
struct EmptyDoc;

impl Formatter for ContentDeletingFormatter {
    type Doc = EmptyDoc;
    type Error = std::convert::Infallible;

    fn parse(&self, _source: &str) -> Result<Self::Doc, Self::Error> {
        Ok(EmptyDoc)
    }
    fn emit(&self, _doc: &Self::Doc) -> Result<String, Self::Error> {
        Ok(String::new())
    }
    fn format(&self, _source: &str) -> Result<String, Self::Error> {
        Ok(String::new())
    }
}

/// Preserves bytes (so content survives) but collapses every source to one
/// `Doc` — the round-trip relations still hold vacuously.
#[derive(Debug, Default)]
struct ConstantParseFormatter;

impl Formatter for ConstantParseFormatter {
    type Doc = EmptyDoc;
    type Error = std::convert::Infallible;

    fn parse(&self, _source: &str) -> Result<Self::Doc, Self::Error> {
        Ok(EmptyDoc)
    }
    fn emit(&self, _doc: &Self::Doc) -> Result<String, Self::Error> {
        // Non-empty and stable, so emission looks canonical.
        Ok(SOURCES[0].to_owned())
    }
    fn format(&self, source: &str) -> Result<String, Self::Error> {
        Ok(source.to_owned()) // identity: content survives
    }
}

/// Ignores its input entirely: both compile paths return the same constant.
#[derive(Debug, Default)]
struct ConstantCompiler;

impl IncrementalCompiler for ConstantCompiler {
    type Source = String;
    type Edit = ();
    type Output = u8;

    fn full(&self, _source: &Self::Source, _basis: &WorkspaceBasis) -> Self::Output {
        0
    }
    fn apply(&self, source: &Self::Source, _edits: &[Self::Edit]) -> Self::Source {
        source.clone()
    }
    fn incremental(
        &self,
        _source: &Self::Source,
        _edits: &[Self::Edit],
        _basis: &WorkspaceBasis,
    ) -> Self::Output {
        0
    }
}

fn basis() -> WorkspaceBasis {
    WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    }
}

/// The formatter-idempotence law must reject a content-deleting formatter.
#[test]
#[should_panic(expected = "erased a non-empty document")]
fn canary_idempotence_law_rejects_content_deletion() {
    liminal_conformance::laws::check_formatter_idempotence(&ContentDeletingFormatter, &SOURCES);
}

/// The canonical-round-trip law must reject a content-deleting formatter.
#[test]
#[should_panic(expected = "erased a non-empty document")]
fn canary_round_trip_law_rejects_content_deletion() {
    liminal_conformance::laws::check_canonical_round_trip(&ContentDeletingFormatter, &SOURCES);
}

/// The formatter-idempotence law must reject a constant parser, even when the
/// formatted bytes are preserved perfectly.
#[test]
#[should_panic(expected = "same document")]
fn canary_idempotence_law_rejects_constant_parser() {
    liminal_conformance::laws::check_formatter_idempotence(&ConstantParseFormatter, &SOURCES);
}

/// The incremental law must reject a compiler that ignores its input.
#[test]
#[should_panic(expected = "same output as the")]
fn canary_incremental_law_rejects_input_ignoring_compiler() {
    liminal_conformance::laws::check_incremental_equals_full(
        &ConstantCompiler,
        &"alpha {#a}".to_owned(),
        &[()],
        &basis(),
    );
}

// ── M17.5 pass-2 findings #5, #6, #7 ───────────────────────────────────────
// The Phase 6/7 laws were left unhardened when F-01 fixed the Phase 1 three.
// An independent reviewer (different model family) found all three vacuous.
// These canaries pin the fix the same way.

/// Applies nothing; diffs by echoing the requested transaction.
#[derive(Debug, Default)]
struct NoOpDiffer;

impl liminal_history::SemanticDiff for NoOpDiffer {
    type Graph = u8;
    type Tx = u8;

    fn apply(&self, graph: &Self::Graph, _tx: &Self::Tx) -> Self::Graph {
        *graph // changes nothing
    }
    fn diff(&self, _after: &Self::Graph, _before: &Self::Graph) -> Self::Tx {
        7 // echoes whatever the caller asked about
    }
}

/// "Deterministic" by returning one constant, never consulting the log.
#[derive(Debug, Default)]
struct ConstantResolver;

impl liminal_resolver::ReplayableResolver for ConstantResolver {
    type Request = u8;
    type Observation = u8;

    fn observe(&mut self, _request: &Self::Request) -> Self::Observation {
        0
    }
    fn replay(&self, _request: &Self::Request, _frozen: &[Self::Observation]) -> Self::Observation {
        99 // never appears in any frozen log the caller supplies
    }
}

/// Every replica is identical because nothing ever changes state.
#[derive(Debug, Default)]
struct InertReplica;

impl liminal_sync::Replica for InertReplica {
    type Op = u8;
    type State = ();

    fn apply(&mut self, _op: Self::Op) {}
    fn merge(&mut self, _other: &Self) {}
    fn state(&self) -> Self::State {}
}

/// The semantic-diff law must reject an `apply` that changes nothing.
#[test]
#[should_panic(expected = "unchanged graph")]
fn canary_semantic_diff_law_rejects_inert_apply() {
    liminal_conformance::laws::check_semantic_diff_matches(&NoOpDiffer, &1u8, &7u8);
}

/// The replay law must reject a resolver that invents observations instead of
/// reproducing recorded ones.
#[test]
#[should_panic(expected = "not present in the frozen log")]
fn canary_replay_law_rejects_invented_observations() {
    liminal_conformance::laws::check_resolver_replay_deterministic(
        &ConstantResolver,
        &1u8,
        &[1, 2],
    );
}

/// The convergence law must reject replicas whose state never moves.
#[test]
#[should_panic(expected = "vacuous")]
fn canary_convergence_law_rejects_inert_replicas() {
    liminal_conformance::laws::check_replicas_converge(InertReplica::default, vec![1u8], vec![2u8]);
}
