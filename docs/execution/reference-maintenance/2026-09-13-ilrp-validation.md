# ILRP validation reference maintenance

Authority: AM-17.13 coordinate maintenance for the pending AM-17.12-authorized
ILRP acknowledgement/persisted-record validation slice. No qualification claim.
DG17.3's probe exception is not exercised by this change.

Old source: commit `1fb7b34a8455016a281fa6304c0a5ade797a9f0d`,
`crates/liminal-jurisdiction/src/ilrp.rs`, SHA-256
`4123755b36c786d3689e63270b582be1b96b36c654ead1011073cc4809a8926e`.
New source: same path in this record's containing commit, SHA-256
`3faa239eb6b042510539445521f5355e4983338d4a376bdec34a5cb5efcdecb7`.

All rows retain family `repair/ILRP/recovery` and requirement `P1-R015`.
Only registry line numbers and matching packet `source` fields change.

| Mutant | Operator | Killing tests (unchanged) | Old/new line | Enclosing symbol | Exact expression |
|---|---|---|---|---|---|
| P1-M048 | skipped-durable-transition | P1-T22, P1-T07 | 291 → 292 | acknowledge_step | `self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;` |
| P1-M049 | disabled-crash-point | P1-T22, P1-T07 | 290 → 291 | acknowledge_step | `self.crash.crash_if_armed(CrashPoint::BeforeAcknowledge);` |
| P1-M050 | ordering-nondeterminism | P1-T20, P1-T21 | 310 → 311 | prepare | `let graph_steps: std::collections::BTreeSet<RepairStepId> = plan` |
| P1-M051 | oracle-short-circuit | P1-T07, P1-T17 | 318 → 319 | prepare | `if graph_steps.contains(&dep.before) && after_is_external {` |

Target verification: the first two remain, in order, in-memory insertion,
BeforeAcknowledge, durable acknowledgement write, AfterAcknowledge. The new
guard precedes that sequence and refuses malformed acknowledgements; valid
acknowledgements still reach the same operations, so skipping persistence or
disabling the crash point has the same mutation effect. The prepare block's
set construction, dependency guard and refusal are unchanged; its displacement
comes entirely from an inserted line in the preceding function. No ambiguous
text match or nearest-line selection was used.

Independent pre-change review: MiMo V2.5 Pro, each of four targets PASS, bound
to both source hashes above. Raw artifact:
`/mnt/4tb/liminal-formal-evidence/reviews/ilrp-anchor-review.json`, SHA-256
`b204f04019494346a59b2b3830e878868b4d5f9e1eea47ee4676c597952d43ca`.
Primary verified the cited target lines and surrounding operations. The review's
phrase "intent skipped" means an error stops recovery, not silently continuing
past a corrupt record. No mutation kill is inferred from this review.

Old packet digest:
`06ae0546816a58537865279c71a4d2bbd5dbd019ba2b2dd8e2c1a849cdf0cfab`.
New packet digest:
`b05abd3a1fc08018c4dd9f9b3ee881d8696d5d145da4da69995104c04511d304`.
Produced by existing `cargo run -p liminal-xtask -- haq packet-digest`; only
the four `source` values and their registry coordinates change. The existing
digest mirror in `phase1-suite-review.md` follows that output. Historical
evidence is not rebound to this new packet.

Development commands ran under a systemd user cgroup with MemoryHigh=20G,
MemoryMax=24G, MemorySwapMax=0, CPUQuota=400%, TasksMax=512 and four Cargo
workers. Full CI and mutant kills are not established by these controls.
Logs below are under `/mnt/4tb/liminal-formal-evidence/reviews/`.

| Command/control | Exit | Artifact | SHA-256 |
|---|---|---|---|
| new malformed-ack test, old implementation | 101 (expected red) | ilrp-ack-red.log | `2a6fe3c1f1cc56dd078c14d25cc4672c09d4369b8d132b30facb0a2f65c6bb5d` |
| same test after guard | 0 | ilrp-ack-green.log | `7dd8d6dad41c5a2e4ef6f078ca6fc562c32a671ce0a182a0019d2364063d2e88` |
| missing-ack recovery control before validation | 101 (expected red) | ilrp-recovery-red.log | `d7f2cf60a8e9bac256b9da9f94f685fdd62af20b6457448847fea41f891806ad` |
| `cargo test -p liminal-jurisdiction --test ilrp_driver` | 0, seven tests | ilrp-recovery-green.log | `a7d3a53c559cd1721f7210f5d292be2f6959cc76d4b9fdea2549c9688c5ecaf1` |
| `cargo nextest run -p liminal-conformance --test milestones --profile ci` | 0, 29 passed / 24 skipped | ilrp-milestones.log | `beba384698d9200bcc2818eeae672788dbfb54db2a77e8b47171a72218e97b1a` |
| packet-digest, isolated target directory | 0 | ilrp-packet-digest-isolated.log | `6c837bfbc9a82015572c84d3d26518e129e373c5231e445af7980bad8658f4d9` |

Preserved infrastructure failure: initial packet-digest build returned 101 for
a missing BLAKE3 native archive. `target/debug` disappeared during inspection;
the actor responsible is unestablished. An isolated target directory built the
same source successfully, with 1.3 GiB peak memory and no swap. No source fix,
cache deletion, hidden test retry or claimed root-cause attribution followed.

`just haq-inventory` and fixed-source `just ci` results will be recorded after
their actual completion. Missing/deferred Phase 1 killing tests remain
unmeasured and ignored; this maintenance does not activate them.
