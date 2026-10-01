---
id: R-001
attack_class: corpus leakage
target: crates/liminal-xtask/src/haq.rs:1580
claim_requires: read access
claim_excludes: chdir; fchdir; wrote; write access; renameat; fuzz
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

**Effect.** This ruling can clear only the claim at the exact persisted target
`crates/liminal-xtask/src/haq.rs:1580`, if its class and claim phrases match.

**Scope.** This ruling answers only the persisted finding at its exact
coordinate. It cannot clear another finding in the same class or file.

**Scope narrowed again (2026-09-07, after A08 at lane `5fb1b57`).** A required
phrase can occur inside a sentence that negates it, so this ruling also lists
phrases whose presence means it does NOT answer the claim.

**In force from 2026-09-10.** Brian authorised signing on 2026-09-07; the key
registered in `conformance/haqp/ruling-signers` was not loaded in the agent
until today, so the ruling was committed unsigned and cleared nothing in the
meantime. The commit carrying this line is the signed one, and `ruling_in_force`
reads the last commit to touch this file.

**Narrowed again (2026-09-09, after A08 at lane `c29bc0ea`).** A ruling that
names a coordinate is now held to it rather than to the file, and no ruling may
clear more than one finding in a record: matching a second is evidence the
phrases are too broad, and the answer to that is a refusal.

**Narrowed again (2026-09-10, after A08 at lane `6b36bbb9`).** The ruling's own
text has always said the carved fuzz binaries keep the full any-touch rule, and
`claim_excludes` never encoded it — so the matcher would have cleared a finding
about a fuzz process reading the held-out corpus, which is leakage rather than a
stage read. `fuzz` is now an excluded phrase. The prose and the machine-readable
claim said different things for four days; the prose was right.
