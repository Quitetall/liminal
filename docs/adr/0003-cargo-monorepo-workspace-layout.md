# 0003. One Cargo monorepo workspace laid out per v4 §117

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** v4 §117 (monorepo structure), RFC-0001
  (`spec/rfc/0001-add-liminal-jurisdiction-crate.md`), root `Cargo.toml`

## Context

v4 §117 prescribes the repository shape: a `crates/` tier of 21 kernel and
runtime crates, plus `domains/`, `backends/`, `integrations/`, `apps/`,
`conformance/`, and `benches/`, and closes with "Start in one monorepo.
Split only when release cadence, ownership, or build cost justifies it."
The scaffold must materialize this shape now so that every future phase has
a named home, while keeping Phase -1 the only place with real code (Law 14).

One deviation from the §117 list is required: Jurisdiction — the deliberate
complexity sink (v4 §7, R4 §§4–9) — needs a typed home of its own rather
than being smeared across `liminal-graph` and `liminal-daemon`. That is a
spec-observable amendment to the §117 crate list, so it goes through the
RFC process (RFC-0001), not this ADR; this ADR merely adopts the result.

## Decision

We will use **one Cargo workspace** with `members = ["crates/*",
"conformance", "benches"]`, containing the §117 crate list plus
`liminal-jurisdiction` per RFC-0001 (24 members total). The
`domains/`, `backends/`, `integrations/`, and `apps/` tiers exist today as
README placeholders only, and each joins the workspace members list when
its first real crate lands (Phase 5+/9+ per Part XXII). Repositories are
split out only when release cadence, ownership, or build cost justifies it,
exactly as §117 states — and such a split requires a superseding ADR.

**What would falsify or reverse this:** workspace build cost or lockfile
contention materially slowing the falsification loop (e.g., cold builds
dominating iteration time), or a component acquiring an independent release
cadence or owner — §117's own split triggers, recorded here as the reversal
condition.

## Consequences

- **Positive:** one lockfile, one toolchain pin, one CI pipeline, atomic
  cross-crate refactors while the crate boundaries are still provisional;
  the multi-year §117 shape is visible in the tree from day one.
- **Negative:** 24 members compile even though most are reserved stubs;
  placeholder tiers can look like abandoned directories to a casual reader
  (their READMEs say otherwise).
- **Follow-ups:** add `domains/*` etc. to `members` when their first crate
  lands; revisit crate boundaries at the Phase -1 exit gate — the layering
  `id → {source, graph} → revision → jurisdiction → daemon → cli` is
  provisional until the lab validates it.

## Alternatives considered

- **Multiple repositories from the start** — rejected: §117 says start in
  one monorepo; pre-Phase-0 crate boundaries will move, and cross-repo
  refactoring plus version coordination is pure overhead with zero
  consumers.
- **Minimal workspace (only the Phase -1 crates), add the rest later** —
  rejected: the reserved stubs cost little to carry, give every spec Part a
  citable home for doc comments and law-trait seams, and prevent §117
  drift by making the target shape concrete now.
- **Fold Jurisdiction into liminal-graph or liminal-daemon** — rejected:
  the complexity sink smeared across two crates is the failure mode R4 §4
  warns about (multiple repair interpreters); one typed home is the point.
  Decided via RFC-0001 because it amends the spec's crate list.
