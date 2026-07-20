# 0019. Dispose Phase 0 prior-art commitments

- **Status:** accepted
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** v4 §118A; §§119–125; M07–M10; M15; M16

## Context

v4 §118A names six prior-art inputs and requires Liminal to adopt strong
boundaries rather than clone whole systems. Phase -1 measured identity,
projection, revision, and adapter behavior. Phase 0 must record a falsifiable
disposition for every named reference without turning references into mandatory
dependencies.

## Disposition matrix

| Reference | Disposition | Boundary retained | Evidence | Falsifier |
|---|---|---|---|---|
| Salsa | Adopt | pure tracked queries, external input mutation, revisioned dependencies, durability-informed invalidation | M08 deterministic replay and effect-reactor separation; ADR-0017 | Phase 1 spike cannot express Workspace Basis, diagnostics, or bounded invalidation without hidden global state |
| Automerge and Peritext | Defer | experimental references for stable positions, marks, and intent-preserving rich-text merge | M09 rich-edit spike stayed below merge-capable Level 4 | Phase 1 requires collaborative merge earlier, or a bounded experiment shows direct reuse changes Phase 1 architecture |
| AtJSON | Adapt | raw content separated from annotations; durable offsets replaced by revision-aware anchors | M09 annotation spike and ADR-0010 Level 2 ceiling | real-CST measurement shows separated annotations cannot preserve required boundaries or exact-source behavior |
| Pandoc | Adapt | interoperability adapter only; never canonical graph | M10 measured Level 1 loss report and ADR-0010 | adapter cannot preserve declared subset/loss, or a backend-specific direct implementation proves safer and simpler |
| Unison | Adapt | content-addressed immutable versions separated from names and mutable entity lineage | M07 identity matrix and ADR-0009 | production identity evidence shows content identity and entity lineage cannot remain separate without ambiguity or unacceptable cost |
| Datomic | Adapt | immutable values, transaction basis, as-of/history, repository-versus-checkout model extended to Basis vectors | M06 anti-chimera and M08 as-of replay | mixed local/external inputs cannot be reproduced from a vector Basis, or history model violates Holder semantics |

## Decision

Accept all six dispositions as architectural boundaries. Direct dependency use
still requires its own reversible ADR where it affects implementation. In
particular, ADR-0017 separately governs Salsa use; this ADR does not itself
authorize direct dependency use.

## Consequences

- Prior art constrains conformance boundaries without becoming semantic
  authority.
- Deferred collaboration work cannot leak into Phase 1.
- Every disposition has evidence and a kill condition.

**What would falsify or reverse this:** any row's falsifier occurs, a named
§118A reference is omitted, or direct dependency evidence contradicts its
boundary. Reversal appends a new ADR and preserves this record.
