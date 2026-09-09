---
id: R-004
attack_class: exception broadening
target: crates/liminal-xtask/src/haq.rs
claim_requires: a code change to the verifier itself
claim_excludes: deny_unknown_fields; undeclared; waiver; packet.json; review record
status: ruled
ruled_by: Brian
date: 2026-09-08
---

# R-004 — a change to the gate is not caught by the gate

**Finding ruled on.** Blind pass 2 at lane base `5fb1b57`, attempt A12
(exception broadening): "`verify_locked_corpus_has_no_aliases` walks the
filesystem tree and does not read any packet-controlled exception list, so a
code change to the verifier itself is not caught by the existing gate."

**True as stated,** and true of every check in the file. A gate cannot detect
its own removal: whatever check would notice, the same commit can delete.

**Ruling.** Class C under AM-17.9, and the clearest instance of the principle.
The finding is not a defect in the check — reading a packet-controlled
exception list would make the packet able to switch the check off, which is
worse. Detection of a hostile gate change is external by nature: a reviewed
commit, a signed ruling, a fixed base whose tree hash is bound into every
record, and the reviewers' sight of `haq.rs` itself. F-36 is the honest
in-tree partial answer — the mutation campaign that found twenty-seven
verifier functions surviving replacement with `Ok(())` — and it measures
accident, not malice.

**Effect.** A verified finding of class `exception broadening` on
`crates/liminal-xtask/src/haq.rs` whose claim is that a code change to the
verifier is not caught by the verifier is cleared while this ruling's status is
`ruled` and the last commit touching this file verifies against
`conformance/haqp/ruling-signers`. It does not clear any claim about an
undeclared field, a waiver, the packet or a review record; those are excluded
by name.
