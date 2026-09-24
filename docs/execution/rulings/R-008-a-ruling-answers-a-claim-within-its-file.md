---
id: R-008
attack_class: exception broadening
target: crates/liminal-xtask/src/haq.rs
claim_requires: ruling target
claim_excludes: signature; signer; unsigned; draft; status
status: draft
ruled_by:
date:
---

# R-008 — a file-scoped ruling is RISK-004, disclosed, not a defect

**DRAFT. Not in force until Brian sets `status: ruled`, fills `ruled_by` and
`date`, and commits it signed against `conformance/haqp/ruling-signers`.**

**Finding ruled on.** Blind pass 1 at lane base `cbed339f`, attempt A08
(exception broadening): "Branch at crates/liminal-xtask/src/haq.rs:3166 accepts
whole-file target plus substring phrase matches, bypassing required
exact-coordinate exception scope."

**What is true.** A ruling whose `target` names a file, not a coordinate, clears
a matching claim anywhere in that file, and the claim is matched by required
and excluded phrases. That is RISK-004 verbatim: "a standing ruling can clear a
finding it does not answer: whether two sentences describe one claim is not
decidable from the sentences." AM-17.15 keeps RISK-004 recorded, because it is
the qualifier reasoning about its own findings.

**Why file scope is intended.** R-001..R-006 rule on classes of claim that
recur anywhere in `haq.rs` (stage reads of the corpus, a gate not catching its
own change), and a coordinate goes stale with every edit to the file. The
existing mitigations stay: a coordinate-scoped ruling is held to its
coordinate, a ruling clears at most one attempt per record, and excluded
phrases carve out neighbouring claims. Requiring coordinates for every ruling
would retire R-001..R-006 and force each to be re-signed at a coordinate that
moves.

**Effect.** A verified finding of class `exception broadening` on
`crates/liminal-xtask/src/haq.rs` whose claim is that a ruling's file-scoped
target and phrase matching clear claims outside its intended scope is cleared
while this ruling is `ruled`. It does not clear a claim about signatures,
signers, drafts or status. Those are the controls that keep a ruling Brian's,
and are excluded by name.
