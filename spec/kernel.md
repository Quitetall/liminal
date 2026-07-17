# Kernel — semantic kernel, laws, and Jurisdiction

**Scope.** The semantic kernel (Node and Relation, everything else derived),
the architectural constitution (goals, non-goals, laws), Jurisdiction as the
deliberate complexity sink — Contracts, Workspace Basis, RepairPlans, ILRP,
the checker, Overlay lifecycle — the editing and projection modes, the
physical representation that keeps the two-primitive model fast, and the
identity model. This is the part of the spec the Phase -1 falsification
laboratory exists to attack.

This file is a curated index into the canonical text in `spec/v4/`. On any
conflict the canonical text wins. Citation forms: `v4 §N`, `R4 §N`.

## Part I — Architectural constitution

- **Goals** — what Liminal is for: a local-first, multimodal, revisioned
  graph runtime under one semantic model. v4 §1.
- **Non-goals** — what Liminal deliberately refuses to be. v4 §2.
- **Architectural laws** — the sixteen laws plus the 3A–3J Jurisdiction
  corollaries; every crate and test cites the law it serves. v4 §3.
  - Law 1 — only Nodes and Relations are semantically fundamental.
  - Law 2 / 2A — semantic minimalism ≠ physical uniformity; a tiny kernel
    does not make the total system simple.
  - Law 3 — Jurisdiction resolves every independently governable semantic
    address at a Workspace Basis; corollaries 3A (no accidental
    Jurisdiction), 3B (capture is never rejected), 3C (no silent acceptance
    or Holder change), 3D (one immutable Workspace Basis per computation),
    3E (zero authoring, zero sound-state noise), 3F (cross-Jurisdiction
    repair is mutation-local), 3G (Promotion is repair; determinism is not
    safety), 3H (cross-Holder repair is intent-logged, ordered, idempotent,
    resumable), 3I (Overlay debt remains visible), 3J (independent dirty
    buffers never form a chimeric Basis).
  - Law 4 — personalization may change representation, never interpretation.
  - Law 5 — effects are explicit.
  - Law 6 — abstraction towers disappear before hot execution.
  - Law 7 — existing tools may hold Jurisdiction, not merely integrate.
  - Law 8 — every conversion declares loss.
  - Law 9 — offline is a normal operating condition.
  - Law 10 — human and AI projections are separate compiler targets.
  - Law 11 — Unix modularity is an interface property, not process overhead.
  - Law 12 — identity strength must match Relation durability.
  - Law 13 — logical entity identity ≠ immutable version identity.
  - Law 14 — risk-retirement order precedes dependency order (the law that
    mandates Phase -1 and forbids optimization now).
  - Law 15 — generated information carries provenance.
  - Law 16 — the system remains recoverable without its richest runtime.

## Part II — The semantic kernel

- **Node** — an identifiable unit of state; `kind`/`revision`/`flags` are
  physical fast paths, not extra semantics. v4 §4; identity policy §4.1,
  payload storage §4.2, logical-versus-physical granularity §4.3.
- **Relation** — a typed association, dependency, order, invariant, or
  contract between Nodes. v4 §5; binary edges with reified n-ary
  relationships §5.1, physically privileged containment and order §5.2,
  revision-aware anchored Relations §5.3, Relation as maintained invariant
  §5.4.
- **Everything else is derived** — Document, Workspace, Text, Resource,
  Schema, Transformation, Macro, Compiler, Formatter, View, History, and
  Synchronization are all constructions over Nodes and Relations, never new
  primitives. v4 §6.

## Part III — Jurisdiction, source, identity, and projections

- **Jurisdiction is the deliberate complexity sink** — ownership, write
  routing, identity, external edits, and replication get one typed home; no
  other subsystem may create an independent ownership model. v4 §7.
- **Vocabulary tiers** — everyday draft/save/sync language for users;
  Jurisdiction/Holder/Overlay/Promotion for advanced surfaces; Contract and
  Basis vocabulary confined to `lim jurisdiction explain` and maintainer
  docs. v4 §7.1; R4 §3.
- **Subjects and the reduction of facets** — Jurisdiction governs Nodes and
  Relations; there is no third subject. v4 §7.2.
- **Jurisdiction Contracts** — internal policy data (scope, resolution,
  mutation, continuity, lifecycle), authored by profiles, never common-case
  user authoring. v4 §7.3.
- **Profile coverage and ergonomics conformance** — coverage is measured
  against trace denominators, not asserted. v4 §7.4; R4 §2.
- **Workspace Basis and Perspectives** — every computation reads one
  immutable Basis; ClientScoped / DurableOnly / Published / Federated
  perspectives keep concurrent working state from becoming a chimera.
  v4 §7.5; R4 §8.
- **Standard Jurisdiction profiles** — the profile catalogue; Phase -1
  implements exactly two (external-file, graph-native). v4 §7.6; R4 §10.
- **RepairPlans, Promotion, and safety** — repairs are dependency DAGs of
  mutation-local proposals; Promotion is the user-facing name for a
  successful repair; determinism is not safety, so unique-but-unsafe
  candidates are never auto-accepted. v4 §7.7; R4 §4, §5, §6.
- **Intent-Logged Repair Protocol (ILRP)** — Prepare / Apply / Acknowledge /
  Finalize / Recover; cross-Holder work is intent-logged, idempotent, and
  resumable after a crash at every durable boundary. v4 §7.8; R4 §7.
- **Jurisdiction Checker** — the eight questions; a sound workspace produces
  zero output. v4 §7.9.
- **Overlay lifecycle and Jurisdiction debt** — an unaccepted draft is a
  durable Overlay that stays visible until repaired, discarded, or archived;
  the Reconciliation Queue precedes any agenda subsystem. v4 §7.10; R4 §9.
- **Initial domain Jurisdiction matrix** — which Holder governs which domain
  at the start. v4 §7.11.
- **Editing and projection modes** — portable file mode §8.1, managed
  textual projection §8.2, rich graph mode §8.3, projection capability
  levels §8.4, exact source preservation and foreign edits §8.5, the pure
  incremental engine and effect reactor §8.6. v4 §8.

## Identity model

- **Entity, version, anchor, alias** — four distinct concepts; content
  hashes identify versions, EntityIds provide lineage. v4 §19.
- **Identity grades** — Ephemeral / Anchored / Inferred / Explicit /
  Managed / External / Content-addressed; Relations declare the minimum
  grade they require. v4 §19.1.
- **No magical round-trip guarantee** — logical identity under arbitrary
  foreign edits is information-theoretically unavailable without Holder
  cooperation; heuristics are recovery aids, never proof. v4 §19.2.
- **Immutable versions and lineage** — content-addressed VersionIds with
  visible, reviewable inferred continuity. v4 §19.3.

See `spec/syntax.md` for how identity appears in surface syntax.

## Part IX — Physical representation

- **Logical graph, specialized stores** — one uniform logical graph over
  many physical stores. v4 §43.
- **Node layout** — typed columns and compact headers instead of universal
  string-keyed maps; 128-bit persistent IDs map to dense handles. v4 §44.
- **Relation layout** — each relation family compiles to its own structure
  (sequences, adjacency, inverted indexes, interval trees, …). v4 §45.
- **Text subsystem** — text is physically privileged; individual characters
  are never ordinary graph allocations. v4 §46.
- **Resources** — bulk payloads live in a content-addressed object store.
  v4 §47.
- **Zero-cost abstraction definition** — what "zero cost" does and does not
  promise. v4 §48.
- **JIT and Cranelift** — reserved for demonstrated repeated computation.
  v4 §49.
- **Memory and startup** — skeleton-first loading, deferred decoding,
  profile-compiled binaries. v4 §50.

Phase -1 note: everything in Part IX is *vocabulary*, not license to build —
Law 14 (v4 §3, Part XXII) forbids optimization until the interpretive
reference semantics survive the falsification gates (R4 §10).

Status: index only — becomes a self-contained chapter at Phase 0.
