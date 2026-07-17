# RFC 0001 — Add `crates/liminal-jurisdiction` to the v4 §117 layout

- Status: accepted
- Date: 2026-07-17
- Amends: v4 §117 (monorepo structure)

## Summary

Amend the v4 §117 crate list with one new workspace member,
`crates/liminal-jurisdiction`: a single crate holding the entire
Jurisdiction policy subsystem — Holders, Contracts, profiles, the checker,
Overlays, the Reconciliation Queue, RepairPlans, ILRP, and `explain` — as
one typed home in the crate graph, positioned between `liminal-revision`
and `liminal-daemon`.

## Motivation

Three forces converge on the same conclusion.

First, R4 §12's final law: "Jurisdiction gives those problems one typed
home." In a Rust crate graph, a typed home *is* a crate — a named unit with
a public API, its own dependency budget, and a boundary the compiler
enforces. A "subsystem" smeared across several crates has no such boundary.

Second, v4 §7 calls Jurisdiction the deliberate complexity sink and the
largest policy subsystem, and forbids any other subsystem from creating an
independent ownership model. The §117 layout, taken literally, would force
Jurisdiction's machinery to scatter across `liminal-graph`,
`liminal-revision`, and `liminal-daemon` — each of which would then carry a
fragment of ownership policy, which is precisely the leak v4 §7 exists to
prevent.

Third, §117 predates Revision 4. R4 promoted RepairPlan dependency DAGs
(R4 §5), the Intent-Logged Repair Protocol (R4 §7), and the Reconciliation
Queue (R4 §9) from design sketches to constitutional machinery with their
own gate tests (R4 §10). §117 simply has no home for them. And §117 itself
frames its layout as a starting point ("Start in one monorepo. Split only
when release cadence, ownership, or build cost justifies it."), so amending
the list is the intended mechanism, not a deviation.

## Guide-level explanation

Nothing user-visible changes: vocabulary tiers (v4 §7.1, R4 §3), CLI
surfaces, and everyday draft/save/sync language are untouched. A spec
reader sees one new crate in the §117 tree:

```text
├── crates/
│   ├── liminal-id
│   ├── liminal-graph
│   ├── liminal-revision
│   ├── liminal-jurisdiction        ← added by this RFC
│   ├── ...
```

A contributor asking "where does ownership, repair, or reconciliation
policy live?" now has a one-crate answer.

## Reference-level explanation

`crates/liminal-jurisdiction` holds the following modules:

| Module | Contents | Spec |
|--------|----------|------|
| `holder` | Holder variants (File, Buffer, Graph, Object, GitObject, Service), merge-runtime references | v4 §7.1; R4 §11.7 |
| `contract` | `JurisdictionContract` policy data (scope, resolution, mutation, continuity, lifecycle), safety requirements | v4 §7.3; R4 §6 |
| `profile` | Jurisdiction profiles; external-file and graph-native in Phase -1 | v4 §7.6; R4 §10 |
| `checker` | The eight-question checker; silent when sound | v4 §7.9 |
| `overlay` | Overlay lifecycle states | v4 §7.10 |
| `reconcile` | Reconciliation Queue, root-cause coalescing | R4 §9 |
| `repair` | RepairPlan DAGs, safety evidence, decisions, revert | v4 §7.7; R4 §5–§6 |
| `ilrp` | Intent-Logged Repair Protocol states, driver, recovery | v4 §7.8; R4 §7 |
| `explain` | Backing for `lim jurisdiction explain`, the only internal-vocabulary surface | v4 §7.1; R4 §3 |

`repair` and `ilrp` are **modules, not separate crates**, deliberately:
R4 §4 requires that save, synchronization, merge, and Promotion all run
through ONE repair interpreter sharing the checker's Contract types.
Separate crates would invite divergent interpreters or a circular
dependency between repair execution and Contract policy; a module boundary
inside one crate keeps the single-interpreter property structural.

Dependency position (acyclic):

```text
liminal-id → {liminal-source, liminal-graph} → liminal-revision
  → liminal-jurisdiction → liminal-daemon → liminal-cli
```

Jurisdiction sits above `liminal-revision` because Contracts, repairs, and
the checker all evaluate at a Workspace Basis (Law 3D, v4 §3; v4 §7.5,
R4 §8), and below `liminal-daemon` because every deployment mode (v4 §56)
must reuse the same policy crate rather than reimplement it.

## Conformance impact

The R4 §10 gate fixtures (`conformance/tests/phase_minus_1.rs` and the
`fixtures/scenarios/` set) and the ILRP crash-matrix fixtures
(`fixtures/ilrp-crash-matrix/`) exercise this crate directly — they are the
tests of exactly the machinery it houses. **None change shape**: this RFC
relocates where the machinery lives in the crate graph; it does not alter
any observable behavior, gate assertion, or fixture format. No held-out
corpus (v4 §7.4, R4 §2.2) is affected.

## Drawbacks

- One more workspace member than the canonical §117 list, and the first
  amendment precedent — future crate additions must clear the same
  RFC bar rather than accrete silently.
- The crate is large by design (it is the complexity sink, R4 §1); its
  internal module discipline substitutes for crate boundaries and must be
  maintained by review.

## Alternatives

- **Do nothing (scatter per §117).** Rejected: fragments of ownership
  policy in `liminal-graph`, `liminal-revision`, and `liminal-daemon`
  recreate the independent-ownership-model leak v4 §7 forbids.
- **Fold into `liminal-graph`.** Rejected: the graph crate is mechanism
  (storage, operations, transactions); Jurisdiction is policy over a Basis
  and must depend on `liminal-revision`, which would invert the layering.
- **Fold into `liminal-daemon`.** Rejected: the daemon is one deployment
  shell among several (v4 §56); library-fused and process-pipeline modes
  need the same policy crate without a daemon.
- **Separate `liminal-repair` / `liminal-ilrp` crates.** Rejected: R4 §4's
  one-interpreter requirement is easiest to violate across crate
  boundaries; shared Contract types would force either a policy-free
  "types" crate (a leak magnet) or a dependency cycle.

## Unresolved questions

- Whether `explain` remains here or splits its presentation layer into
  `liminal-cli` once Phase 0 fixes the diagnostic surface (R4 §3).
- Whether the Federated perspective's merge-runtime delegation (R4 §11.7)
  eventually justifies a separate crate at Phase 6 — deferred to the RFC
  that introduces a merge runtime.
- The optimization boundary for compiled Jurisdiction plans (v4 §125)
  remains open and is untouched: this crate is interpretive until the
  Phase -1 gates pass (Law 14, v4 §3; R4 §10).
