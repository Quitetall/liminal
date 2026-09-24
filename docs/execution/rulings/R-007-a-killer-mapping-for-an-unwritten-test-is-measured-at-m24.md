---
id: R-007
attack_class: weak mutants
target: conformance/haqp/packet.json
claim_requires: killing tests
claim_excludes: disposition; killed; not-ready; anchor; coordinate
status: draft
ruled_by:
date:
---

# R-007 — whether a named killer kills its mutant is measured at M24, not asserted at M17.5

**DRAFT. Not in force until Brian sets `status: ruled`, fills `ruled_by` and
`date`, and commits it signed against `conformance/haqp/ruling-signers`.**

**Finding ruled on.** Blind pass 1 at lane base `cbed339f`, attempt A04 (weak
mutants): "Mutant at conformance/haqp/packet.json:1652 names formatter-focused
P1-T18 and P1-T19, neither of which exercises changed jurisdiction threshold
behavior."

**What is true.** P1-M042 shifts a threshold in
`crates/liminal-jurisdiction/src/checker.rs`, and its killers are M20 `lim fmt`
tests. They are derived by AM-17.10's rule from the requirement the mutant
defends, P1-R015 (D20.3: `lim fmt` replaces governed Holders atomically). That
path runs through the jurisdiction checker, which is why the mutant is mapped
there.

**Why it is not fixable in this stage.** P1-T18 and P1-T19 do not exist.
AM-17.4 defers every Phase 1 test, and ADR-0021 moves ADR-0020 §3's mutation
clauses to HAQP-1b at M24. Whether a test that is not yet written exercises a
threshold is exactly what that campaign measures, by running the mutant.
Re-mapping by judgment now would replace the derived rule with an unmeasured
guess. This is RISK-001, and the F-74 A02 precedent (RISK-007): a claim about
the strength of tests AM-17.4 says are unwritten.

**Effect.** A verified finding of class `weak mutants` on
`conformance/haqp/packet.json` whose claim concerns whether a mutant's named
killing tests exercise it is cleared while this ruling is `ruled`. It does not
clear a claim about a disposition, a kill, a `not-ready` row, or an anchor or
coordinate. Those are defects in the packet as it stands, and are excluded by
name.
