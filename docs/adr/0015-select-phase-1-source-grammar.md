# 0015. Select Phase 1 source grammar

- **Status:** proposed — user decision required
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** v4 §§15–20, 119; M09; M13; M15; M17

## Context

Phase 1 needs a lossless error-tolerant CST for the external-file to HTML
vertical slice. Phase -1 proved canonical round-trip only for a toy annotated
subset at Level 2; it did not choose production syntax. v4 §119 reserves three
alternatives and recommends starting Markdown-compatible while retaining a fully
explicit debug form.

All alternatives must preserve malformed bytes, lower to one explicit semantic
core, keep identity claims bounded by ADR-0009, and satisfy M17's independent
full-reparse oracle before capability can rise.

## Decision required

Choose exactly one:

1. **Strict CommonMark-compatible superset.** Lowest adoption friction; hardest
   compatibility constraint when explicit graph constructs outgrow Markdown.
2. **Separate native `.lim` grammar.** Clean semantic surface; highest adoption,
   migration, and dual-tooling cost.
3. **Dual frontend with shared explicit core.** Constrained Markdown-compatible
   frontend plus explicit native/debug form, both lowering to the same L1/L2
   semantics. More frontend work; strongest separation between ergonomic sugar
   and complete representation.

**Recommendation:** option 3. Begin Phase 1 with the constrained
Markdown-compatible frontend as the only product path; keep the explicit form a
debug/conformance target until real requirements justify user-facing `.lim`.

## Consequences pending acceptance

- Parser substrate and M17 real-CST spike follow the selected frontend.
- Every frontend shares projection laws and explicit semantic core.
- No parser implementation begins while this ADR is proposed.

**What would falsify or reverse this:** M17 cannot preserve malformed bytes or
meet canonical round-trip laws; two frontends drift semantically; or measured
maintenance cost exceeds the value of the explicit core. Reversal requires a new
ADR and migration fixtures.

