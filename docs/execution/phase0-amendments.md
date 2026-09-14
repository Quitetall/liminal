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

AM-17.4 (user-ratified 2026-08-08, ADR-0021): stage HAQP-1 around Phase 1
authorization. M17.5's exit criterion becomes **HAQP-1a** — everything ADR-0020
requires except §3's mutant clauses, which defer to **HAQP-1b** at M24.

Rationale (M17.5 finding F-28): §3 requires 64 semantic mutants at a 100% kill
rate, and all 35 distinct killing tests backing the packet's 65 mutants were
unrunnable — 27 did not exist and 8 are `#[ignore]`d under AM-17.2. Lifting the
quarantine needs Phase 1 authorization, which M17.6 grants AFTER M17.5. ADR-0020
§3 assumed the suite it mutates already exists; for Phase 1 it does not.

No threshold is reduced: every §3 clause survives verbatim into 1b, only its
evaluation point moves. §3's gate-canary clause stays in 1a, because canaries
run against the packet's own gates rather than against Phase 1 tests. The packet
records `qualification_stage`, and the verifier rejects a 1a packet claiming
mutant kills as well as a 1b packet without them.

Also reconciles M17.8's predicted counts, which were authored before the suite
grew: it predicted 237 active / 21 backlog; the meter reads 356 active / 43
backlog (Phase 1 deferred rose to 27 as M18–M23's 20 milestone tests were
written and quarantined). The prediction is corrected rather than the meter.

AM-17.5 (user-approved 2026-08-14): HAQP requirement coordinates are exact
anchors, not substring checks; gate rows use complete `## Exit gate` heading
anchors, semantic `Dnn.n` anchors require identifier boundaries, and packet
`kind`, `source`, `critical`, and `stateful` fields must match the closed
authoritative M18–M24 registry in `liminal-xtask`. Blind-review findings may
link only to `verified_defect` attempts; false-positive and caught-violation
attempts remain attempt records without emitted findings, and malformed linkage
blocks the pass. M10's Pandoc pin and golden move to installed 3.10.2.

AM-17.6 (user-ratified 2026-08-16): the qualification metadata child may lift
EXACTLY ONE `#[ignore]` — the one on
`phase1_suite_packet_is_complete_and_unratified` in
`conformance/tests/phase0.rs` — and change nothing else in that file.

Rationale: that gate is M17.5's stated exit criterion and it asserts the packet
is COMPLETE, so it cannot be live before the flip that completes the packet.
Every other placement fails. Un-ignoring at the fixed base commits a red tree
and makes the base look broken to anyone who checks it out. Un-ignoring after
the flip moves HEAD, so `provenance.evidence_parent == HEAD^` no longer holds
and the qualified gate refuses. The metadata child is the only position left.

Discovered by running M17.5's exit command as written: it reports
`0 tests run: 0 passed, 18 skipped`, not the `1 passed` the work order predicts.
The trap was an ordering constraint nobody had written down, in the same family
as F-28 and the first ADR-0021 deadlock — no piece is defective, and the
sequence they compose into is impossible.

No threshold is reduced and the guarantee is not widened to the file.
`qualification_metadata_path` still refuses `conformance/tests/phase0.rs`;
`verify_gate_unignore_only` inspects the DIFF and requires one removed line, no
added lines, and that the removed line is that exact attribute. Gate logic
therefore still cannot change in the child, which is what `verify_provenance`
exists to prevent. Four canaries pin it, including one asserting the pinned
attribute string still matches the tree — otherwise the check passes vacuously
against a file it no longer describes.

AM-24.1 (M24, migration approved 2026-09-07): reconcile M24's opening
prerequisite with accepted ADR-0021. Phase 1 authorization follows HAQP-1a at
M17.6; HAQP-1b runs at M24; final suite ratification follows both stages.
No aggregate test, mutation threshold, quarantine or human authority is changed.
Recorded in the active Phase 0 ledger because M24 remains a proposed gate work
order; final amendment disposition remains part of Phase 0 closeout.

AM-24.2 (M18–M23, migration reconciliation 2026-09-07): apply the same accepted
ADR-0021 prerequisite correction as AM-24.1 to M18–M23, including M18/M19's
duplicated exit-criteria wording. M17.6 authorization after HAQP-1a permits
execution; final suite ratification follows HAQP-1b at M24. This records the
already accepted staging rule, not a new GO, test activation, amendment
ratification, or change to full M19 breadth, assertions, goldens or thresholds.

AM-17.13 (M17): user-approved 2026-09-13, allow only independently verified,
behavior-preserving mutant file/line reference updates and their derived packet
digest mirror under AM-17.11, with a committed before/after audit record per
`docs/execution/haqp-reference-maintenance.md`; ambiguous, missing, rewritten,
or behavior-changing targets and golden changes require a separate decision,
while qualifier logic, mutation identity, coverage, thresholds, historical
evidence, and phase authority remain unchanged.

AM-17.12 (M17): user-approved 2026-09-13, implement the six StoreOwner/read-only
GraphStore interface obligations and migration witnesses recorded in
`verification/authority-amendment-proposal.md`, preserving valid behavior and
existing assertions while requiring checked admission, validated recovery and
scoped durable receipts; full verification waits for the next user checkpoint,
and formal adoption, phase GO and suite ratification remain separate.

AM-17.12 supplement (M17, DG17.3): user-approved 2026-09-13, permit the
narrow generated ILRP probe setup migration documented in M17 without changing
assertions, output bytes, budgets, goldens, coverage or thresholds. Required
verification/local compute is now authorized with explicit resource bounds;
phase authorization and formal adoption remain separate.

AM-17.12 supplement (M17, DG17.4): user-approved 2026-09-14, conditional on
source audit and before/after transaction evidence, add only `same-file` to the
closed concurrency dependency list with its exact existing version pin and
relocate the existing single deferred epoch write from workspace.rs to owner.rs.
Preserve all negative controls, classifications, counts and qualification
criteria; regenerate the derived packet digest. This is not phase authorization.
