---
id: R-005
attack_class: vacuity
target: crates/liminal-xtask/src/haq.rs
claim_requires: bails before; verify_packet_shape
claim_excludes: constant; oracle; formatter; canonical-reparse
status: ruled
ruled_by: Brian
date: 2026-09-08
---

# R-005 — the qualified path does check packet shape, and the doctored-tree test exists

**Finding ruled INCORRECT.** Blind pass 2 at lane base `5fb1b57`, attempt A09
(vacuity): "the committed packet is not-run so `verify_qualified_repo` bails
before reaching `verify_packet_shape`, and the inventory gate test only asserts
acceptance of the committed tree, not rejection of a doctored one."

**Both halves are false in the tree the reviewer read.** At `5fb1b576`:

- `verify_qualified_repo` reads the packet and then calls `verify_packet_shape`
  as its first check. `read_packet` reads and deserializes; it has no early
  return on `qualification_state`. Nothing bails before the shape check.
- `the_inventory_gate_refuses_a_doctored_tree` is present in that same file. It
  copies the packet, the review markdown and the oracle source into a scratch
  tree, doctors them, and requires the gate to refuse. Acceptance of the
  committed tree is a separate test, and both exist.

This is the reviewer's documented ~20-30% false-positive rate, not a defect.
Under grilling decision 6 (2026-09-03) a finding ruled incorrect is cleared by
a signed ruling, which is what this is — not a set-aside of a true finding.

**Effect.** A verified finding of class `vacuity` on
`crates/liminal-xtask/src/haq.rs` whose claim is that the qualified path bails
before `verify_packet_shape` is cleared while this ruling's status is `ruled`
and the last commit touching this file verifies against
`conformance/haqp/ruling-signers`. It does not clear any claim about a
constant-returning formatter, an oracle, or the canonical-reparse relation.
