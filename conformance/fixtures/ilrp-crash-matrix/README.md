# ILRP crash-matrix fixtures

**Populated by:** M2 (single-step promotion, killer experiment #1) and M4
(two-step repair DAG). Empty until then by design.

**Spec:** v4 §7.8 (Intent-Logged Repair Protocol), v4 Law 3H, R4 §7,
R4 §10 ("the daemon is terminated after every ILRP durable boundary").

## What lives here

One expected-outcome table per crash-tested scenario: the expected
terminal state and durable-world digest for every
`(scenario, fault point, occurrence)` triple.

- **Fault points** are the ILRP durable boundaries named by R4 §10:
  intent commit, external apply, acknowledgement, before graph
  finalization, and after graph finalization but before the completion
  notification.
- **Occurrence** is the nth time a boundary fires within the scenario —
  a two-step DAG hits the external-apply boundary twice, and each hit is
  a distinct crash case.
- **Expected terminal** is one of `Committed`, `NeedsReview`, or
  `Aborted` — never a hidden half-state (R4 §10).
- **World digest** is a content hash over the recovered durable world
  (files + graph store + intent log). Recovery must be idempotent:
  recovering twice yields equal digests.

The matrix cases themselves are *derived*, not hand-enumerated: the
harness records a hit trace from an uncrashed baseline run and turns
every observed `(point, occurrence)` into a kill case, so a newly added
durable boundary cannot be silently untested. These fixtures pin the
*expected outcomes* the derived cases must reach.

## Format (sketch, finalized at M2)

`<scenario>.matrix.toml` — one `[[case]]` per triple:

```toml
[[case]]
fault_point = "external_apply"
occurrence = 1
terminal = "NeedsReview"
world_digest = "blake3:…"
```

## Consuming tests

`tests/phase_minus_1.rs::crash_ilrp_resumes_after_kill_at_every_boundary`,
plus the milestone tests `crash_matrix_promote_single_step` (M2) and
`crash_matrix_two_step_dag` (M4), and the harness-level
`recovery_never_guesses` (a third-state file must land in `NeedsReview`
with zero bytes lost — v4 §7.8 step 5: "never guess").
