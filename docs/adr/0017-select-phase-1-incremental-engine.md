# 0017. Select Phase 1 incremental engine

- **Status:** proposed — user decision required
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** v4 §§24, 118A, 122; R4 §8; M06; M08; M15

## Context

Phase 1 needs tracked incremental parsing, graph derivation, diagnostics, and
HTML preview. Phase -1 proved explicit Workspace Basis selection,
component-granular invalidation, pure deterministic replay, and effects outside
queries. It did not justify building a production revision engine.

Required boundaries are immutable values, deterministic tracked queries,
explicit inputs, effects outside queries, durability-informed invalidation,
Workspace Basis vectors, demand-driven computation, interned symbols,
diagnostic accumulation, and as-of/history views.

## Decision required

Choose exactly one:

1. **Salsa behind Liminal-owned interfaces.** Use Salsa for tracked dependency
   execution and durability; represent Workspace Basis/freshness as explicit
   inputs; keep history, effect reactor, and Holder selection outside Salsa.
2. **Liminal-specific engine.** Implement dependency graph, revisions,
   durability, cycle handling, diagnostics, and memo invalidation directly.

**Recommendation:** option 1. Salsa retires less novel infrastructure risk while
Liminal-owned interfaces prevent its database shape from becoming semantic
authority. Add differential tests against Phase -1 pure-query replay before
expanding tracked scope.

## Consequences pending acceptance

- Salsa remains an implementation detail, not source of truth or effect host.
- As-of graph history stays in Liminal storage and enters queries as explicit
  immutable inputs.
- Custom engine work requires measured evidence that Salsa violates a mandatory
  boundary.

**What would falsify or reverse this:** a Phase 1 spike demonstrates Salsa cannot
model Basis vectors, deterministic replay, diagnostics, or bounded invalidation
without hidden global state or unacceptable recomputation. Reversal requires a
new ADR and the same differential suite against the replacement.

