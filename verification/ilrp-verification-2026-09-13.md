# ILRP slice verification: incomplete qualification

Tested source: `b5df4ae25c7e0b047770d7c9d7ce1c6bee1f9b92`, tree
`facf8501104dcc2225217bdd7e152a356992ecc1`, clean detached worktree
`/mnt/4tb/liminal-ilrp-ci-2026-09-13` at full-run start.
This verifies the StoreOwner/actor and ILRP structural-validation slices, not
completed raw-write closure, checked Basis/admission, history provenance,
finalization permits, receipts, formal proofs or phase qualification.

## Actual outcomes

All logs below live under `/mnt/4tb/liminal-formal-evidence/reviews/`.

| Check | Actual result | Log |
|---|---|---|
| Full `just ci`, native worktree target | exit 100 at nextest: 581 passed, 6 failed, 47 skipped | ilrp-full-ci-native-target.log |
| Fresh crash producer | exit 0: 8/8 boundaries, 18 injected faults | ilrp-stage-crash-producer.log |
| Independent replay of fresh crash evidence | exit 0, one test passed | ilrp-stage-crash-replay.log |
| `just test-threaded` after evidence regeneration | exit 101: five registry failures in xtask (228 other xtask tests passed); preceding packages passed | ilrp-stage-threaded.log |
| Workspace doctests | exit 0 | ilrp-stage-doctests.log |
| Rustdoc with warnings denied | exit 0 | ilrp-stage-rustdoc.log |
| `cargo deny` | exit 0 | ilrp-stage-deny.log |
| `just haq-inventory` | exit 0 | ilrp-stage-inventory.log |
| `just gates` | exit 0: 587 active / 47 deferred declarations, NOT 587 passing tests | ilrp-stage-gates.log |

Formatting, Clippy and bootstrap self-tests completed in the full CI recipe
before nextest failed. The separately run later stages do not turn the failed
recipe into a PASS. Their wrapper completed with exit 1 because threaded tests
failed. The code-bearing ILRP commit received PASS WITH NITS; the spelling fix
received PASS. These are commit reviews, not independent campaign completion.

## Remaining failures and scope

Three canary/concurrency tests reject the new direct `same-file` dependency
outside the frozen closed list. Two durable-surface/qualification refusal tests
reject the undisclosed epoch commit now in `owner.rs`. DG17.4 records the exact
two proposed registry changes for Brian; neither was made. The sixth full-run
failure was the old crash record's lockfile binding. Its actual fault matrix
passed, and producer regeneration plus independent replay cleared that failure.

Fresh `conformance/haqp/evidence/crash.json` was copied byte-for-byte from the
producer output, never hash-patched. Exactly three binding fields changed:
lockfile digest, source commit, source tree. All scenario/boundary/recovery
observations are unchanged. It remains bound to the tested source above, not
to this documentation/evidence commit. Prior bytes remain in Git history.
No golden, threshold, corpus, frozen algorithm or probe was edited.

Raw API closure, checked admission and the other approved production changes
are still incomplete. Do not launch a qualifying campaign on this candidate:
the known registry refusals prevent qualification, and remaining production
changes will require new fixed-source evidence. Phase GO, suite ratification,
formal adoption and unsigned rulings remain human-owned.

The optional independent MiMo dependency-source inventory returned
`error: parse: error decoding response body` (client exit 1); raw response is
`same-file-registry-review.json`. It supplies no review evidence for DG17.4.
The exact dependency change remains a proposal pending the human decision and
source verification; no failed model response is treated as approval.

## Resource bounds and preserved failures

One compute unit at a time; MemoryHigh=20G, MemoryMax=24G, MemorySwapMax=0,
CPUQuota=400%, TasksMax=512; Cargo, nextest and threaded-test workers capped at
four. Existing zero retries, timeout values and serialized nextest crash group
remain unchanged. Full native-target run peaked at 6,460,665,856 bytes; remaining
stages peaked at 2,532,282,368 bytes and used zero cgroup swap. No OOM occurred.

Earlier preserved failures:

- Systemd PATH omitted taplo: CI exit 127 before tests; explicit tool PATH fixed it.
- Typos rejected the owner doc comment's spelling: CI exit 2; fix committed separately.
- External CARGO_TARGET_DIR conflicted with existing harness/fuzz replay target
  discovery: nextest had 42 failures. Corrected execution profile uses the clean
  worktree's own target directory; no harness assertion or implementation changed.
  Its generated proptest regression seed is retained externally as
  `ilrp-harness-path.proptest-regressions`, not silently removed.
- An earlier main-checkout build lost a BLAKE3 native archive while target/debug
  disappeared. Cleanup actor remains unknown. Separate build storage isolated
  that interference. This is not attributed to an agent or OOM without evidence.

Post-commit review dispositions verified at source: each missing-ack loop case
creates its own store; `topo_order` checks both dependency endpoints before
ack validation; UUID ordering agrees with fixed-prefix canonical repair-key
ordering; `Any` is compared exactly, never a wildcard acceptance of a concrete
poststate. Do not remove the payload-ID check: map lookup does not validate it.

## Artifact bindings

| Artifact | SHA-256 |
|---|---|
| fresh crash.json | `215d3c6afe570b690daa170bd91135c5527fb72f933d5d8dffb2524554164c1b` |
| ilrp-full-ci-native-target.log | `5f7af687fdfa5aba5d75fdff69a118d13e880d341e63e73ea18ac3c30b0b35c1` |
| ilrp-stage-crash-producer.log | `bcd20ca9b58d7e2e61a9c231b0d0a6ad3e581abf4dd05402d78e541e0e68f491` |
| ilrp-stage-crash-replay.log | `cd873f3160497f10b1aa7fdd499665a2da8036cdb249476097a9fcac80684837` |
| ilrp-remaining-checks.tsv | `4893463357bfe3dc77d56fa1a110a12a0ce97d5615497efca8f9096bef187911` |
| ilrp-stage-threaded.log | `c799000714657fa5a3295da61e9be394b22b65d8ecb31684021ae7abc232b434` |
| ilrp-stage-doctests.log | `7053def87d36b1ea741a060e7fd1c51063b2512a65acbc5bf0bcdbd02b159cd4` |
| ilrp-stage-rustdoc.log | `1e5fe67235a2b770ef5bd5ee19459f526ede521c46ae0c78e720adf003f4d3c2` |
| ilrp-stage-deny.log | `34b3605a4357ef569f7eb15db56dfa03e63d20460c28da0efe01b5e81da78b14` |
| ilrp-stage-inventory.log | `b9707345d2db069196ec0fd786807a57a209f626e84c78ee68e6b632677a9350` |
| ilrp-stage-gates.log | `b1079e6995e672fa6647d8f4e21f108802dd4002babd40a0633b934ed0b070f1` |
