//! The §112 constitutional laws as GENERIC functions over the trait seams in
//! their §117 home crates. The laws are written once, here, now; every future
//! implementation plugs in and inherits them. Instantiation tests live in
//! `tests/laws.rs`, phase-tagged `#[ignore]`d until each seam has an
//! implementation.

use std::collections::{BTreeMap, BTreeSet};

use liminal_format::Formatter;
use liminal_history::SemanticDiff;
use liminal_query::IncrementalCompiler;
use liminal_resolver::ReplayableResolver;
use liminal_revision::WorkspaceBasis;
use liminal_sync::Replica;

/// INDEPENDENT content oracle (ADR-0020 §5; M17.5 finding F-01).
///
/// Implemented HERE, inside the law, over RAW text — it never calls the
/// implementation's parser, canonicalizer, or equality. Returns the multiset
/// of word tokens and the set of well-formed `{#id}` durable markers.
///
/// This exists because self-referential relations (`parse(format(x)) ==
/// parse(x)`) are satisfied by a formatter that deletes all content and a
/// parser that returns a constant. An anchor outside the implementation is the
/// only thing that makes those relations mean anything.
fn content_witness(text: &str) -> (BTreeMap<String, usize>, BTreeSet<String>) {
    let mut words: BTreeMap<String, usize> = BTreeMap::new();
    for token in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if !token.is_empty() {
            *words.entry(token.to_owned()).or_default() += 1;
        }
    }

    let mut ids = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find("{#") {
        let after = &rest[open + 2..];
        let Some(close) = after.find('}') else { break };
        let candidate = &after[..close];
        if !candidate.is_empty()
            && candidate
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            ids.insert(candidate.to_owned());
        }
        rest = &after[close + 1..];
    }
    (words, ids)
}

/// Assert that `output` still carries every word token and every durable id
/// present in `input`, and that a non-empty input did not become empty.
/// Content-destroying implementations die here.
fn assert_content_survives(input: &str, output: &str, what: &str) {
    assert!(
        input.trim().is_empty() || !output.trim().is_empty(),
        "{what} erased a non-empty document: input {input:?} produced {output:?}"
    );
    let (in_words, in_ids) = content_witness(input);
    let (out_words, out_ids) = content_witness(output);
    for (word, count) in &in_words {
        let seen = out_words.get(word).copied().unwrap_or(0);
        assert!(
            seen >= *count,
            "{what} dropped content: token {word:?} appears {count}x in input \
             {input:?} but {seen}x in output {output:?}"
        );
    }
    // Surface-agnostic id survival: a durable id may legitimately change
    // SPELLING between surfaces (`{#a}` in the compact syntax becomes
    // `(id = "a")` in the explicit one), but its VALUE must survive somewhere
    // in the output. Checking the value as a token keeps this oracle
    // independent of the implementation while permitting the declared
    // surface transform.
    for id in &in_ids {
        assert!(
            out_words.contains_key(id) || out_ids.contains(id),
            "{what} dropped durable id {id:?}: input {input:?} produced {output:?}"
        );
    }

    assert_order_survives(input, output, &in_ids, what);
}

/// Every input token must appear in the output IN THE SAME RELATIVE ORDER.
///
/// M17.5 pass-2 #8: `assert_content_survives` compared word MULTISETS, so
/// source `"a,b"` against output `"b a"` satisfied it — the tokens survived
/// while their order did not. An implementation that emitted every word of the
/// document in sorted order passed every content check this oracle made, and a
/// formatter that scrambles content is exactly the defect these laws exist to
/// catch.
///
/// Order is checked as a SUBSEQUENCE rather than an equality, and that
/// asymmetry is deliberate: a declared surface transform may INSERT tokens
/// (`hello {#a}` becomes `hello (id = "a")`, adding `id`), but no legitimate
/// formatting or emission reorders the author's content. Insertions pass,
/// permutations do not.
///
/// **Durable ids are excluded from the sequence**, and that exclusion is
/// load-bearing rather than a convenience. The explicit surface hoists an id
/// into its node's HEADER, so `alpha {#a}` emits as
/// `node paragraph (id = "a") { literal "alpha"; }` — the id legitimately moves
/// ahead of the text it labels. Ids are position-mobile by declared transform;
/// the author's content is not. Id survival is asserted separately by value in
/// `assert_content_survives`, so nothing goes unchecked. (A content word that
/// happens to equal an id value is also skipped, which only weakens the check
/// and never produces a false failure.)
///
/// Punctuation is still not compared, and that is not an oversight: rewriting
/// punctuation is precisely what the compact-to-explicit surface transform
/// does, so an oracle that pinned it would forbid the implementation's declared
/// behaviour rather than test it.
fn assert_order_survives(input: &str, output: &str, ids: &BTreeSet<String>, what: &str) {
    let tokens = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|token| !token.is_empty() && !ids.contains(*token))
            .map(ToOwned::to_owned)
            .collect()
    };
    let input_tokens = tokens(input);
    let output_tokens = tokens(output);

    let mut cursor = output_tokens.iter();
    for (index, token) in input_tokens.iter().enumerate() {
        assert!(
            cursor.any(|candidate| candidate == token),
            "{what} reordered content: input token {token:?} (#{index}) does not appear \
             after its predecessors in the output. Input {input:?} produced {output:?}; \
             input order {input_tokens:?}, output order {output_tokens:?}"
        );
    }
}

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

        // Anchor outside the implementation (F-01): the relations above are
        // self-referential and a content-deleting formatter satisfies them.
        assert_content_survives(source, &formatted, "format");
    }

    // Non-degeneracy of `parse` (F-01): a parser returning one constant makes
    // every round-trip relation vacuously true. Callers supply semantically
    // distinct sources, so at least two parses must differ.
    assert_parse_discriminates(formatter, sources);
}

/// A parser that maps every input to the same `Doc` satisfies every
/// round-trip law vacuously. Requires at least two of the declared sources to
/// parse differently. Callers MUST pass semantically distinct sources.
fn assert_parse_discriminates<F>(formatter: &F, sources: &[&str])
where
    F: Formatter,
    F::Doc: PartialEq + std::fmt::Debug,
    F::Error: std::fmt::Debug,
{
    if sources.len() < 2 {
        return;
    }
    let docs: Vec<F::Doc> = sources
        .iter()
        .map(|s| formatter.parse(s).expect("source must parse"))
        .collect();
    assert!(
        docs.windows(2).any(|pair| pair[0] != pair[1]),
        "parse maps every declared source to the same document — a constant \
         parser satisfies the round-trip relations vacuously; sources: {sources:?}"
    );
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

        // Anchor outside the implementation (F-01): emit must carry the
        // source's content, not merely agree with its own parse.
        assert_content_survives(source, &emitted, "emit");
    }

    assert_parse_discriminates(formatter, sources);
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

    // Non-degeneracy (F-01): a compiler returning one constant from both paths
    // satisfies the equality above vacuously. Callers MUST pass edits that
    // observably change the compilation — a law fed no-op edits proves nothing.
    let baseline = compiler.full(source, basis);
    assert!(
        edits.is_empty() || full != baseline,
        "compiling the edited source produced the same output as the \
         unedited source — either the declared edits are not observable or \
         the compiler ignores its input; both make this law vacuous"
    );
}

/// Law: `semantic_diff(apply(tx, g), g)` matches `tx` (v4 §112).
pub fn check_semantic_diff_matches<S>(differ: &S, graph: &S::Graph, tx: &S::Tx)
where
    S: SemanticDiff,
    S::Tx: std::fmt::Debug,
    S::Graph: PartialEq + std::fmt::Debug,
{
    let after = differ.apply(graph, tx);

    // Non-degeneracy (M17.5 pass-2 #5): `diff(apply(g, tx), g) == tx` is
    // satisfied by an `apply` that changes nothing and a `diff` that echoes
    // its argument. An observable transaction MUST move the graph, or the
    // law is comparing the implementation to itself.
    assert!(
        &after != graph,
        "apply(graph, tx) returned an unchanged graph — either the declared \
         transaction is not observable or apply ignores it; both make this \
         law vacuous"
    );

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

    // Non-degeneracy (M17.5 pass-2 #6): a resolver returning one constant is
    // trivially "deterministic". Replay must REPRODUCE A RECORDED
    // observation — v4 §112: replay reproduces what was recorded, or it is
    // not a replay. Checked against the caller's frozen log, which the
    // resolver did not construct.
    assert!(
        frozen.contains(&first),
        "replay returned {first:?}, which is not present in the frozen log — \
         a replay must reproduce a recorded observation, not invent one"
    );
}

/// Law: replicas converge under supported operation schedules (v4 §112, §88).
///
/// Ops are partitioned between two replicas which then merge in BOTH orders;
/// all four end states must agree.
pub fn check_replicas_converge<R>(make: impl Fn() -> R, ops_a: Vec<R::Op>, ops_b: Vec<R::Op>)
where
    R: Replica,
    R::State: PartialEq + std::fmt::Debug,
{
    let mut a = make();
    for op in ops_a {
        a.apply(op);
    }
    let mut b = make();
    for op in ops_b {
        b.apply(op);
    }

    // Non-degeneracy (M17.5 pass-2 #7): replicas whose `apply` is a no-op and
    // whose `state()` is constant satisfy every convergence assertion below.
    // At least one side must have OBSERVABLY moved from the empty replica.
    let empty = make();
    assert!(
        a.state() != empty.state() || b.state() != empty.state(),
        "neither operation sequence changed replica state — convergence is \
         vacuous when every replica is identical"
    );

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
