# Repair-DAG fixtures

**Populated by:** M4 (repair DAGs, determinism ≠ safety, revert,
Promotion unification — killer experiment #2). Empty until then by
design.

**Spec:** R4 §5 (repair plans are dependency DAGs), v4 §7.7, v4 Laws 3F
(mutation-local repair) and 3G (Promotion is repair; determinism is not
safety).

## What lives here

Serialized `RepairPlan` values plus their expected interpretation:

- **Plan JSON** — the plan as data: `steps` (a map of `RepairStepId` to
  `ProposedMutation` with subject, operation, expected prestate,
  expected poststate, idempotency key) and `dependencies`
  (`{before, after}` edges), mirroring the v4 §7.7 structs.
- **Expected schedule** — the topological order (or, where several
  orders are valid, the dependency edges every valid schedule must
  honor).
- **Expected authorization** — the per-mutation, mutation-local verdict.
  Authorization is conjunctive: every mutated subject's Jurisdiction
  must authorize its own mutation, and no Contract may commandeer
  another subject (Law 3F).
- **Expected auto-apply verdict** — with the safety evidence that
  justifies it, or the reason the plan is offered for review instead
  (a unique candidate without domain safety evidence must not
  auto-apply — R4 §6).

## Consuming tests

`tests/phase_minus_1.rs::two_step_repair_dag_orders_id_insert_before_reattach`,
`cross_jurisdiction_repair_is_mutation_local`,
`promotion_uses_the_same_repair_interpreter`, and the M4 tests
`dag_id_insertion_precedes_reattach` and
`promotion_observationally_equiv_repairplan`.

The end-to-end scenario driving the canonical two-step DAG is
`../scenarios/dag_id_then_reattach.scenario.toml`; fixtures here test the
plan interpreter in isolation from the toy workspace.
