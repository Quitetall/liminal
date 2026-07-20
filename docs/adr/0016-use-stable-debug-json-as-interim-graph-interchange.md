# 0016. Use stable debug JSON as interim graph interchange

- **Status:** proposed — user acceptance required
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** v4 §§14, 121; M15; Phase 1

## Context

Phase 1 needs deterministic inspectable interchange for tests and diagnostics.
The in-memory model and migration story remain provisional. v4 §121 explicitly
warns against freezing a final binary representation before that evidence.

## Proposed decision

Use versioned stable debug JSON as the only Phase 1 graph interchange. Require
canonical field ordering, deterministic bytes, explicit schema version,
round-trip tests, malformed/unknown-field rejection, provenance, and migration
fixtures from the first persisted revision. Mark the format non-final and keep it
off performance-critical paths.

Do not choose CBOR-like snapshots, custom packed binary, or memory-mappable
layout during Phase 1.

**What would falsify or reverse this:** debug JSON cannot represent required
semantics without ambiguity, migration tests expose unmanageable compatibility,
or measured size/startup cost blocks the Phase 1 vertical slice. Any replacement
requires a new ADR plus bidirectional migration evidence.

## Consequences

- Tests and users can inspect interchange with ordinary tools.
- Phase 1 pays serialization overhead and makes no storage-performance claim.
- A later final serialization decision remains mandatory.

