---
id: R-002
attack_class: vacuity
target: crates/liminal-xtask/src/haq.rs:142
claim_requires: canonical-reparse relation
claim_excludes: mutation; mutant
status: ruled
ruled_by: Brian
date: 2026-09-06
---

# R-002 — the packet declares the canonical reparse relation

**Finding ruled on.** Blind pass 1 at lane base `78c8f9b`, attempt A01
(vacuity): "constant output remains idempotent, and the packet declares no
semantic canonical-reparse relation capable of detecting lost meaning".

**Ruling.** The packet declares `P1-T02 laws::canonical_round_trip_law_holds`
and `P1-T04 classes::malformed_source::malformed_source_never_panics_and_round_trips`,
both of which assert `parse(emit(parse(x))) == parse(x)` through
`Phase1Document` equality, which since F-45 (A02) compares the derived graph
and the HIR with source positions removed. A formatter emitting constant
output fails both for any source with more than one paragraph. Idempotence
(`P1-T01`) is not the only declared relation; the finding's premise is false
against the committed packet.

**Effect.** This ruling can clear only the claim at the exact persisted target
`crates/liminal-xtask/src/haq.rs:142`, if its class and claim phrases match.

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
