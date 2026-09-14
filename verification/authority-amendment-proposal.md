# AM-17.12: close ordinary raw-write authority before proving ILRP

Status: **user-approved for implementation, 2026-09-13**. Brian authorized
steps 1–4: record this amendment, migrate scoped StoreOwner/read-only GraphStore,
enforce checked repair admission/recovery/finalization/receipts, and add bypass
controls while retaining existing behavior and assertions. Full verification
requires the next checkpoint. Approval is not SAS successor acceptance, phase
authorization, proof establishment, or suite ratification. The number was checked
against baseline `64ee56528df5f95b0f3c12f7f881942a6b2343f3`; AM-17.12 was unused.

## Approved decision

Make ordinary `GraphStore` handles read-only. Move root write authority into an
explicit, non-serializable `StoreOwner` capability held by trusted workspace
assembly. Ordinary repair callers receive checked repair capabilities, not the
owner or a general transaction builder.

The approved plan permits scoped API amendments. This proposal makes the
previously unspecified root-authority boundary explicit before changing the
frozen interfaces. It does not ask again whether to pursue a proven safety core.

## Why this boundary matters

At the baseline, `GraphStore::begin` opens unrestricted transactions
(`crates/liminal-graph/src/store/mod.rs:306`), `GraphTxn::apply` accepts graph
operations (`:493`), and `GraphTxn::put_aux` accepts generic namespace writes
(`:501`). `GraphStore::put_working_aux` is another mutable surface (`:391`).
These paths do not require the mutation-local checker or a validated ILRP
intent. Proving the repair helper while retaining ordinary access to these
paths would not establish the plan's by-construction authority claim.

ILRP also deserializes raw intent state before treating terminal values as
authoritative (`crates/liminal-jurisdiction/src/ilrp.rs:356` and `:376`). Its
acknowledgement helper inserts an executor-supplied acknowledgement without
checking the acknowledgement's internal step identity or observed poststate
(`:289`). These are code-observed migration obligations, not claims that an
end-to-end exploit has already been reproduced.

The daemon currently discards the successful `IntentState` returned by `run`
before follow-up bookkeeping (`crates/liminal-daemon/src/runner.rs:957`). The
new receipt interface must make committed, contested, aborted, and deferred
outcomes distinct; an `Ok(NeedsReview)` must not confer a committed receipt.

## Approved interface obligations

1. **Root authority.** Opening/creating an owned store yields `StoreOwner`,
   which lends a read-only store view and explicitly scoped coordinator
   capabilities. This intentionally changes the frozen opening/beginning
   interface. Root ownership is an explicit trusted assembly boundary, not a
   claim to prevent arbitrary equal-privilege local code from opening files.
   Owner and privileged capabilities have private construction, are bound to
   one store instance, and cannot be created by deserialization.
2. **Narrow internal writers.** Bootstrap, reactor materialization, epoch,
   capture/debt, and bookkeeping receive separate permitted operation and aux
   namespace sets. They do not receive a subject-local repair capability by
   virtue of being internal. The permitted sets must be enumerated and tested
   before their call sites migrate; no catch-all internal writer is exposed
   to ordinary request handling.
3. **Ordinary repair admission.** The existing interpretive Checker remains
   the semantic authority. Its complete runtime conjunction produces an
   `AuthorizedRepair` only after validating the captured Basis, subjects,
   ordering, governing profiles, safety and recovery/revert conditions.
   Public DTOs, `SafetyEvidence`, caller-supplied Boolean claims and unchecked
   deserialization cannot mint that capability. Human acceptance is a distinct
   recorded route, not an invented actor or automatic approval.
4. **Durable admission and recovery.** ILRP consumes authorized work, persists
   intent before external effects, and validates recovered DTOs against the
   store's durable history. A stored state label alone is not a capability.
   Validate repair identity, plan key/step identity, dependencies, acknowledged
   step identity and expected observed poststate before skipping any effect or
   admitting finalization. Preserve the one shared run/recovery interpreter.
5. **Finalization and receipts.** `FinalizationPermit` is bound to the repair,
   validated acknowledgements and relevant store state. A single transaction
   publishes graph effects and committed intent. Only its completed durable
   result produces a committed receipt. Subsequent file mirrors and repair
   records remain separately scoped writes and cannot enlarge that receipt.
   The run result must distinguish committed receipt from review/abort/defer.
6. **Leaf proof core.** The proof-safe crate remains a dependency leaf. Move
   only the minimal representations needed for checked decisions downward;
   do not introduce a graph/jurisdiction dependency cycle or a second policy
   interpreter. Trusted I/O adapter boundaries and owner assembly are explicit
   assumptions with exact coordinates, not invisible proof preconditions.

Concrete Rust signatures and representation layout are T2 implementation
details subject to these constraints and compile-fail/bypass tests. If they
cannot realize this authority boundary, stop rather than weaken the claim.

## Required migration witnesses

- Ordinary read views cannot start raw accepted transactions or write ILRP aux.
- A capability from another store cannot authorize this store's mutation.
- Raw DTOs and handcrafted terminal state cannot construct trusted outcomes.
- Wrong/missing acknowledgement and wrong poststate refuse before finalization.
- Every internal writer refuses operations outside its exact permitted set.
- Contested ILRP completion does not produce a committed repair receipt.
- Existing positive, mutation-local, Basis, graph/replay and all eight ILRP
  crash-boundary tests retain their assertions and behavior.
- Same executable decision bodies are proved and called by production; the
  arithmetic and Boolean bootstrap controls do not discharge these witnesses.

## Adoption and protected surfaces

Record this amendment in M17 and the active Phase 0 ledger in a separate
reviewed commit **before** production changes. Record any later discovered
semantic gap through the normal ambiguity procedure.

Do not change the accepted SAS's 777 IDs or bytes, the frozen HAQP instrument,
its thresholds or packet, locked corpora, or M18-M24 authorization. Successor
adoption of formal qualification obligations remains a separate human action.
This amendment authorizes interface migration only, not a phase GO or a
claim that the new implementation has already met its proof obligations.
