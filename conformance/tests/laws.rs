//! Instantiations of the §112 constitutional laws.
//!
//! The laws themselves are already written, generically, in
//! `liminal_conformance::laws` — un-ignoring a test here means ONLY supplying
//! the first real implementation of the trait seam and calling the law with
//! it. The assertion logic must come from the generic law, never be
//! reimplemented per test.

/// `parse(format(parse(x))) ≡ parse(x)` + formatting idempotence (v4 §112,
/// §20) via `laws::check_formatter_idempotence` over the first real
/// `liminal_format::Formatter` (the Phase 1 Markdown-compatible formatter),
/// fed by `fixtures/malformed-source/` and golden sources.
#[test]
fn formatter_idempotence_law_holds() {
    use liminal_format::MarkdownFormatter;

    let sources = [
        "alpha {#a}\n\nbeta {#b}",
        "leading blanks\n\nsecond block",
        "literal {#malformed id!}",
    ];
    let formatter = MarkdownFormatter::default();
    liminal_conformance::laws::check_formatter_idempotence(&formatter, &sources);
}

/// `parse(emit(graph))` preserves the supported subset; `emit(parse(source))`
/// canonicalizes (v4 §8.4 level 2, §124) via `laws::check_canonical_round_trip`.
#[test]
fn canonical_round_trip_law_holds() {
    use liminal_format::MarkdownFormatter;

    let sources = [
        "alpha {#a}\n\nbeta {#b}",
        "unsuffixed paragraph\n\nthird block {#c}",
    ];
    let formatter = MarkdownFormatter::default();
    liminal_conformance::laws::check_canonical_round_trip(&formatter, &sources);
}

/// `incremental_compile(x, edits) ≡ full_compile(apply(x, edits))` (v4 §112)
/// via `laws::check_incremental_equals_full` over the first
/// `liminal_query::IncrementalCompiler` (Phase 1 vertical slice; the M8
/// revision prototype may instantiate a toy version earlier).
#[test]
fn incremental_equals_full_compile_law_holds() {
    use std::collections::BTreeMap;

    use liminal_id::TransactionId;
    use liminal_query::{ParagraphCompiler, SourceEdit};
    use liminal_revision::{BasisPerspective, WorkspaceBasis};

    let compiler = ParagraphCompiler;
    let source = "alpha {#a}\n\nbeta {#b}".to_owned();
    let edits = [SourceEdit {
        start: 0,
        end: 5,
        replacement: "ALPHA".to_owned(),
    }];
    let basis = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    liminal_conformance::laws::check_incremental_equals_full(&compiler, &source, &edits, &basis);
}

/// `semantic_diff(apply(tx, g), g)` matches `tx` (v4 §112) via
/// `laws::check_semantic_diff_matches` over the first
/// `liminal_history::SemanticDiff`.
#[test]
#[ignore = "Phase 6: semantic diff over the history crate"]
fn semantic_diff_matches_transaction_law_holds() {
    unimplemented!("laws::check_semantic_diff_matches(&differ, &graph, &tx)")
}

/// Resolver replay with frozen inputs is deterministic (v4 §112, §110) via
/// `laws::check_resolver_replay_deterministic` over the first
/// `liminal_resolver::ReplayableResolver` (Phase 7 HTTP/database examples;
/// the M8 effect reactor may instantiate a toy version earlier).
#[test]
#[ignore = "Phase 7: replayable resolver (toy version possible at M8)"]
fn resolver_replay_deterministic_law_holds() {
    unimplemented!("laws::check_resolver_replay_deterministic(&resolver, &request, &frozen)")
}

/// Sync replicas converge under supported operation schedules (v4 §112, §88)
/// via `laws::check_replicas_converge` over the first `liminal_sync::Replica`
/// (Phase 6 — an adapter over an evaluated engine such as Automerge, never a
/// custom CRDT, v4 §118A/§123).
#[test]
#[ignore = "Phase 6: replica adapter over an evaluated merge engine"]
fn replicas_converge_law_holds() {
    unimplemented!("laws::check_replicas_converge(make_replica, ops_a, ops_b)")
}
