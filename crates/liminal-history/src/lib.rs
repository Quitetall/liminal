//! Perfect recall — snapshots plus append-only transaction segments.
//!
//! History uses `snapshot + append-only transaction segments` and supports
//! undo/redo, branching, semantic diff, blame and provenance, time travel,
//! replay, and optional compaction policies; the user may keep full history
//! indefinitely, and compaction must never be mandatory (v4 §87). Every
//! accepted semantic change is a transaction of operations with full
//! provenance (v4 §86).
//!
//! **RESERVED — implementation begins Phase 6** (v4 Part XXII "history,
//! sync"). Until this crate takes over, the toy Phase -1 transaction log —
//! checksummed append-only NDJSON segments plus atomic snapshots per v4 §92 —
//! lives in `liminal-graph::store` (ADR-0007). That toy store is this crate's
//! interpretive reference semantics: its record framing and recovery rules
//! are findings the real history engine inherits.
//!
//! This crate exists now so the constitutional vocabulary has a compiled,
//! greppable home: the [`SemanticDiff`] seam lets the §112 law compile as a
//! generic conformance test today.

/// The semantic-diff seam between graph states and transactions (v4 §87,
/// §112).
///
/// SHAPE PROVISIONAL — exists only so the §112 law compiles as a generic
/// conformance test now; Phase 6 may reshape it freely (real history adds
/// snapshots, segment storage, branching, and blame, v4 §87).
///
/// Law (v4 §112): `semantic_diff(apply(tx, g), g)` matches `tx` — diffing a
/// graph against the result of applying a transaction must recover that
/// transaction. `apply` is pure here (it returns the new graph value rather
/// than mutating): history is a value, in the Datomic sense adopted by
/// v4 §118A (immutable database values, as-of/history queries).
pub trait SemanticDiff {
    /// An immutable graph state.
    type Graph;
    /// A semantic transaction (v4 §86); `PartialEq` — or a semantic
    /// equivalence refined in Phase 6 — so the §112 law is checkable.
    type Tx: PartialEq;

    /// Apply `tx` to `graph`, returning the resulting graph state.
    fn apply(&self, graph: &Self::Graph, tx: &Self::Tx) -> Self::Graph;

    /// Compute the transaction that carries `before` to `after`.
    ///
    /// Must recover `tx` when `after == apply(before, tx)` (v4 §112).
    fn diff(&self, after: &Self::Graph, before: &Self::Graph) -> Self::Tx;
}
