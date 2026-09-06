---
id: R-002
attack_class: vacuity
target: conformance/haqp/packet.json
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

**Effect.** A verified finding of class `vacuity` whose target file is
`conformance/haqp/packet.json` and whose claim is that no canonical-reparse
relation is declared is cleared by this ruling while its status is `ruled` and the last commit
touching this file verifies against `conformance/haqp/ruling-signers`.
