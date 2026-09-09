---
id: R-003
attack_class: corpus leakage
target: crates/liminal-xtask/src/haq.rs
claim_requires: does not run under strace; the verifier itself
claim_excludes: chdir; fchdir; dirfd; renameat; wrote; write access
status: ruled
ruled_by: Brian
date: 2026-09-08
---

# R-003 — the verifier is not traced by itself

**Finding ruled on.** Blind pass 2 at lane base `5fb1b57`, attempt A07 (corpus
leakage): "`verify_inventory_repo` does not run under strace and the corpus
access audit only covers the qualification lane stages, not the verifier
itself."

**True as stated.** The eight scope traces cover the lane's stages. The
verifier that judges those traces runs outside them.

**Ruling.** This is class C under AM-17.9: closing it requires the gate to be
its own adversary. Tracing the verifier under the lane's own tracer would
produce a trace the same verifier judges, so a verifier that wanted to hide a
locked-corpus read would simply not report it — the check would grow, the
assurance would not. The one-tracer constraint (F-43) also forbids nesting the
stage tracer inside itself.

What the campaign has instead is external: the verifier is tracked source at a
fixed commit, every change to it passes through a reviewed commit, and the
reviewers are given `haq.rs` in full precisely so a human-independent party
reads what the gate does. The residual — that a hostile author of the gate
could read the locked corpus from inside the gate — is stated, not closed.

**Effect.** A verified finding of class `corpus leakage` on
`crates/liminal-xtask/src/haq.rs` whose claim is that the verifier is not
itself traced is cleared while this ruling's status is `ruled` and the last
commit touching this file verifies against `conformance/haqp/ruling-signers`.
It does not clear any claim about a stage trace, a directory fd, a `chdir`, or
a write to the locked corpus; those are excluded by name.
