# Complete ILRP ordering proof: pending representation decision

Status: recommendation approved in full by the user on 2026-09-14.
Authority is recorded as DG17.6 / AM-17.12 supplement in M17 and the Phase 0
amendment ledger. This document preserves the decision rationale; implementation
and proof qualification remain incomplete.
Source inspected: ab5a33d38e6953d1c9f38066596b90658580ca87.
The approved whole-core objective remains `PLAN.md`; no obligation is closed.

## Evidence that makes this a decision

The prior acknowledgement slice explicitly rejected a vector adapter because it
changed allocation-failure behavior (`safety-ack-slice.md:19-24`). The replacement
borrows stack singleton slices. That technique proves a membership predicate,
not a complete schedule over arbitrary numbers of steps and dependencies.

`RepairStepId` wraps UUID (`crates/liminal-id/src/ids.rs:25-52,101-104`). Existing
IDs cannot be borrowed as an ordinary `&[u128]` without a representation change
or an invalid reinterpretation. This proposal introduces neither unsafe casts
nor an identity-layout change. The exact vstd admission still disables
default/std/alloc features; the proven leaf would continue to borrow slices.

The host's `topo_order` already allocates maps, sets and a result vector
(`crates/liminal-jurisdiction/src/repair.rs:212-263`). No explicit fallible
allocation handling was found in the inspected ordering/admission/recovery
paths. Public `CycleError` has only UnknownStep and Cycle. Admission wraps
refusals in an allocated Review vector; returning that wrapper is not an
allocation-free exhaustion guarantee. Claiming otherwise would be false.

## Recommended decision

Permit temporary host-side proof representations with checked size arithmetic
and fallible allocation. Introduce an explicit non-allocating resource-exhaustion
error for this new preparation, propagated before intent/effect acceptance or
before recovery advances. Preserve all existing diagnostics and their ordering
when the old validations already refuse. Preserve accepted semantics when the
required resources are available. Do not misclassify exhaustion as a cycle,
corrupt persisted data, or an automatic human-review decision.

This is scoped to new proof-representation preparation. It does not claim to
make every existing allocation in Kahn ordering, DTO decoding, graph storage or
receipt construction fallible. Those resource contracts remain separately
required under the plan. It does not enable vstd allocation features, impose a
new fixed plan-size ceiling, adopt assumptions, or authorize a phase.

Alternative: retain allocation behavior and admit explicit trusted collection/
identity views into the proof model. That reduces new buffers but expands the
trusted adapter specification and its review/qualification burden. The current
plan does not silently authorize new unchecked view assumptions. Changing ID
layout or imposing a fixed stack-size maximum would be separate, broader choices.

## Required complete result, regardless of representation

The executable core must characterize acceptance/refusal of the actual plan and
actual schedules: complete unique membership; all dependency endpoints; strict
edge order; graph-before-external prohibition; and smallest-ready identity
tie-breaking for the original host schedule. The external-first applied order
needs a separate stable-partition relationship preserving membership and edge
order. Finalization's graph filtering must bind to the same checked identities
and operations. A two-boolean edge classifier is not a substitute.

Call-path inventory includes admission (`admission.rs:151,214-222`), Prepare
(`ilrp.rs:606-625`), Apply/recovery (`ilrp.rs:738`), persisted validation
(`ilrp.rs:942`), finalization (`ilrp.rs:396`), committed-history validation
(`ilrp.rs:505`), resulting Basis (`ilrp.rs:313`) and applied receipt order
(`ilrp.rs:337-343,524`). Existing history/provenance validation must remain;
repeating a structural check does not establish authorization.

Before production migration: record the resolved decision/amendment, confirm the
public test seam, implement real positive/negative controls, prove the full
checker and partition/filter relationships, and test production refusal before
effects. Translation mistakes, missing/duplicate steps, omitted dependencies,
wrong identity bits, wrong partitioning, deterministic tie errors and injected
resource refusal all require independent controls. Qualification remains open.
