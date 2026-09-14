# Independent StoreOwner coordinate audit

Reviewer: `/root/reference_anchor_audit`, GPT-5.6 Sol, read-only worker.
Report received 2026-09-14T05:26:13Z. This preserves the report's findings;
it is not a human signature, test execution, or qualification verdict.

Baseline: `f11e130c19f8717811e9c9396a6bc7f993ce7af5`.

| Source | Baseline SHA-256 | Reviewed SHA-256 |
|---|---|---|
| crates/liminal-graph/src/store/mod.rs | `6098de85457d51b86b443d4e3295e6d473b37472daf8f7bc7f2347a086f00d0e` | `16ff70c6b0571f0fb3e0432e929381c5f1b7347be2159733d06627dc5bd37866` |
| crates/liminal-jurisdiction/src/ilrp.rs | `3faa239eb6b042510539445521f5355e4983338d4a376bdec34a5cb5efcdecb7` | `6f8e42566b5a637cff8f643e970839ce522604cf4e021521508b0c8d547a829c` |
| crates/liminal-jurisdiction/src/admission.rs | new file | `d4d8094bac267a974a611f8ebc137e0c893d29e54502b8da3fd532a1d2008cdb` |
| crates/liminal-xtask/src/haq.rs | `69c10e4d54a20f7498291466daf9151cbd54119bc4a84c450fe593e85b996cc7` | `c9cc641c4392a84bb75182d960dd4be94e62cbba1bb65a8295ea4582d5647539` |

The reviewed `haq.rs` hash includes DG17.3 probe setup but precedes coordinate
edits. Graph and ILRP hashes bind the actual target bodies. The primary agent
checked these hashes and old/new source-line equality before updating references.

## Coordinate/source-transformation identity

Every expression below is unchanged in the same enclosing symbol. G denotes
`crates/liminal-graph/src/store/mod.rs`; I denotes
`crates/liminal-jurisdiction/src/ilrp.rs`.

| Mutant | Source old → new | Enclosing symbol/arm | Exact expression |
|---|---|---|---|
| P1-M017 | G:100 → G:127 | State::apply / SetPayload | `node.revision.0 += 1;` |
| P1-M018 | G:81 → G:108 | State::apply | `match op {` |
| P1-M019 | G:302 → G:334 | GraphStore::head | `Ok(self.lock()?.state.head)` |
| P1-M021 | G:320 → G:440 | GraphStore::node_at | `return Ok(inner.state.nodes.get(&id).cloned());` |
| P1-M022 | G:475 → G:629 | GraphStore::commit_txn | `inner.log.append(&record)?;` |
| P1-M023 | G:122 → G:149 | State::apply / RetargetRelation | `rel.revision.0 += 1;` |
| P1-M024 | G:160 → G:187 | State::apply / AttachResource | `n.revision.0 += 1;` |
| P1-M025 | G:186 → G:213 | State::apply_commit | `match &aux.value {` |
| P1-M026 | G:84 → G:111 | State::apply / CreateNode | `return Err(StoreError::Conflict(format!("node exists: {}", node.id)));` |
| P1-M045 | I:278 → I:575 | IlrpDriver::commit_intent | `Ok(())` |
| P1-M048 | I:292 → I:589 | IlrpDriver::acknowledge_step | `self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;` |
| P1-M049 | I:291 → I:588 | IlrpDriver::acknowledge_step | `self.crash.crash_if_armed(CrashPoint::BeforeAcknowledge);` |
| P1-M050 | I:311 → I:611 | IlrpDriver::prepare | `let graph_steps: std::collections::BTreeSet<RepairStepId> = plan` |
| P1-M051 | I:319 → I:619 | IlrpDriver::prepare | `if graph_steps.contains(&dep.before) && after_is_external {` |

P1-M044 remains I:178 (`CrashPoint::name`, `match self {`); P1-M052 remains
I:226 (CrashInjector delegation, `(*self).crash_if_armed(at);`). Repeated
expressions at other coordinates are not interchangeable targets.

## Behavior and remaining risk

Coordinate identity and the source transformation are established. Runtime
kill preservation is **UNKNOWN**. The packet's affected deferred mutants have
no declared runnable patch or measured kill; this audit cannot create either.

- P1-M050/P1-M051 may be redundant under the earlier DAG checks in
  `admission.rs:151` and `:214–222`. This is an equivalent-mutant/survivor risk,
  not a certified equivalence. M050's set is used for membership, not iteration,
  so part of that concern predates this migration.
- P1-M048 still removes the same acknowledgement commit call, but the new
  shared progress validator can change how the defect is detected downstream.
  An unchanged source mutation is not proof of an unchanged kill mechanism.
- P1-M045 still substitutes the success return after a durable intent commit;
  the surrounding admission/history preconditions are stronger.
- Other moved targets retain their exact operation/dispatch semantics. This
  does not establish equality of the entire old and hardened programs.
- P1-M044's pre-existing defect label describes a different behavior from its
  CrashPoint-name target. Coordinate maintenance does not repair that mismatch.

Concrete patch construction and baseline/current executions remain required
before claiming a measured kill, equivalence, or M24 qualification. Preserve
the IDs and mappings; do not contrive a new target to manufacture a kill.

## DG17.3 probe

The probe diff is confined to trusted fixture/admission setup: StoreOwner,
FILE/blob setup, concrete file pre/poststates and Basis, driver construction,
and Checker authorization. Existing Committed assertion, empty recovery
assertion, error text, and `Committed:8` output expression remain unchanged.
The independent audit did not run the probe. Runtime golden verification is
recorded separately, not inferred from this static inspection.
