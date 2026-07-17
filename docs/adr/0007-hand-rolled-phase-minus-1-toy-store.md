# 0007. Hand-roll the Phase -1 toy store per v4 §92 instead of embedding a database

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** v4 §92 (crash consistency), §7.8 / R4 §7 (ILRP), §113
  (crash-recovery test class), §121/§122 (production store selection —
  explicitly a later ADR); Part XXII -1.0 (crash-boundary exit gate);
  `crates/liminal-graph/src/store/`

## Context

v4 §92 defines the durability contract: append operations atomically,
checksummed segments, snapshots via atomic replacement, recovery after
interrupted writes — and the graph store is the ILRP coordinator because it
can transactionally retain intent and progress (§7.8, R4 §7). The Phase
-1.0 exit gate kills the daemon after **every** ILRP durable boundary and
requires recovery to `Committed`, `NeedsReview`, or `Aborted` with no
hidden half-state.

That crash matrix must test **Liminal's own fsync discipline** — which
fsync call establishes which durable boundary. Behind redb, rusqlite, or
fjall, the durable boundary belongs to the vendor's WAL: a passing matrix
would certify their crash consistency, not ours, and the coordinator
semantics §92 makes load-bearing would go unexercised. The falsification
workflow also wants a store you can `grep`, `diff`, and hand-corrupt.

## Decision

We will hand-roll the Phase -1 toy store, implementing §92's requirements
directly and nothing more: a **checksummed append-only NDJSON log**
(one record per line, crc32-framed; recovery truncates to the longest
checksummed prefix) plus **atomic snapshot replacement** (`tmp → fsync →
rename → fsync(dir)`), with `append()` returning only after fsync so the
ILRP fault points sit immediately after known durable boundaries. The store
is **explicitly throwaway**: private to `liminal-graph::store`, no public
API commitment, no GC, no performance work. §92 is the production
requirement anyway, so this code doubles as the reference articulation of
that contract. Production store selection is a separate, later ADR per
v4 §121/§122 — this ADR decides nothing beyond Phase -1.

**FALLBACK TRIGGER (recorded now, before sunk cost accrues):** if the toy
store exceeds **~1 week of M1**, we switch to rusqlite and accept that the
§92 checksummed-segment portion of the crash gate is deferred — the ILRP
state-machine recovery tests still run, but the which-fsync-is-which-
boundary evidence waits for the production store ADR.

**What would falsify or reverse this:** the fallback trigger firing; or the
crash matrix revealing that honest fsync discipline is hard enough that
Phase -1 should test against a vendor WAL after all — either way the
superseding ADR must state which §92 obligations went untested.

## Consequences

- **Positive:** the -1.0 crash matrix tests the actual contract §92 imposes
  on the real system; every durable boundary is a line of our code with a
  fault point after it; store state is human-inspectable during debugging;
  corruption tests (§113) can bit-flip real records.
- **Negative:** we own recovery bugs a vendor already fixed; NDJSON + full
  snapshots are slow and large (irrelevant — Law 14 forbids optimization,
  and benches record baselines only); risk of accidental attachment to
  throwaway code (mitigated: module is private, this ADR labels it).
- **Follow-ups:** production store ADR at the §121/§122 decision point,
  informed by what the toy's crash matrix taught; if the fallback fires,
  record the deferred gate portion in that ADR.

## Alternatives considered

- **redb** — rejected: its transaction durability replaces ours; the crash
  matrix would validate redb's commit path, leaving the §92 coordinator
  discipline — the thing Phase -1 exists to falsify — untested.
- **rusqlite** — rejected as the primary for the same WAL-ownership reason,
  but retained as the named fallback: it is the fastest honest retreat that
  keeps transactional intent-retention (§92's coordinator requirement) if
  the hand-rolled store overruns.
- **fjall** — rejected: an LSM engine solves compaction and throughput
  problems Phase -1 does not have, while still owning the durable boundary
  we need to own.
- **In-memory store with mocked durability** — rejected outright: R4 §7.5
  recovery semantics against pretend fsyncs is exactly the "hidden
  half-state" theater the exit gate exists to catch.
