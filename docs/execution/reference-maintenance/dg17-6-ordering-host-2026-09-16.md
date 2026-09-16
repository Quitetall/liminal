# DG17.6 ordering-host coordinate maintenance

Authority: AM-17.13, with the separately approved DG17.6/AM-17.12 host
proof-buffer integration pending in this same change. The user requested commit
and merge of the latest development slice. This is neither qualification nor
an amendment to the frozen measuring instrument's behavior.

## Exact source identity

Old source commit: `6331ef7be0a300e9d7ddce54ca8167caf9876e80`.
All paths below are `crates/liminal-jurisdiction/src/ilrp.rs`.
Old file SHA-256:
`287105c022110e555005f0b90db49c5961e8100985c2bc70c409abc03ff4c567`.
New file SHA-256:
`174c5cb9a1ff6941172643f75c084114414b28405c538c731e4f0990a966cce8`.
The new revision is the commit containing this record, not a self-embedded hash.

All five mutants retain family `repair/ILRP/recovery`, their defect labels,
dispositions, operators, requirements and killing-test mappings.

| Mutant | Old/new line | Enclosing symbol | Exact expression | Operator | Requirement | Killing tests |
| --- | --- | --- | --- | --- | --- | --- |
| P1-M045 | 575 / 578 | `IlrpDriver::commit_intent` | `Ok(())` | success-error-substitution | P1-R016 | P1-T17, P1-T18 |
| P1-M048 | 589 / 592 | `IlrpDriver::acknowledge_step` | `self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;` | skipped-durable-transition | P1-R015 | P1-T22, P1-T07 |
| P1-M049 | 588 / 591 | `IlrpDriver::acknowledge_step` | `self.crash.crash_if_armed(CrashPoint::BeforeAcknowledge);` | disabled-crash-point | P1-R015 | P1-T22, P1-T07 |
| P1-M050 | 611 / 617 | `IlrpDriver::prepare` | `let graph_steps: std::collections::BTreeSet<RepairStepId> = plan` | ordering-nondeterminism | P1-R015 | P1-T20, P1-T21 |
| P1-M051 | 619 / 625 | `IlrpDriver::prepare` | `if graph_steps.contains(&dep.before) && after_is_external {` | oracle-short-circuit | P1-R015 | P1-T07, P1-T17 |

## Behavioral check and independent review

The complete `commit_intent` and `acknowledge_step` bodies are unchanged.
M045 still substitutes the same post-commit success result; M048 removes the
same acknowledgement-persistence call; M049 disables the same pre-ack hook.
The graph-step collection and dependency-refusal block in Prepare are unchanged:
M050 still changes the ordered collection construction and M051 still
short-circuits the same conjunction. The separately authorized production
changes add ordering validation; this record does not claim that entire-driver
behavior, mutant reachability, or measured kill outcomes are unchanged.
The unchanged registry/operator validators at `haq.rs:9919-9936` and
`haq.rs:10032-10076` were inspected; no operator implementation was edited.
M044 at line 178 and M052 at line 226 require no coordinate change. The three
ILRP durable-call census sites are unchanged; no crash-scope count was edited.

Before changing references, actual LAMU `review_diff` independently inspected
both numbered source versions, their hashes, packet mappings, and the complete
new host/harness source. The service selected MiMo V2.5 Pro; primary verdict
PASS WITH NITS, with explicit PASS for each proposed move. The critic also
verified each coordinate, but its later general-code analysis was truncated;
no complete independent critic verdict is claimed. Source hashes stayed stable
during review and the client exited 0.

Durable review artifacts under `/mnt/4tb/liminal-formal-evidence/reviews/`:

| Artifact | SHA-256 |
| --- | --- |
| `order-host-merge-review-r1.request.json` | `1f5a354c38c193049af4fd1e782f6e3f43ad59a069cbe21711754123c9bf9848` |
| `order-host-merge-review-r1.result.json` | `2b12af7c7d419607b5cdda89dda4761cea7f7a14a9a646c45667eb9e1fc35787` |

Findings were checked at cited source. The redundant illegal-state match arm
retains old behavior and needs no lint suppression. Checking realloc's requested
new size is intentional; its old layout is not the allocation requested. The
iterator guard executes before each push, so an extra element returns an error
without growing the buffer; it does not silently accept overflow. No production
fix was warranted. The control remains explicitly layout-specific and graph-only.

## Packet and verification boundary

Old canonical packet digest:
`0e439fe364210f0dd22a1121b9f4d7bc717a7a93933202679197e728397242b9`.
New canonical packet digest:
`d5ff711d0dfe95005bddda64c1d2f7e78c30d2a7293e156b06423c87e32c4a17`.
The existing `cargo run --locked --offline -p liminal-xtask -- haq packet-digest`
command exited 0. Only the five packet `source` fields, their registry line
numbers, and the digest mirror in `phase1-suite-review.md` changed. No golden,
mapping, threshold, assertion or qualification policy changed. Historical evidence
remains in Git history. The existing CI producer regenerated `canaries.json`;
its only diff is the new packet digest inside the observed missing-digest
diagnostic. Neither the expected diagnostic nor the canary assertion changed.

The initial inventory command exited 1 at M045's stale coordinate, as expected;
its raw output is preserved as `order-host-merge-inventory-before.{stdout,stderr}`.
Replacement verification is recorded below. All named Phase 1
killing tests remain deferred/unmeasured; inventory validity is not a mutant kill.

## Completed development checks

The bounded service `liminal-host-merge-checks-r1` exited 0 in 8m08.036s,
with a 2-CPU quota, 4-GiB memory limit, zero permitted swap, 256-task limit and
1200-second runtime limit. Observed memory peak was 4 GiB and swap was zero.
The executable script and per-command logs are retained under the review root.

| Command | Actual exit | Result |
| --- | --- | --- |
| `cargo test --locked --offline -p liminal-jurisdiction` | 0 | 31 tests and one doctest passed |
| `cargo run --locked --offline --manifest-path verification/resource-controls/Cargo.toml --target-dir target` | 0 | Admission, Prepare and four nonterminal recovery-state controls passed |
| `cargo clippy --locked --offline --manifest-path verification/resource-controls/Cargo.toml --target-dir target --all-targets -- -D warnings` | 0 | Standalone harness lint passed |
| `just ci` | 0 | 630 nextest cases passed, 47 skipped; threaded tests, doctests, docs, lint and dependency policy passed; 22 bootstrap and 142 proof-support controls passed |
| `just haq-inventory` (within full CI) | 0 | Rebound inventory accepted |
| `just haq-canaries` (within full CI) | 0 | All 33 canaries caught |
| `just gates` | 0 | 630 active, 47 deferred, including two Phase 0 M17 gates |

No assertion or timeout was weakened. The sanitizer replay test completed in
97.983s after a SLOW notice; that notice was not a failure. All active workspace
tests ran; deferred named killing tests were not un-ignored. No full HAQP lane,
fixed-source proof qualification or Phase 0 closeout is claimed.

| Artifact under the review root | SHA-256 |
| --- | --- |
| `order-host-merge-inventory-before.stderr` | `a74baea2855c374511f5fab522251dc33e273bb5cad6c6aede06b56f1b048e40` |
| `order-host-merge-digest.stdout` | `0a9d99294c8d1eb93560860a79515139c5e6db6cb59237c1b0d0297d0a393686` |
| `order-host-merge-full-ci.stdout` | `d421bb2e8ae06bd59dd48bf39e497759fc3daaef450bc430f74877970586650e` |
| `order-host-merge-full-ci.stderr` | `aacaa2b3d53e567d60a2a2fb667c488bb425a67f3316d7fe606d9c826ec7e0d7` |
| `order-host-merge-checks-r1.stdout` | `3d158f9e35f11d8cde9e280fe322ec8ead549a163d572d431a554c0e9c2f98d8` |
| `order-host-merge-checks-r1.stderr` | `9747d6b5fa2aa1c8ee30019ab7d32f23810ad07c0e0948a584879aa05e1176e0` |
| `order-host-merge-resource.stdout` | `a7e58ada1fc60944474077f64a3cf746c4e314be2c1625e58472863adf47134e` |
| `order-host-merge-resource.stderr` | `55c578e3ed975263bb3ace110e5fc58e76191f06c90fc3feb1d898dadb6dc225` |
| `order-host-merge-jurisdiction.stdout` | `8f629cf63e082950b7707fe8bca73212a980ebdd5f4cd5928103e82a0d7f269b` |
| `order-host-merge-resource-lint.stderr` | `4ec808d13cba03019af741cd79a28763831f60577b3427f15e2e299c4c3ebaac` |
| `order-host-merge-gates.stdout` | `353f31692a2628df623d84e37977ab321a6e67079db600ed6e6dfe2e3c839548` |
