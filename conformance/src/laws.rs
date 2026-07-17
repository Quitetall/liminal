//! The §112 constitutional laws as GENERIC functions over the trait seams in
//! their §117 home crates. The laws are written once, here, now; every future
//! implementation plugs in and inherits them. Instantiation tests live in
//! `tests/laws.rs`, phase-tagged `#[ignore]`d until each seam has an
//! implementation.

use liminal_format::Formatter;
use liminal_history::SemanticDiff;
use liminal_query::IncrementalCompiler;
use liminal_resolver::ReplayableResolver;
use liminal_revision::WorkspaceBasis;
use liminal_sync::Replica;

/// Law: `parse(format(parse(x))) ≡ parse(x)` and formatting is idempotent
/// (v4 §112, §20).
#[allow(
    clippy::similar_names,
    reason = "formatter/formatted is the clearest vocabulary for this law"
)]
pub fn check_formatter_idempotence<F>(formatter: &F, sources: &[&str])
where
    F: Formatter,
    F::Doc: PartialEq + std::fmt::Debug,
    F::Error: std::fmt::Debug,
{
    for source in sources {
        let parsed = formatter.parse(source).expect("source must parse");
        let formatted = formatter.format(source).expect("source must format");
        let reparsed = formatter.parse(&formatted).expect("formatted must parse");
        assert_eq!(
            reparsed, parsed,
            "parse(format(x)) must equal parse(x) for {source:?}"
        );
        let twice = formatter
            .format(&formatted)
            .expect("formatted must reformat");
        assert_eq!(
            twice, formatted,
            "formatting must be idempotent for {source:?}"
        );
    }
}

/// Law: `parse(emit(graph))` preserves the supported subset and
/// `emit(parse(source))` canonicalizes (v4 §8.4 capability level 2, §124).
pub fn check_canonical_round_trip<F>(formatter: &F, sources: &[&str])
where
    F: Formatter,
    F::Doc: PartialEq + std::fmt::Debug,
    F::Error: std::fmt::Debug,
{
    for source in sources {
        let doc = formatter.parse(source).expect("source must parse");
        let emitted = formatter.emit(&doc).expect("doc must emit");
        let round = formatter.parse(&emitted).expect("emitted must parse");
        assert_eq!(
            round, doc,
            "parse(emit(doc)) must preserve the doc for {source:?}"
        );
        let again = formatter.emit(&round).expect("round must emit");
        assert_eq!(again, emitted, "emission must be canonical for {source:?}");
    }
}

/// Law: `incremental_compile(x, edits) ≡ full_compile(apply(x, edits))` at one
/// Basis (v4 §112).
pub fn check_incremental_equals_full<C>(
    compiler: &C,
    source: &C::Source,
    edits: &[C::Edit],
    basis: &WorkspaceBasis,
) where
    C: IncrementalCompiler,
    C::Output: std::fmt::Debug,
{
    let incremental = compiler.incremental(source, edits, basis);
    let edited = compiler.apply(source, edits);
    let full = compiler.full(&edited, basis);
    assert_eq!(
        incremental, full,
        "incremental(x, edits) must equal full(apply(x, edits)) at one Basis"
    );
}

/// Law: `semantic_diff(apply(tx, g), g)` matches `tx` (v4 §112).
pub fn check_semantic_diff_matches<S>(differ: &S, graph: &S::Graph, tx: &S::Tx)
where
    S: SemanticDiff,
    S::Tx: std::fmt::Debug,
{
    let after = differ.apply(graph, tx);
    let recovered = differ.diff(&after, graph);
    assert_eq!(
        &recovered, tx,
        "semantic_diff(apply(tx, g), g) must match tx"
    );
}

/// Law: resolver replay with frozen inputs is deterministic (v4 §112, §110).
pub fn check_resolver_replay_deterministic<R>(
    resolver: &R,
    request: &R::Request,
    frozen: &[R::Observation],
) where
    R: ReplayableResolver,
    R::Observation: std::fmt::Debug,
{
    let first = resolver.replay(request, frozen);
    let second = resolver.replay(request, frozen);
    assert_eq!(
        first, second,
        "replay from frozen observations must be deterministic"
    );
}

/// Law: replicas converge under supported operation schedules (v4 §112, §88).
///
/// Ops are partitioned between two replicas which then merge in BOTH orders;
/// all four end states must agree.
pub fn check_replicas_converge<R>(make: impl Fn() -> R, ops_a: Vec<R::Op>, ops_b: Vec<R::Op>)
where
    R: Replica,
    R::State: std::fmt::Debug,
{
    let mut a = make();
    for op in ops_a {
        a.apply(op);
    }
    let mut b = make();
    for op in ops_b {
        b.apply(op);
    }

    let mut a_then_b = make();
    a_then_b.merge(&a);
    a_then_b.merge(&b);
    let mut b_then_a = make();
    b_then_a.merge(&b);
    b_then_a.merge(&a);

    assert_eq!(
        a_then_b.state(),
        b_then_a.state(),
        "merge order must not change the converged state"
    );

    a.merge(&b);
    b.merge(&a);
    assert_eq!(a.state(), b.state(), "pairwise merge must converge");
    assert_eq!(a.state(), a_then_b.state(), "all schedules must agree");
}
