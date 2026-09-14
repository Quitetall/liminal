# RFC 0002 — Record the approved safety dependency leaf

- Status: draft (topology projection of approved AM-17.12 implementation)
- Date: 2026-09-14
- Amends: v4 §117 (monorepo structure); clarifies RFC-0001's module boundary

## Summary

Record `crates/liminal-safety` as the dependency leaf selected in approved
AM-17.12 obligation 6. This record does not claim separate constitutional
acceptance, formal-companion adoption, proof establishment or phase authorization.
The implementation authority is already recorded in M17 and
`verification/authority-amendment-proposal.md`.

## Motivation

The approved construction plan verifies the executable deterministic decisions
called by production. A dependency leaf keeps graph, revision, jurisdiction and
I/O dependencies outside the proof-safe representation boundary. The canonical
v4 §117 layout and RFC-0001 do not yet show that selected crate.

## Guide-level explanation

No user vocabulary or command behavior changes. Contributors see an additional
workspace member consumed by `liminal-jurisdiction`. Jurisdiction remains the
single policy home and retains its shared repair/run/recovery interpreter.

## Reference-level explanation

The leaf has no dependency on graph, revision or jurisdiction. Minimal lossless
representations cross its interface; they do not create a third semantic
primitive or independently authorize effects. Its initial acknowledgement
predicate checks identity and dependency membership for v4 §7.8. Poststate,
durable history and the runtime translations remain outside that narrow proof.

RFC-0001's `repair` and `ilrp` modules remain in jurisdiction. This is not a
separate repair executor, ownership model, compiled policy or verified twin.
The leaf body proved by Verus is the body compiled into production.

DG17.5 separately authorizes exact `vstd` dependency admission with default,
std and alloc disabled, and refusal on resolved pin or feature drift. Dependency
admission is not a claim that all library macros have been formally proved.

## Conformance impact

Existing phase gates, fixture formats, assertions and eight ILRP crash
boundaries remain unchanged. Add leaf acknowledgement literal/compile-fail
controls, retain `ilrp_rejects_mismatched_ack_before_persistence`, and exercise
dependency-admission wrong-pin and feature-unification refusals. No held-out
corpus, golden, threshold, or M18–M24 gate is changed. Candidate formal evidence
remains separate from HAQP and cannot mark incomplete obligations established.

## Drawbacks

One additional workspace member and pinned verification-library dependency.
Rust translation adapters and host/compiler assumptions remain explicit proof
boundaries. Every subsequent production migration needs fresh applicable
verification; the baseline HAQP campaign cannot qualify later code.

## Alternatives

- Keep decisions inside jurisdiction: retains ordinary code but does not provide
  the approved small dependency-leaf boundary.
- Prove a separate specification implementation: rejected by ADR-0018's single
  semantic path and the approved same-executable-body requirement.
- Split the repair interpreter: rejected; this RFC retains RFC-0001's one
  interpreter and only extracts deterministic checked predicates.

## Unresolved questions

Formal constitutional projection acceptance is distinct from existing
implementation approval. Companion adoption and whole-core qualification remain
pending. The acknowledgement slice alone does not discharge `ilrp-ack`; further
protocol, store, Basis, adapter, model and evidence obligations remain in
`verification/PLAN.md`.
