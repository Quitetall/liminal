# 0008. Phase -1 execution amendments ledger

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** docs/execution/00-protocol.md §2 (amendment procedure); all Phase -1 work orders

## Context

The execution protocol (docs/execution/00-protocol.md §2) requires that every
amendment to a frozen surface — a signature rename, a parameter addition, a
work-order correction — is recorded in the work order's Amendments section AND
appended to this ADR as a single line. This creates an auditable trail of every
deviation from the planned scaffold, reviewed at M12 (the final gate).

## Decision

We will maintain this file as the single ledger of all Phase -1 execution
amendments. Each line takes the form:

```
AM-<milestone>.<sequence> (M<nn>): <one sentence>
```

Amendments are ratified or reverted at M12. No amendment is recorded without
its corresponding work-order entry.

## Amendment log

AM-1.1 (M01): liminald CLI — hidden `__fire` subcommand added — the injector must be provable before `exec` exists, through the production env path (D01.1).
AM-1.2 (M01): test naming — plan shorthand gains the `crash_` prefix so the serialized nextest crash group matches; assertions unchanged (D01.3).
AM-2.1 (M02): ScenarioScript moves crates — model relocates to liminal_daemon::scenario; conformance/src/scenario.rs becomes a re-export shim.
AM-2.2 (M02): Setup gains `buffers: Vec<SetupBuffer>` — promote_single_step contains `[[setup.buffer]]` that serde currently drops silently. Additive.
AM-2.3 (M02): liminal_graph::store::ns module — aux-namespace constants get one home (protocol §7).
AM-2.4 (M02): conformance `[[bin]] lim-toy` — D02.6 binary-location resolution.
AM-2.5 (M02): test-name prefixing — recovery_never_guesses → crash_recovery_never_guesses (nextest crash-group filter). Assertions unchanged.

**What would falsify or reverse this:** an M12 audit finding an amendment that
was applied in code but not recorded here, or recorded here but not in the
work order.

## Consequences

- **Positive:** every deviation from the scaffold is visible and auditable.
- **Negative:** a small amount of bookkeeping per amendment.
- **Follow-ups:** M12 audits this file against the work orders and code.

## Alternatives considered

- **No ledger** — rejected because silent deviations to frozen surfaces would
  accumulate unchecked, and Phase -1's conclusions depend on the scaffold
  matching the spec.
