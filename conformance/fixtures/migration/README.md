# Migration fixtures

**Populated by:** the first persisted-format ADR — the moment any
on-disk format (log record, snapshot, content-addressed object, intent
log, Overlay store) is declared stable enough to need versioning. Empty
until then by design; Phase -1 formats are explicitly provisional.

**Spec:** v4 §113 (snapshot migration tests), v4 §115 (migration
compatibility tests), v4 Law 16 (the system remains recoverable without
its richest runtime).

## What lives here

Verbatim byte captures of every persisted format version ever shipped,
with expected post-migration state:

- `v<N>/` — pristine stores written by version N. These bytes are
  **frozen forever**: once a format version has existed in a release,
  its fixture is never regenerated, "cleaned up", or re-encoded, because
  the fixture *is* the compatibility promise.
- For each old version: the expected logical content after migration to
  the current version, and the expected round-trip result where
  downgrade paths are declared.
- Refusal cases: stores from unknown future versions must be rejected
  loudly, never partially migrated or silently reinterpreted.

Migration must read every prior version. Dropping support for an old
format version requires its own ADR and a documented offline conversion
path.

## Consuming tests

`tests/classes/migration.rs` (snapshot/format migration round-trip).
