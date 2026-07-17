# Malformed-source fixtures

**Populated by:** Phase 1, when `liminal-cst` and the error-tolerant
parser land. Empty until then by design (Law 14: no parser exists in
Phase -1).

**Spec:** v4 §113 (malformed source recovery tests), v4 Part XXII
Phase 1 (error-tolerant CST), R4 §11.6.

## What lives here

Deliberately broken, truncated, mis-nested, mixed-encoding, and
adversarial source inputs, each with an expected recovery outcome:

- The parser must never panic and never reject the input outright — it
  produces an error-tolerant CST that preserves every input byte.
- Expected diagnostics (count and stable codes, not exact prose).
- Expected recovery shape: which regions parse, which are preserved as
  opaque/unknown constructs (v4 §107).

Inputs in this directory are intentionally wrong. Lint and spell-check
tooling is configured to skip `**/fixtures/**` — a "fixed" malformed
fixture is a destroyed fixture. Do not normalize, reformat, or correct
these files.

This directory also seeds the Phase 1 fuzz corpus (`cst_parse` and
`format_idempotent` targets, documented in `fuzz/README.md` when they
are created): every interesting fuzz-found input is minimized and
checked in here or in `../../corpora/regression/`.

## Consuming tests

`tests/classes/malformed_source.rs` (Phase 1: recovery never panics,
produces an error-tolerant CST).
