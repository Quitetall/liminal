---
id: R-004
attack_class: exception broadening
target: crates/liminal-xtask/src/haq.rs:1
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

**Effect.** This ruling can clear only the claim at the exact persisted target
`crates/liminal-xtask/src/haq.rs:1`, if its class and claim phrases match. It
does not clear another claim, an undeclared field, a waiver, the packet or a
review record.
