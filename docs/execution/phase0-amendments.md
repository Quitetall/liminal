# Phase 0 execution amendments

**Status:** open ledger. Created at the M12 gate; closed by the M17 Phase 0
amendment-disposition ADR.

Each entry is `AM-<milestone>.<number> (M<nn>): <one sentence>`. Entries are
append-only while Phase 0 is active. M17 ratifies or reverts every entry in one
immutable ADR; absence of entries is recorded explicitly there.

AM-17.1 (M17): Phase 1 suite eligibility gains the HAQP-1 qualification profile
from accepted ADR-0020: bidirectional traceability, 64 semantic mutants at 100%
applicable kill rate, gate canaries, five deterministic generated/fuzz families,
independent oracles, exhaustive registered crash boundaries, and blinded two-pass
adversarial review; M17's five gate names and predicted counts remain unchanged.

AM-17.2 (M17): the Phase 1 crates implemented during M17 (`liminal-cst`,
`liminal-cir`, `liminal-hir`, `liminal-format`, and the `liminal-query`
`ParagraphCompiler`) are declared **HAQP qualification substrate**, not
executed Phase 1 work. HAQP-1 cannot mutate, fuzz, or oracle-check code that
does not exist, so building them under M17 was necessary; but their existence
must not satisfy Phase 1 exit gates that M18-M24 have not executed. The seven
Phase 1 exit-gate tests (`formatter_idempotence_law_holds`,
`canonical_round_trip_law_holds`, `incremental_equals_full_compile_law_holds`,
`malformed_source_never_panics_and_round_trips`, `fuzz_regressions_stay_fixed`,
`full_document_html_matches_golden`, `incremental_patch_equals_full_render`)
are therefore returned to their original `#[ignore = "Phase 1: ..."]` tags
verbatim, to be un-ignored by the milestone that earns each one. Substrate
code is retained, not reverted. Resolves M17.5 finding F-02; restores M17.8's
predicted meter (backlog 23 now, 21 after the two Phase 0 gates flip).

AM-17.3 (M17): the `liminal_format::Formatter` seam gains
`emit_in_dialect(&self, doc, dialect) -> Result<String, Self::Error>` with a
default implementation delegating to `emit`, so no existing implementor changes
behavior. `emit` remains the canonical projection (always explicit surface) and
was NOT mutated. Rationale: `format()` is `emit(parse(x))`, so a compact
Markdown document was lowered to `#!liminal-explicit-v1` on every format — M19
requires the canonical round-trip law "across compact and explicit", and M20's
`lim fmt` must not rewrite a reader's chosen surface on save. `MarkdownFormatter`
implements the compact surface for the compact-representable subset (a
`paragraph` node whose only child is a literal, carrying at most an `id`) and
falls back to the explicit surface for anything richer, never emitting a lossy
approximation. Addition over mutation, per Brian's direction 2026-07-20.
