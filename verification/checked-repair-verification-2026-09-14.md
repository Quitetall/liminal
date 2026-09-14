# Checked-repair runtime verification

Runtime candidate: `51361e19c3063eb162e59069d09a6ed12c0f3eb6`, tree
`8f1b67f8a72b12f5b888e0647662c53a372b43ee`. This is implementation evidence,
not a completed HAQP campaign, formal qualification, suite ratification or GO.
Raw artifacts are under `/mnt/4tb/liminal-formal-evidence/reviews/`.

## Full CI and retained failures

Verification used the isolated checkout
`/mnt/4tb/liminal-ilrp-ci-2026-09-13`, native target directory, four build/test
workers, CPU quota 400%, MemoryHigh 20 GiB, MemoryMax 24 GiB, and no swap
allowance. Existing zero-retry and serialized crash-group settings were retained.

1. `checked-repair-51361e19-ci.log`: exit 127 before compilation because the
   systemd environment lacked the user tool directory containing `taplo`.
   SHA-256 `2dab40d0c180da99d54bdd55582b3967dd6cc942b9258ad9b9f23d5d41381438`.
2. `checked-repair-51361e19-ci-r2.log`: exit 100; nextest reported 602 passed,
   three failed, 47 skipped. The concurrency declaration still named the old
   mutex line 216 instead of 248; two canary tests consequently failed on the
   wrong diagnostic before their intended provenance check. This was not a
   runtime crash failure. SHA-256
   `c3bceefa13416e8b28749a75c93732598a7adac86969f87fe5104be013459b3d`.
3. The existing `just haq-concurrency` producer refreshed its source binding and
   exact primitive coordinates. The focused
   `haq::tests::concurrency_evidence_is_read_from_disk_not_assumed` test failed
   before this refresh (exit 101) and passed afterward (one test, exit 0).
   `just haq-canaries` regenerated the current packet-digest diagnostic.
   No expected failure, threshold, scan predicate, or test assertion changed.
4. `checked-repair-51361e19-ci-r3.log`: complete `just ci` exit 0. Nextest:
   **605 passed, 47 skipped**, 33.844 seconds for that stage. Threaded tests,
   doctests, rustdoc, deny, inventory and all 33 canaries also completed.
   SHA-256 `cb5a12689c817992e4ca2f7970e1a2041bb5f585443a49d4d740ba05db506cac`.

The successful run used fixed runtime source plus the two freshly produced
evidence files. It was not a byte-clean checkout after those producers ran.
Compared with the preceding 587/47 baseline, the runtime test delta is +18
active, with no ignored-test activation. The first conversational status read
the final failing block too narrowly and reported 604/1; the complete nextest
summary above is authoritative and corrects that report.

A later main-checkout `just fmt-check` accidentally traversed the user's
untracked nested copy and failed its formatting check. No formatter writes were
requested or made. That invocation is not a source-candidate failure or pass;
full follow-up verification remains isolated from that copy.

## Required post-commit review

Attached MCP transport refused; fresh LAMU stdio invoked the actual
`review_commit` tool against the full commit. No model was pinned.

- Default `max` artifact `checked-repair-51361e19-commit-review.jsonl` has a
  complete main PASS WITH NITS but a truncated critic. It is retained as partial
  review evidence, not a completed ensemble. SHA-256
  `4e8075b1cd883a3fc25b4020584069086e3ada39cbbb77993dd2391f12a60763`.
- A complete `fast` single-Pro review, automatically selected MiMo V2.5 Pro,
  returned PASS WITH NITS, no BUG or SECURITY findings, and process exit 0.
  `checked-repair-51361e19-commit-review-fast.jsonl`, SHA-256
  `16b957f734f4e19f2994082a4069edde9ea2ca0f86e0785698575ce8e0e185ff`.
  This satisfies the per-commit review call; it is not cross-vendor qualification.

Source-verified dispositions:

- Keep `expect_head` adding a guard to a root transaction. Its existing `None`
  means no prior guard exists, not retargeting. The suggested early return would
  ignore an explicitly requested guard (`store/mod.rs:705-711`).
- Private-field construction failure is precisely the reconciliation capability
  forgery test's intended failure, not a false compile-fail witness.
- Content keys are 64 lowercase hex bytes for current `ContentHash`; the review's
  SHA-256 label was wrong (the content hash uses BLAKE3). No algorithm changed.
- Same-generation substitution refusal and stale scoped-transaction refusal are
  intended guards. Session edits increment the generation before capture
  (`session.rs:78-87`). Hypothetical future task concurrency is not a present
  performance or correctness result.
- Full-history replay, prefix previews and file-mirror search have explicit
  linear/quadratic costs. No measured budget breach establishes an optimization
  mandate; no cache or weaker verifier was introduced.
- Root assembly intentionally possesses root authority. A read-only store view
  is not a promise that the owner itself lacks write privileges.

## Standards

Independent review identified missing canonical citations and stale owner module
documentation. The follow-up changes documentation only, preserves every source
line count and runtime body, and receives independent reference verification.
Capability wrapper repetition is intentional authority separation; no common
unrestricted writer was introduced to remove that heuristic smell.

The reviewer also identified a protocol breach: the runtime commit bundled
several implementation concerns instead of step-local commits and used a
non-work-order subject. This process failure is recorded, not retroactively
waived. History is not rewritten and no unchecked milestone is marked complete.

## Spec

Independent review identified one partial interface concern: public `run` and
`recover_all` still return status-only results, while `run_checked` returns the
receipt-aware result. Verified at `ilrp.rs:459-469,656-701` and
`daemon/workspace.rs:125-133`.

The compatibility status APIs do not mint a trusted receipt. All four daemon
accepted-repair paths use `run_checked`; `record_repair` requires a privately
constructed receipt and checks its store, exact plan and evidence
(`daemon/runner.rs:1131-1167`). Therefore this finding does not demonstrate a
bookkeeping-authority bypass. Compatibility APIs remain for existing frozen
callers and are documented explicitly; automatic follow-up bookkeeping recovery
is not claimed. A status-only recovery caller may obtain a verified receipt with
`run_checked`, including after finalization. No receipt is silently expanded to
cover the separate bookkeeping transactions.

Review totals: Standards two documented breaches (documentation corrected,
historical commit-shape breach retained) plus architectural heuristics; Spec one
interface concern with the verified compatibility boundary above. Neither axis
substitutes for the other.

## Still required

The production leaf proof core, same-executable-body proofs, complete bounded
models, qualified Linux/injected-fault adapters, formal evidence aggregation,
fresh full HAQP campaign, and applicable human adoption/phase decisions remain
open. No current-core formal obligation is marked established by these results.
