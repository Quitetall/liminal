# Architecture

This is the codemap: where things live, how the crates layer, and which
invariants you must not break. It deliberately stays at the level of module
boundaries; for the *why*, the canonical specification in [`spec/v4/`](spec/v4/)
is the constitution. Citations of the form "v4 §N" point into
`spec/v4/liminal_master_architecture_plan_v4.md`; "R4 §N" points into
`spec/v4/liminal_architecture_revision_4.md`. Code cites the spec, never the
reverse; a change that alters spec-observable behavior needs an RFC in
[`spec/rfc/`](spec/rfc/), a repo/implementation decision needs an ADR in
[`docs/adr/`](docs/adr/).

## Bird's eye view

The semantic kernel is two primitives (v4 Law 1):

```text
Node
Relation
```

Everything else — documents, text, tables, tasks, histories, policies — is a
derived composition (v4 §6). The kernel being tiny does not make the system
simple (v4 Law 2A): ownership, write routing, identity continuity, foreign
edits, and replication remain hard. All of that complexity is deliberately
sunk into one subsystem, **Jurisdiction** (v4 §7, R4 §1), which resolves every
independently governable Node or Relation to exactly one Holder, declared
merge domain, or provisional Overlay at an immutable **Workspace Basis**
(v4 Law 3, Law 3D). No other subsystem may create an independent ownership
model.

The project is in Phase -1, the falsification laboratory (v4 Part XXII,
Law 14): everything that exists is an *interpretive reference implementation*
whose job is to be cheap to run, cheap to inspect, and cheap to prove wrong.

## Crate layering

The dependency graph is acyclic and strictly layered:

```text
                 liminal-id                 identity vocabulary (v4 §4.1, §19, §44)
                 /        \
      liminal-source   liminal-graph        file-Holder staging (§7.8, §8.5)
                 \        /                 kernel types + toy §92 store
              liminal-revision              WorkspaceBasis, Perspectives,
                     |                      component-granular invalidation (§7.5, R4 §8)
            liminal-jurisdiction            THE COMPLEXITY SINK (RFC-0001):
                     |                      Contracts, profiles, checker, Overlays,
                     |                      Reconciliation Queue, RepairPlan, ILRP
              liminal-daemon                toy workspace harness `liminald`,
                     |                      crash injection (no sockets, not a daemon)
               liminal-cli                  `lim` — check, overlays, repairs,
                     |                      repair undo, jurisdiction explain
          conformance/ (liminal-conformance)
                                            §112 laws, R4 §10 gates, fixtures,
                                            held-out corpus policy, debt meter
```

`benches/` (`liminal-benches`) holds the §116 benchmark harness — baselines
recorded against the toy store, never CI gates until Phase 1 (Law 14).

### Tier A — where the real Phase -1 code lives

- **`crates/liminal-id`** — the identity vocabulary: `ContentHash` /
  `VersionId` (content-derived, v4 §19.3), UUIDv7 entity newtypes
  (`NodeId`, `RelationId`, `EntityId`, `TransactionId`, `RepairId`, ...),
  `IdentityGrade` (v4 §19.1), `JurisdictionSubject` (v4 §7.2). Law 13's
  entity/version split is enforced here by the type system.
- **`crates/liminal-graph`** — `Node`, `Relation`, `Operation`,
  `Transaction` (v4 §4–5, §86), and the hand-rolled toy store (ADR-0007):
  checksummed append-only log + atomic snapshot replacement per v4 §92.
  **The toy store is the ILRP coordinator (v4 §92):** it can transactionally
  retain repair intent and step progress in the same commit as graph state
  (`put_aux` inside a `GraphTxn`), which is exactly what makes crash recovery
  provable. This grants recoverability, *not* supranational Jurisdiction over
  the files and services being changed.
- **`crates/liminal-source`** — file-Holder mechanics: observations,
  prestate-checked staged writes (`.lim-staged` siblings that crash recovery
  can scan for), atomic rename (v4 §7.8 step 2, §8.5).
- **`crates/liminal-revision`** — `WorkspaceBasis`, `BasisComponent` (all
  seven variants of v4 §7.5), `BasisPerspective::{ClientScoped, DurableOnly,
  Published, Federated}` (R4 §8), and `resolve`, which encodes the
  anti-chimera rules of Law 3J.
- **`crates/liminal-jurisdiction`** — the one typed home for ownership
  complexity (RFC-0001 amends v4 §117 to add it). Modules: `holder`,
  `contract` (v4 §7.3), `profile` (exactly two profiles in Phase -1:
  external-file and graph-native, R4 §10), `checker` (the eight questions of
  v4 §7.9; sound means empty report and zero output), `overlay` (v4 §7.10),
  `reconcile` (the core Reconciliation Queue, R4 §9 — exists *before* any
  agenda subsystem), `repair` (RepairPlan dependency DAGs, v4 §7.7, R4 §5–6),
  `ilrp` (Intent-Logged Repair Protocol, v4 §7.8, R4 §7), and `explain`
  (the only surface where internal vocabulary is allowed to appear, R4 §3).
  Repair and ILRP are **modules, not separate crates**, because R4 §4 demands
  one interpreter for save, sync, merge, recovery, writeback, and Promotion.
- **`crates/liminal-daemon`** — the toy harness binary `liminald`: scripted
  scenarios, recovery sweeps, and the crash injector that turns every ILRP
  durable boundary into a real `SIGABRT` death point for the conformance
  crash matrix. Not a daemon; sockets arrive in Phase 2.
- **`crates/liminal-cli`** — the `lim` binary. `lim check` is silent and
  exits 0 on a sound workspace (asserted as empty byte strings, Law 3E);
  `lim overlays` / `lim repairs` surface reconciliation debt (Law 3I, R4 §9);
  `lim repair undo <id>` is the one-command revert (Law 3G); everyday
  vocabulary everywhere except `lim jurisdiction explain` (R4 §3).
- **`conformance/`** — the specification made executable: the R4 §10 gate
  tests, the §112 laws as generic functions over minimal trait seams, the
  crash matrix derived from observed fault-point hit traces, fixtures and
  corpora (development / held-out / regression, R4 §2.2), the R4 §2.3 SLO
  scorecard, and the spec-debt meter behind `just gates`. Tests that cannot
  pass yet are `#[ignore]`d with phase-tagged reasons — they are the backlog.

### Tier B — reserved stubs

These crates exist so the v4 §117 layout is real and so the §112 laws can be
written generically today, but they contain documentation and constitutional
vocabulary only. Each `lib.rs` states its purpose, spec sections, and the
phase at which implementation begins:

| Crate | Holds today | Spec | Implementation begins |
|---|---|---|---|
| `liminal-text` | doc only (toy uses `String` payloads) | v4 §46 | Phase 1 |
| `liminal-cst` | `OpaqueBlock`, `CoarseKind` | v4 §9, §22 | Phase 1 |
| `liminal-hir` | doc only | v4 §10 | Phase 1 |
| `liminal-cir` | doc only | v4 §11 | Phase 1 |
| `liminal-query` | `Query` trait seam (effects live outside queries) | v4 §24, §118A | interpretive use in Phase -1.3; real engine Phase 1 |
| `liminal-format` | the formatter laws | v4 §20 | Phase 1 |
| `liminal-protocol` | doc only (lists §58 methods as future surface) | v4 §58 | Phase 2 |
| `liminal-transform` | `TransformContract` | v4 §14, §28 | Phase 3 |
| `liminal-lua` | doc only | v4 §94 | Phase 3 |
| `liminal-history` | `SemanticDiff` seam | v4 §86–87 | Phase 6 |
| `liminal-sync` | `Replica` seam (evaluate existing CRDTs; never custom in Phase -1) | v4 §88 | Phase 6 |
| `liminal-resolver` | `Availability`, `Freshness`, replay seam | v4 §39–40 | Phase 7 |
| `liminal-resource` | doc only | v4 §47 | Phase 8 |
| `liminal-plugin-api` | doc only | v4 §93, §98 | Phase 11 |
| `liminal-wasm-host` | doc only | v4 §95 | Phase 11 |

`domains/`, `backends/`, `integrations/`, and `apps/` are deliberately empty
trees with READMEs naming their future crates (v4 §117); they join the
workspace when their first crate lands.

## The correctness laws (v4 §112)

These properties are the system's laws; the conformance crate holds each one
as a named test (passing where the API exists, ignored with the assertion
spelled out where it does not):

```text
parse(format(parse(x))) ≡ parse(x)
incremental_compile(x, edits) ≡ full_compile(apply(x, edits))
semantic_diff(apply(tx, g), g) matches tx
resolver replay with frozen inputs is deterministic
sync replicas converge under supported operation schedules
captured Workspace Bases never combine independent dirty buffers for one durable subject
repair recovery after any injected crash reaches Committed, NeedsReview, or Aborted
    without hidden half-state
automatic repair is authorized, domain-safe, idempotent, and one-command revertible
Promotion semantics are observationally equivalent to the corresponding accepted RepairPlan
```

## Boundary invariants — never break these

1. **Capture is never rejected (Law 3B).** Typing, drawing, dictation, and
   paste are preserved even when the intended Holder is unavailable,
   read-only, ambiguous, or inconsistent. There is no code path that refuses
   an edit; inadmissible writes become durable Overlays. Accordingly, repair
   authorization has no "Forbidden" outcome — only auto-apply or review.
2. **Sound states are silent (Law 3E, R4 §2.3).** The checker on an entirely
   sound workspace produces empty output and exit 0 — asserted as empty byte
   strings, not "no errors". Diagnostics from one root cause are coalesced;
   internal vocabulary (Jurisdiction, Holder, Overlay, Promotion) stays out
   of routine surfaces (R4 §3).
3. **One repair interpreter (R4 §4).** Save, sync, reattachment, merge,
   recovery, writeback, and Promotion are all RepairPlans evaluated by the
   same interpreter under the same authorization, identity, invariant, Basis,
   crash-recovery, and revert rules. Never add a parallel mutation or
   lifecycle mechanism.
4. **Determinism is not safety (Law 3G, R4 §6).** A unique textual or merge
   result is only a proposal. Automatic acceptance additionally requires a
   domain safety predicate (structural disjointness, a validator proving
   non-interference, or explicit approval), and every automatic repair is
   recorded and one-command revertible.
5. **Repair is mutation-local (Law 3F).** Each proposed mutation is governed
   by the Jurisdiction of the subject it changes; no Contract gains power
   over another subject merely because a Relation connects them.
6. **Cross-Holder repair goes through ILRP (Law 3H, v4 §92).** Persist the
   repair intent before the first external mutation; give every step an
   idempotency key with expected prestate and poststate; never claim
   multi-file atomicity; after a crash, resume, finalize, revert, or surface
   `NeedsReview` — never guess, never hide half-applied work.
7. **No chimeric Basis (Law 3J, Law 3D).** Every computation runs against one
   immutable Workspace Basis under an explicit Perspective. Two clients'
   independent dirty buffers over one durable subject never enter the same
   semantic snapshot.
8. **Overlay debt stays visible (Law 3I, R4 §9).** Unresolved Overlays are
   durable, age by profile policy, surface through `lim overlays` and at
   boundaries, and are never silently garbage-collected.
9. **Interpretive only until Phase -1 exits (v4 §125, Law 14).** No compiled
   Jurisdiction plans, no indexed policy dispatch, no generated policy code,
   no custom CRDT, no production parser, no performance work. The
   Jurisdiction and RepairPlan interpreters are the conformance oracles;
   optimized paths may exist only after differential tests prove them
   equivalent — including injected-crash recovery and concurrent-buffer
   Basis cases.

If a change would violate one of these, the change is wrong or the
constitution needs amending — in that order of suspicion, and the amendment
path is an RFC, not a quiet divergence.
