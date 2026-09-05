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
- **RESOLVED (M17.5 F-33, closed 2026-09-03):** this previously disclosed that
  36 of the 65 declared mutants were anchored to declarations — function
  signatures and enum variants — which no operator can mutate, leaving 29
  against a floor of 64, and deferred the repair to 1b.

  The disclosure did not survive contact with §6. A blind reviewer rediscovered
  it on 2026-09-02 at six coordinates and returned six verified defects, and a
  finding can only be cleared by a commit that changes the code at its
  coordinate — there is no "resolved by disclosure" path, by design. A defect
  that is documented but not fixed still fails the gate, which is the correct
  behaviour and the reason the disclosure was the wrong instrument.

  The plan was re-anchored instead: every declared mutant now names a line where
  its declared operator can genuinely be applied, 17 operators were corrected
  where the original 5×13 grid had assigned an operator to a family whose code
  cannot exhibit it (F-06's finding, in the specific), no two mutants share an
  anchor, and the heaviest operator supplies 12.3% against §3's 25% cap.
  Dispositions remain `predeclared`: re-anchoring makes the declarations TRUE,
  it does not claim a kill, so it is 1a work and 1b's evaluation is untouched.

- **RESOLVED by definition (M17.5 A05, AM-17.7 → AM-17.8, closed 2026-09-04):**
  AM-17.7 scoped §5's "exhaustive registered crash boundaries" to ILRP and
  disclosed that the runtime's other ~30 fsynced durable transitions were out of
  scope. That was a disclosure without a criterion. AM-17.8 supplies one: a
  registrable crash boundary is a point where a crash leaves state that recovery
  must *reconcile*. ILRP's eight protocol steps qualify. A single checksummed log
  append does not — it either landed or it did not, and recovery is the
  longest-checksummed-prefix rule `store/log.rs` documents and
  `prop_torn_tail_truncates_cleanly` proves. The registry stays at eight because
  eight is the count of reconciliation points, not because the rest were
  deferred. `packet.crash_boundary_scope` carries the criterion and
  `verify_crash_boundary_scope` still pins every transition to a declared surface.

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
