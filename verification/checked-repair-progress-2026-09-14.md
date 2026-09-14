# Checked repair and scoped-store integration

Implementation authority: AM-17.12, DG17.3, and Brian's instruction to complete
the remaining work. Baseline: `f11e130c19f8717811e9c9396a6bc7f993ce7af5`.
This is a development ledger, not qualification, SAS acceptance, or phase GO.

## Implemented runtime boundaries

- Store creation yields `StoreOwner`; ordinary `GraphStore` references cannot
  open transactions, create snapshots, or construct privileged writers.
- Coordinator, bootstrap, reactor, capture, bookkeeping, host-control, epoch,
  and reconciliation writers have separate operation/namespace restrictions.
  A refused operation poisons its transaction. Scoped transactions retain their
  original head guard; callers cannot retarget that guard after checking scope.
- Working buffer generations are immutable once captured. The explicit,
  owner-issued corruption fixture grant remains able to corrupt selected blobs;
  ordinary capture does not acquire that fault-injection authority.
- `AuthorizedRepair` is privately constructed, non-serializable, consumed once,
  and bound to the checked store instance and revision. Automatic admission uses
  the existing interpretive checker. Human acceptance requires the caller's
  `ActorId`; that value records an identity claim, not authentication.
- File inputs may use an existing durable file mirror only after recomputing
  the requested content hash. A corrupt content-addressed entry is not hidden
  by fallback. This makes clean undo admit existing bytes without widening
  bookkeeping rights or inserting a new durable write.
- New prepares use `ilrp:prepare:checked-v1`. Recovery checks actual retained,
  checksummed history: initial prepare, immutable plan/evidence, legal progress,
  acknowledgement identities/poststates, and exact final graph operations.
  Live publication and recovery share the same progress validator.
- `FinalizationPermit` checks graph pre/poststates against a captured head. The
  existing single final transaction publishes graph effects and committed
  intent together; its head comparison occurs under the append mutex.
- `CommittedRepair` borrows its store and carries the verified plan, evidence,
  accepted transaction/revision, and external-then-graph application order.
  Accepted-repair bookkeeping requires that receipt and checks store and exact
  metadata binding. Contested outcomes produce no receipt and retain overlays.

The resulting Basis now records the final transaction and graph revision plus
acknowledged external observations, rather than copying the input Basis. This
is historical evidence, not a claim that external files remain unchanged. File
mirrors and decision records still commit separately and are not included in
the graph-finalization receipt. The legacy input `WorkspaceBasis.transaction`
label is retained as DTO metadata; it is not used as proof of store provenance.

## Compatibility and explicit limits

Unchecked legacy intent records are preserved and refused rather than silently
promoted to checked authority. This patch does not provide an automatic legacy
readmission tool. Historical human-approval labels are not newly authenticated.
Trusted owner assembly, host I/O, and explicit fault grants remain assumptions.

The leaf `liminal-safety` proof core, production same-body proofs, complete
finite models, qualified adapters, and formal evidence aggregation are still
separate unfinished work in `PLAN.md`. These runtime guards do not discharge
those obligations. No heldout corpus, golden value, SAS bytes, or phase
authorization has been changed by this slice.

## Development evidence

- Graph/jurisdiction/daemon focused run: exit 0, 74 runtime tests and five
  compile-fail doctests, before the two later graph-finalization refusal tests.
- Admission suite subsequently: exit 0, nine tests, including impossible graph
  poststate, intervening graph edit, stale/cross-store capability, forged
  terminal state, durable receipt replay, and file-mirror controls.
- All original seven ILRP driver test names and assertions remain. Prepared,
  Applying, and Finalizing recovery fixtures use real driver progress. The
  hookless ExternalApplied fixture explicitly synthesizes trusted-root history
  after the real acknowledgement boundary; it is not a crash witness.
- Same-generation substitution and reactor target-kind race tests each failed
  on the unfixed implementation (exit 101), then passed after their fixes.
- Full workspace/all-target compilation passed. Full workspace Clippy with
  warnings denied passed after narrow, documented census-preserving allowances.
- Public one-command revertibility conformance test: exit 0, including clean
  undo and stale-undo review behavior, using freshly built native-target CLI.

These development command results are in the session transcript, not fabricated
durable log files. Fresh fixed-candidate verification must produce durable logs.
Full CI, fresh canaries/campaign evidence, exact reference maintenance, and final
external reviews are not claimed complete by this document.

Early LAMU reviews were diagnostic only: one unbound graph review reported
PASS WITH NITS; another jurisdiction review's critic text was truncated. Neither
is current-candidate review evidence. Verified false positives included claims
that separate borrows of the same store fail `ptr::eq` and that terminal-history
rejection was only implicit. Both claims contradict the actual code. Required
complete, hash-bound reviews and per-commit reviews remain outstanding.
