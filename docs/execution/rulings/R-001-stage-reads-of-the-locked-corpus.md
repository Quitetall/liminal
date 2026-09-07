---
id: R-001
attack_class: corpus leakage
target: crates/liminal-xtask/src/haq.rs
claim_requires: read access
claim_excludes: chdir; fchdir; wrote; write access; renameat
status: ruled
ruled_by: Brian
date: 2026-09-06
---

# R-001 — a qualification stage may read the locked corpus; it may never write it

**Finding ruled on.** Blind pass 1 at lane base `78c8f9b`, attempt A07
(corpus leakage): "stage remainder handling checks locked-corpus writes only,
so unauthorized read access remains accepted" — the same claim as F-44's
candidate option 3.

**Ruling (F-44, 2026-09-05, reaffirmed here).** For every stage's trace, a
write-class syscall on a path resolving under the repository's
`conformance/corpora` refuses unconditionally. Reads and stats are permitted:
the SLO graduation test measures the profile on the held-out corpus, which is
why it is held out, and git's index refresh and directory walkers touch every
tracked path. The carved fuzz binaries — the one process family whose evidence
is about the corpus — keep the full any-touch rule. The prohibition on
*tuning* against held-out data binds people and agents and is enforced by the
corpus manifest and its history, not by a stage trace.

**Effect.** A verified finding of class `corpus leakage` whose target file is
`crates/liminal-xtask/src/haq.rs` and whose claim is that stage reads of the
locked corpus are accepted is cleared by this ruling while its status is `ruled` and the last commit
touching this file verifies against `conformance/haqp/ruling-signers`.

**Scope (added 2026-09-06 after A08 at lane `aa00d41`).** This ruling answers
only a claim whose prose contains `read access`. Matching on attack class and
target file alone cleared an unrelated finding in the same class and file; a
ruling answers a claim, not a coordinate.

**Scope narrowed again (2026-09-07, after A08 at lane `5fb1b57`).** A required
phrase can occur inside a sentence that negates it, so this ruling also lists
phrases whose presence means it does NOT answer the claim.
