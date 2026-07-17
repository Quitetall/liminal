# Conversion-loss fixtures

**Populated by:** M10 (Pandoc adapter-boundary spike, v4 §-1.4) writes
the first real contract; Phase 4 broadens to the full converter set.
Empty until then by design.

**Spec:** v4 Law 8 (every conversion declares loss), v4 §14 (transform
contracts), v4 §107 (unknown constructs), v4 §113 (differential
conversion tests), v4 §114 (conversion-loss expectations).

## What lives here

Declared-loss contracts and the differential fixtures that hold
converters to them. Each case consists of:

- An input document (or graph fixture) exercising specific constructs.
- The conversion route (e.g. Resolved Graph IR → Pandoc AST → back).
- The **declared contract**: what the transform *requires*, *preserves*,
  *introduces*, and *may discard* (Law 8's four clauses).
- The expected differential result: the round-trip diff must fall
  entirely within the declared may-discard set. Loss outside the
  declaration is a conformance failure; so is *undeclared* preservation
  claimed by accident — the contract is the spec, in both directions.
- ID survival and foreign-node preservation observations (v4 §-1.4:
  record semantic loss, ID survival, and the exact projection
  capability level achieved).

## Consuming tests

`tests/classes/conversion_loss.rs` (differential conversion honors
declared loss contracts) and M10's
`pandoc_roundtrip_loss_report_golden`, whose byte-exact report lives at
`../../golden/pandoc_loss.md`.
