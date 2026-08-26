# 0021. Stage HAQP-1 qualification around Phase 1 authorization

- **Status:** accepted
- **Date:** 2026-08-08
- **Deciders:** Brian
- **Related:** ADR-0020; M17 (AM-17.2, AM-17.4); M24;
  `docs/execution/m17-5-adversarial-findings.md` F-28

## Context

ADR-0020 requires, in §3, that qualification predeclare **at least 64 semantic
mutants** and achieve a **100% kill rate**, with one survivor blocking
eligibility. M17.5's exit criterion inherits that requirement.

M17.5 finding **F-28** established that the requirement is currently
unsatisfiable, and not through any defect in the packet. The 65 declared mutants
name **35 distinct killing tests**. Measured 2026-08-03:

| killing tests | state |
|---|---|
| 27 | did not exist — they were strings in `packet.json` and nothing else |
| 8 | exist, `#[ignore]`d under the AM-17.2 Phase 1 quarantine |
| **0** | **runnable** |

Twenty of the 27 have since been written (`conformance/tests/milestones/m18–m23`)
and all pass, but they remain quarantined: they are Phase 1 exit gates, and
AM-17.2 exists because F-02 found Phase 1 gates pre-greened while Phase 1 was
unauthorized. The remaining seven have no subject to test — there is no `lim fmt`
command, no benchmark baseline, and no M24 aggregate gate.

Lifting the quarantine requires Phase 1 authorization. Phase 1 authorization
comes from M17.6's GO/NO-GO, which is sequenced **after** M17.5. So M17.5 cannot
complete, and no amount of M17.5 work changes that.

The underlying cause is simpler than the mechanics suggest: **ADR-0020 §3
assumed the suite it mutates already exists.** For every other evidence kind
that assumption holds — fuzz, crash, generated, canaries and reviews all run
against artifacts Phase 0 produces. For mutants it does not. A suite that has
not been written cannot be mutation-tested, and predeclaring the tests a future
milestone will write is exactly what an inventory is for.

## Decision

HAQP-1 SHALL be evaluated in **two stages**, gated on Phase 1 authorization.

### HAQP-1a — provable before Phase 1

Everything ADR-0020 requires **except** the mutant clauses of §3:

- §1 fixed clean tree and two execution lanes
- §2 bidirectional traceability
- §3's **gate canaries** — every ratification gate keeps its disposable canary.
  Canaries run against the packet's own gates, not against Phase 1 tests, so
  they are provable now and remain in 1a.
- §4 generated, metamorphic and fuzz evidence
- §5 oracle and fault independence
- §6 two independent adversarial reviews
- §7 bounded campaign and residual risk

Satisfying HAQP-1a makes the packet eligible for M17.6's GO/NO-GO. It does
**not** ratify the Phase 1 suite, and it does not claim the suite detects seeded
defects.

### HAQP-1b — the mutation requirement, deferred to M24

These §3 clauses defer verbatim, with no threshold weakened:

- at least 64 semantic mutants across critical surfaces, with the declared
  operator set
- at least eight mutants per critical surface family; no operator supplying more
  than a quarter of the denominator
- 100% kill of applicable, non-duplicate, non-equivalent mutants; one survivor
  blocks
- equivalent/duplicate dispositions naming the mutant, showing the proof, and
  carrying concurring verification in both adversarial-pass records

HAQP-1b runs at **M24**, when M18–M23 have written their tests and the gates are
legitimately un-ignored. Phase 1 suite ratification requires **both** stages.

### What this does not permit

- It does not un-ignore any Phase 1 gate. AM-17.2 stands.
- It does not lower the mutation bar. Every threshold in §3 survives intact into
  1b; only its evaluation point moves.
- It does not let a stage-1a packet claim mutant kills. The packet records
  `qualification_stage`, and `verify_qualified_repo` rejects a 1a packet whose
  mutants claim `killed` and a 1b packet whose mutants do not.

## Consequences

- M17.5 becomes completable: its exit criterion is HAQP-1a (AM-17.4).
- Phase 1 ratification gains a second, later gate at M24 rather than losing one.
  The suite is qualified against the tests that actually exist at each point.
- The mutation evidence, when it arrives, will be stronger than it would have
  been now: it will measure the real Phase 1 suite rather than a suite of
  predeclared names.
- A reader of a `complete` packet must check which stage completed. The stage is
  a required field precisely so that check cannot be skipped.
- **Disclosed limitation (M17.5 F-33, added 2026-08-26):** the deferred plan is
  not merely unevaluated, it is currently **inapplicable**. 36 of the 65
  declared mutants are anchored to declarations — function signatures and enum
  variants — which no operator can mutate, so they can never be killed while
  still counting toward §3's denominator. 65 − 36 = **29**, against a floor of
  64. HAQP-1b is therefore not "evaluate the existing plan" but "author most of
  one": re-anchor 36 and add roughly 35 more, which is the same work as F-06's
  risk-weighted selection.

  A 1a packet may complete over this plan, and a reader must not mistake that
  for the plan being sound. `verify_mutant_anchors_support_operators` refuses
  inside the `1b` branch so the gap cannot be rediscovered after Phase 1 is
  built, and a canary asserts the count is still exactly 36.

- **Disclosed limitation (M17.5 A05, AM-17.7, added 2026-08-26):** §5's
  "exhaustive registered crash boundaries" is, in a 1a packet, exhaustive over
  **ILRP only** — 8 of the runtime's 38 durable transitions. Every `txn.commit()`
  is a durable boundary by `liminal-graph`'s own documentation and none is
  registered. A reader must not read a complete 1a packet as evidence that the
  runtime's crash behaviour has been exhaustively probed; it evidences the ILRP
  protocol's. `packet.crash_boundary_scope` states the bound, and
  `verify_crash_boundary_scope` pins it to tracked source so it cannot silently
  widen or rot.

- **Disclosed limitation:** between M17.6 and M24, Phase 1 proceeds on a suite
  whose defect-detection power has been argued but not measured. That is a real
  risk and it is accepted deliberately, because the alternative — building
  Phase 1 first so the suite can be mutated, then qualifying — inverts ADR-0020's
  purpose entirely.

## Alternatives considered

- **Re-target the 65 mutants at surfaces whose tests are active today**
  (`liminal-format` unit tests, the law canaries, the fuzz regression corpus).
  Makes §3 satisfiable immediately, and the campaign has already measured those
  surfaces. Rejected as the primary answer because it narrows the claim from
  "the Phase 1 suite detects seeded defects" to "the Phase 1 substrate does",
  which is a weaker statement than ADR-0020 wrote down and would be recorded
  under the same name. Nothing prevents doing this additionally within 1a as
  supporting evidence, provided it is not labelled as satisfying §3.
- **Authorize Phase 1 first and qualify at exit.** Removes the circularity, and
  inverts the point of the profile: Phase 1 would be built before the suite
  judging it was trusted, which is F-02 at full scale.
- **Lift AM-17.2 for the qualification run only.** Deliberately repeating F-02.
  The gate F-28 added exists specifically to refuse this, and removing it to
  pass would be tuning the measure.
