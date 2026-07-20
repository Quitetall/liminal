# ILRP crash-matrix inventory

**Populated by:** M2 (single-step promotion) and M4 (two-step repair DAG).

**Spec:** v4 §7.8, Law 3H, R4 §§7, 10.

## Coverage authority

This directory documents the inventory contract. No hand-authored matrix case
file defines executable coverage. Runnable fixtures under
`conformance/fixtures/scenarios/` declare expected terminal state; a sound
baseline trace supplies every `(scenario, fault point, occurrence)` case.

- **Fault point** is one of the five registered ILRP durable boundaries: intent
  commit, external apply, acknowledgement, before graph finalization, and after
  graph finalization but before notification.
- **Occurrence** is the nth firing in the baseline. A two-step DAG can hit one
  boundary twice; both observations become distinct crash cases.
- **Expected terminal** comes from scenario `expect.terminal` and is one of
  `Committed`, `NeedsReview`, or `Aborted`.
- **World digest** is computed from recovered files, graph store, and intent
  log. Committed recovery must match the sound baseline. Every case must produce
  equal first- and second-recovery digests.

`ToyRun::baseline` records ordered hits. `HitTrace::enumerate_faults` returns
that full inventory. `ToyRun::crash_matrix` kills at every derived pair, proves
the point fired, recovers to a non-empty terminal set, compares expected state,
checks the durable world, rejects staged leftovers, and repeats recovery for
idempotence. Adding a hand-listed case cannot increase coverage.

## Consuming tests

- `tests/phase0.rs::ilrp_fixture_inventory_covers_every_durable_boundary`
- `tests/phase_minus_1.rs::crash_ilrp_resumes_after_kill_at_every_boundary`
- `tests/crash.rs::crash_matrix_promote_single_step`
- `tests/crash.rs::crash_matrix_two_step_dag`
- `tests/crash.rs::crash_recovery_never_guesses`

Third-state recovery must land in `NeedsReview`, preserve all bytes, and remain
idempotent. Recovery never guesses.
