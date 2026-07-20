# 0012. Ratify the Phase -1 amendment ledger

- **Status:** accepted
- **Date:** 2026-07-19
- **Deciders:** Brian
- **Related:** v4 Part XXII final gate; M12 Algorithm B;
  [ADR-0008](0008-phase-minus-1-execution-amendments.md)

## Context

Phase -1 exposed missing seams and scaffold mismatches while preserving the
falsification experiments. M12 must ratify or revert every M1–M11 amendment so
none becomes accidental architecture. ADR-0008 was reconciled against all work
orders; seven omitted M3–M6 entries were restored before this disposition.

## Decision

We will ratify every implemented M1–M11 amendment. None is reverted:

| Amendment | Disposition | Reason |
|---|---|---|
| AM-1.1 | Ratify | Hidden crash-injector seam proves real process death. |
| AM-1.2 | Ratify | Test name routes through serialized crash group. |
| AM-2.1 | Ratify | Scenario model has one owning crate. |
| AM-2.2 | Ratify | Buffer setup no longer deserializes away. |
| AM-2.3 | Ratify | Aux namespaces have one canonical home. |
| AM-2.4 | Ratify | `lim-toy` is required conformance process surface. |
| AM-2.5 | Ratify | Recovery test routes through crash group. |
| AM-3.1 | Ratify | Relation identity requirements reach checker Q4. |
| AM-4.1 | Ratify | Safety and executor logic require stored graph evidence. |
| AM-4.2 | Ratify | Human acceptance is explicit and testable. |
| AM-4.3 | Ratify | Block merge belongs to source semantics. |
| AM-5.1 | Ratify | Offline aging and restart need scenario vocabulary. |
| AM-6.1 | Ratify | Published perspective fails closed honestly. |
| AM-6.2 | Ratify | Ambiguous working Holder needs explicit selection. |
| AM-7.1 | Ratify | Evidence ordering needs public grade strength. |
| AM-8.1 | Ratify | Resolver and query steps exercise M08 seams. |
| AM-8.2 | Ratify | Graph snapshots need stable dependency key. |
| AM-8.3 | Ratify | Durability classification is constitutive input. |
| AM-8.4 | Ratify | Scenario sources need deterministic identity. |
| AM-8.5 | Ratify | Working blobs must not advance durable graph head. |
| AM-8.6 | Ratify | Every Basis must pin graph revision. |
| AM-8.7 | Ratify | Live and frozen queries share one `World` seam. |
| AM-8.8 | Ratify | File blob mirror must follow landed saves. |
| AM-8.9 | Ratify | Materialized observations need an honest node kind. |
| AM-8.10 | Ratify | Cross-crate query worlds need workspace root access. |
| AM-8.11 | Ratify | Scenario Basis capture must see ingested durable files. |
| AM-8.12 | Ratify | Frozen replay needs a public synthetic `StateView`. |
| AM-9.1 | Ratify | Time-boxed spikes remain isolated workspace members. |
| AM-9.2 | Ratify | Real-CST remeasurement remains binding Phase 0 work. |
| AM-11.1 | Ratify | Manifest generation remains conformance-only. |
| AM-11.2 | Ratify | Scorecard execution remains conformance-only. |
| AM-11.3 | Ratify | Trace generation remains conformance-only. |
| AM-11.4 | Ratify | Per-event measurement uses the one production runner. |
| AM-11.5 | Ratify | Public histories provide independent real traces. |
| AM-11.6 | Ratify | Compression preserves decoded traces while manifest locks compressed bytes. |

M10 introduced no amendments. M12 amendments remain recorded in ADR-0008 and
the M12 work order; they are user-ratified checkpoint corrections, outside this
M1–M11 omnibus disposition.

**What would falsify or reverse this:** an amendment present in code but absent
from its work order or ADR-0008, a listed amendment not actually implemented, or
a later gate proving an amendment weakened its experiment. The affected item
must receive a superseding ADR or be reverted before proceeding.

## Consequences

- **Positive:** every Phase -1 scaffold deviation has explicit constitutional
  disposition.
- **Negative:** ratified additive seams become compatibility constraints until
  superseded deliberately.
- **Follow-ups:** M12 go/no-go appendix records this ADR and all M12 amendments.

## Alternatives considered

- **Ratify only amendments that changed signatures** — rejected because additive
  seams and runner vocabulary also shape what the experiments measured.
- **Keep ADR-0008 as sufficient disposition** — rejected because a ledger records
  history; it does not decide whether each deviation survives Phase -1.
