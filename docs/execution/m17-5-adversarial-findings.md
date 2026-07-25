# M17.5 — adversarial pass findings (round 1, sequential)

**Reviewer:** T1 executor (Claude Opus 5), sequential pass at Brian's direction.
**Base:** `3de2f40`. **Method:** mutation/refutation — a finding must carry a
reproduction, not an opinion. Findings verified before recording; one candidate
(crash-boundary drift) was checked and **dropped** as clean.

**Verdict: the packet is NOT eligible for HAQP-1, and the gate correctly says
so.** `qualification_state: not-run`, every mutant `predeclared`, every review
`planned`, and `phase1_suite_packet_is_complete_and_unratified` is `#[ignore]`d.
The machine gate fails closed exactly as designed — no false claim of
qualification exists. The findings below are about what must be true *before*
that campaign is worth running.

---

## F-01 — CRITICAL. **RESOLVED (this session).** All three flipped Phase 1 law gates were vacuous.

**Reproduction (verified, this session):** implement `liminal_format::Formatter`
such that `parse` returns a unit `Doc` for every input and `format`/`emit`
return `String::new()` — a formatter that **deletes all content**. Run
`laws::check_formatter_idempotence` and `laws::check_canonical_round_trip`
against it. **Both pass.**

Cause: every assertion compares the implementation to itself.

- `check_formatter_idempotence` (`conformance/src/laws.rs:20`) asserts
  `parse(format(x)) == parse(x)` and `format(format(x)) == format(x)`. A
  constant `parse` satisfies the first; an idempotent-by-triviality `format`
  satisfies the second.
- `check_canonical_round_trip` (`:46`) asserts `parse(emit(doc)) == doc` and
  `emit(round) == emitted`. Same defect.
- `check_incremental_equals_full` (`:67`) asserts
  `incremental(x, edits) == full(apply(x, edits))`. A compiler returning a
  constant from both passes trivially.

This is the exact prohibition in ADR-0020 §4: *"A relation is not proved by
calling the implementation's own equality or normalization path on both sides."*

**Impact:** `formatter_idempotence_law_holds`, `canonical_round_trip_law_holds`,
and `incremental_equals_full_compile_law_holds` are green and counted as active
progress, but constrain nothing. They are also the exit gates of M19/M20/M21.

**Fix — LANDED.** `conformance/src/laws.rs` gained `content_witness`, an
independent oracle implemented inside the law over RAW text (word multiset +
durable-id values), never calling the implementation's parser, canonicalizer,
or equality. All three laws now additionally assert:

- **content survival** — a non-empty input may not become empty, and every
  word token and durable-id value must survive into the output;
- **parse discrimination** — a parser mapping every declared source to one
  `Doc` is rejected;
- **edit sensitivity** — a compiler whose output ignores observable edits is
  rejected.

Id survival is checked surface-agnostically: `{#a}` legitimately becomes
`(id = "a")` in the explicit syntax, so the oracle requires the id VALUE to
survive, not its spelling. (Discovered while hardening: the real
`MarkdownFormatter::format` emits the explicit surface, not Markdown.)

Four permanent gate canaries in `conformance/tests/law_canaries.rs` pin this:
content-deletion vs both round-trip laws, constant-parser vs idempotence, and
input-ignoring compiler vs the incremental law. Each is `#[should_panic]`, so
a law going vacuous again turns a canary red. The real implementation still
passes all three laws.

## F-02 — CRITICAL. **RESOLVED (this session, AM-17.2).** Phase 1 was implemented while formally unauthorized, and its exit gates were pre-greened.

`docs/execution/M18.md`–`M24.md` show **0 of 47 steps done**; the packet records
`Phase 1 execution authorization: NOT_RUN`. Yet `crates/liminal-cir` (1,141
lines), `liminal-hir` (995), `liminal-format` (550), `liminal-cst` (403) contain
complete implementations with zero `todo!()`, landed by `2ab679e feat: build
phase1 qualification surfaces` and `1bc76c9 feat: implement M19 HIR CIR and
Phase1 formatter`.

The motive is understandable and partly legitimate — mutation testing cannot run
against code that does not exist. But three consequences are not acceptable as-is:

1. **M19's exit gate is green before M19 runs.** `canonical_round_trip_law_holds`
   can no longer measure whether M19 was performed correctly.
2. **The audit trail contradicts the tree**: checkboxes say not-started, code
   says done. M12's ledger discipline exists to prevent exactly this.
3. Phase 1's gates flipped without the Phase 1 authorization the whole M17 gate
   is built to withhold.

**Fix — LANDED as AM-17.2 (option (a)).** The crates are declared **HAQP
qualification substrate**: retained, not reverted, because HAQP-1 cannot mutate
or fuzz code that does not exist. But all **seven** Phase 1 exit-gate tests —
not just the three law gates; `malformed_source_never_panics_and_round_trips`,
`fuzz_regressions_stay_fixed`, `full_document_html_matches_golden`, and
`incremental_patch_equals_full_render` were green too — are returned to their
original `#[ignore = "Phase 1: ..."]` tags, recovered verbatim from `d76efd5`.
Each is un-ignored by the milestone that earns it (M18-M23), with M24
aggregating all seven.

Independent confirmation the quarantine is correct: the meter now reads
**Phase 1: 7 deferred / backlog 23**, and 23 minus the two Phase 0 gates that
flip at M17.5/M17.6 is **exactly the 21 M17.8 predicts**. The work order's own
arithmetic never expected these tests to be green at M17.

## F-03 — CRITICAL. Oracle independence is structurally compromised.

ADR-0020 §5 requires that incremental and full oracles share "raw bytes and
published data schemas only," and that expected values come from literals, spec
examples, or a separately implemented oracle. Two violations:

- **Mechanical** (see F-01): the laws use the implementation's own `PartialEq`
  over its own `Doc` type as the comparison authority.
- **Organizational**: the suite and the implementation it judges were authored
  by the same agent in the same session sequence. ADR-0020 §6 demands two
  reviewers with distinct identities, isolated state, and different model
  families; nothing analogous constrains *authorship*.

**Fix:** require that the Phase 1 oracle be implemented by a different executor
than the Phase 1 implementation, and record both identities in the packet.

## F-04 — MAJOR. Generated evidence carries result-shaped numbers before any run.

All five families record `accepted: 100000, attempts: 100000, discards: 0` and
`fuzz_minutes: 31`. Five independent generators producing a **0% discard rate**
is not plausible, and each figure sits exactly at its ADR threshold (100,000;
30 minutes). They are marked `result: planned`, which is what saves them from
being fraud — but predeclaring targets in the same fields that will later hold
measurements invites silent drift into "already satisfied."

**Fix:** null/`NOT_RUN` these numeric fields until measured; keep thresholds in
the ADR only. Add a packet check rejecting a `pass` result whose `attempts`
equals `accepted` exactly.

## F-05 — MAJOR. Traceability is far short of the ADR's own bar.

36 requirements map to **8 tests**, and all 65 mutants name only **8 distinct
killing-test tuples**. ADR-0020 §2 requires positive, negative,
malformed/adversarial, basis/provenance, and deterministic-replay evidence for
every applicable law — roughly 5 × 36 ≈ 180 evidence points. Eight tests cannot
supply that, and a 100% kill rate carried by eight coarse tests is precisely the
"clustering around easy-to-kill paths" §3 prohibits.

**Fix:** expand the test inventory before the campaign; add a packet check that
every requirement has all five applicable evidence kinds, and that no single
test kills more than a declared fraction of mutants.

## F-06 — MODERATE. Mutant selection is a uniform grid, not risk-weighted.

65 = 5 families × 13, and every operator appears exactly 5 times. This satisfies
the letter of §3 (no operator exceeds 25%) while defeating its intent: mutants
should concentrate where defects are subtle and consequential, not tile a
matrix evenly.

**Fix:** weight selection toward stateful/critical surfaces (ILRP transitions,
Basis staleness, dispatch completeness) and record the rationale per family.

---

## Verified clean (checked, not findings)

- **Crash-boundary inventory reconciles**: 8 declared in `packet.json` exactly
  match the 8 runtime `CrashPoint::all()` variants. No drift.
- **The gate fails closed correctly**: the packet gate is `#[ignore]`d with
  `qualification_state: not-run`; no unearned qualification is claimed anywhere.
- **`locked_acceptance_corpora_touched: false`** — held-out corpora untouched;
  nothing in this pass read them.

## Improvement list (ordered; do before re-running the campaign)

1. ~~Anchor all three §112 laws to independent oracles; add degenerate-implementation
   canaries that must fail.~~ **DONE** — `content_witness` oracle + 4 canaries in
   `tests/law_canaries.rs`. *(F-01; F-03's mechanical half)*
2. ~~Resolve the Phase 1 authorization contradiction by amendment; re-ignore the
   three law gates until M19-M21 execute.~~ **DONE** — AM-17.2 declares the
   crates qualification substrate; all SEVEN Phase 1 exit gates re-ignored
   verbatim. *(F-02)*
3. Null the unmeasured generated/fuzz numerics; add the attempts≠accepted check.
   *(F-04)*
4. Expand tests to cover all five evidence kinds per requirement before
   qualification. *(F-05)*
5. Re-weight mutant selection by risk with recorded rationale. *(F-06)*
6. Separate implementation authorship from oracle authorship; record identities.
   *(F-03)*

**Round-2 rule (per the agreed cap):** after these land, one full confirmation
sweep. A new confirmed escape in round 2 blocks GO unless Brian explicitly
extends. Reaching the cap with any confirmed escape unfixed and unaccepted
blocks GO by construction.
