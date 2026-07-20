# 0014. Correct spec-debt meter active counts

- **Status:** accepted
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** ADR-0013; `docs/execution/M12.md`; v4 §112–114

## Context

ADR-0013 recorded `just gates` output of 238 active and 21 deferred tests at
M12.4. Adversarial review of the Phase 0 backlog found that the static scanner
incremented `active_tests` as soon as it saw `#[test]`, before it could observe a
following `#[ignore]`. Every deferred test was therefore counted once as active
and once as deferred. Independent `cargo nextest list` evidence confirmed that
the M12.4 tree had 238 total tests, of which 21 were ignored: 217 were active.

## Decision

We will count a test only when its function declaration is reached, after its
complete attribute set is known. A reasoned ignore is deferred under its Phase
tag; a bare ignore is visible as `unphased` debt; neither is active.

Corrected meter checkpoints are:

| Checkpoint | Active | Deferred | Notes |
|---|---:|---:|---|
| M12.4 Phase -1 close | 217 | 21 | Corrects ADR-0013's count only; Phase -1 deferred remains 0. |
| M12.5 after corrective tests and Phase 0 gate stubs | 219 | 39 | Adds 2 active regressions and 18 deferred Phase 0 gates. |
| Predicted Phase 0 close | 237 | 21 | All 18 Phase 0 gates active; pre-existing later-phase backlog unchanged. |

ADR-0013's **GO** decision remains valid. This ADR corrects its meter evidence;
it does not supersede or weaken the decision.

**What would falsify or reverse this:** `scan_workspace_debt` disagreeing with
nextest's active/ignored partition for the same tracked Rust test surface, or an
attribute form that the static scanner cannot classify without ambiguity.

## Consequences

- **Positive:** “active” again means non-ignored, matching documentation and
  nextest semantics; future phase deltas measure real backlog movement.
- **Negative:** historical active counts printed before this correction are
  totals-with-double-counting and must not be compared directly to corrected
  counts.
- **Follow-ups:** M13–M17 use the corrected 219/39 Phase 0 baseline and finish at
  237/21 if no test inventory amendment occurs.

## Alternatives considered

- **Rename the metric to total tests** — rejected because rendered output and
  protocol explicitly define active as non-ignored, and backlog flips must move
  tests from deferred to active.
- **Preserve ADR-0013's number for continuity** — rejected because accepted ADRs
  are immutable; this corrective ADR preserves the audit trail.
