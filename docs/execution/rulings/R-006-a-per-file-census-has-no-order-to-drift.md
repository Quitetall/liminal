---
id: R-006
attack_class: nondeterminism
target: crates/liminal-xtask/src/haq.rs
claim_requires: compares counts not order
claim_excludes: spawn; thread; concurrent; pthread; async
status: ruled
ruled_by: Brian
date: 2026-09-08
---

# R-006 — a per-file census has no order to drift

**Finding ruled INCORRECT.** Blind pass 2 at lane base `5fb1b57`, attempt A06
(nondeterminism): "the durable surface measurement compares counts not order
and the scope check uses `BTreeMap` for declared surface so ordering drift is
not detected by the current gate."

**The measurement has no order to lose.** `durable_transition_sites` returns a
map from file path to the number of durable transitions that file performs, and
`crash_boundary_scope` declares the same shape. Reordering two `commit_if`
calls inside one file changes neither which files perform durable transitions
nor how many each performs, so there is no drift for the check to miss. A
`BTreeMap` is the right structure precisely because the census is a set of
(path, count) facts and its iteration order is then deterministic.

The cited coordinate does not carry the claim either: `haq.rs:2847` in that
tree is a doc comment about mutant kill attribution and Phase 1 test
quarantine.

If ordering ever becomes load-bearing — a boundary sequence whose ORDER is the
invariant, as ILRP's eight protocol steps are — that is a different surface with
a different check, and this ruling does not reach it.

**Effect.** A verified finding of class `nondeterminism` on
`crates/liminal-xtask/src/haq.rs` whose claim is that the durable census
compares counts and not order is cleared while this ruling's status is `ruled`
and the last commit touching this file verifies against
`conformance/haqp/ruling-signers`. It does not clear any claim about spawned
threads or concurrent execution.
