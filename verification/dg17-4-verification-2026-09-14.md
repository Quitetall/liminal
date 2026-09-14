# DG17.4 verification — 2026-09-14

Scope: the conditional registry maintenance approved in `6451c0c6`, implemented
in `9fee33a9`, followed by producer-generated canary evidence in `39d345e2`.
The source audit is
`docs/execution/reference-maintenance/2026-09-14-dg17-4.md`.

## Fixed candidate and result

- Tested commit: `39d345e2bade2e9df81f2d6ecb972ae618a244bd`.
- Tested tree: `bfbcf373adba82dcc1084da31a73d0f72b636d2b`.
- Checkout: `/mnt/4tb/liminal-ilrp-ci-2026-09-13`, clean before and after CI.
- Full `just ci`: **exit 0**, systemd unit `liminal-dg17-4-ci-final`.
- Nextest: **587 passed, 47 skipped**, 46.930 seconds.
- Threaded workspace tests, doctests, rustdoc, fmt, Clippy, dependency checks,
  22 bootstrap runner controls, inventory and **33/33 canaries** passed.
- Separate `just gates`: **exit 0**, 587 active / 47 deferred declarations;
  this meter is not an independent pass count or phase authorization.

No assertion, retry count, timeout, crash serialization, canary outcome,
constitutive golden, or locked corpus was changed. The user-owned nested
`liminal-5.3-spark/` in the main checkout was untouched. The CI checkout used
its native Cargo target directory, avoiding the previously documented harness
binary-path failure with an external `CARGO_TARGET_DIR`.

## Failure retained and corrected by the existing producer

The first full run at `9fee33a9` exited **100**: 586 passed, one failed,
47 skipped. The sole failure was
`canary_evidence_must_match_a_replay_from_the_fixed_commit`. Inspection of the
test and replay function showed exact comparison of the recorded rows against
the fixed commit's packet and Markdown. The existing `just haq-canaries`
producer exited **0**, caught all 33 canaries, and changed exactly one JSON
string: the missing-packet-digest diagnostic still contained an older digest.
All IDs, caught values, expected failures and mutation semantics stayed equal.
The generated file was copied verbatim and committed, then full CI reran.
No check was bypassed and the original failure was not retried away.

## Resource envelope and receipts

Both CI jobs used MemoryHigh=20G, MemoryMax=24G, MemorySwapMax=0,
CPUQuota=400%, TasksMax=512, and four Cargo/test workers. The first failed run
reported 4,588,769,280 bytes peak memory and zero cgroup swap. The successful
run's systemd completion journal reported 1 minute 36.831 seconds wall time,
2 minutes 44.984 seconds CPU, and 1.9G memory peak. Live polling observed zero
cgroup swap; completed-unit MemoryPeak/MemorySwapPeak fields were unset, so
the journal is the final peak source. Neither unit reported an OOM result.

Durable logs are under `/mnt/4tb/liminal-formal-evidence/reviews/`:

| File | SHA-256 |
|---|---|
| dg17-4-ci.log | `4906ca3226df37b16071f4e2ae59c30e8b21a08d4158da1aa092e6bbd99354b6` |
| dg17-4-ci-final.log | `7ab776caf1f53829d807646cccbccc41cef28b3ab08bd7cdcad26a80ef17af18` |
| dg17-4-canaries.log | `29aa01e5601036e2648b18b8c112010484570bdc050f440c12e5220a10cb599a` |

The gates output is `dg17-4-gates.log` in that directory. The original generated
crash and canary files from the verification checkout were also preserved in
named Git stashes before switching commits; their identical bytes already
exist in the corresponding evidence commits. No stash was dropped.

## External registry review disposition

The actual LAMU `review_commit` of `9fee33a9` returned PASS WITH NITS, as did its
critic. Each substantive question was checked against source: the scan reads
direct dependencies from each workspace member's manifest, not the transitive
Cargo graph (`haq.rs:11923`); `same-file` is a direct graph dependency and is
exactly pinned in the workspace manifest (`Cargo.toml:39`). No additional
`winapi-util` exception is warranted or approved. The relocated durable site
is checked by the tracked-source census, which passed in full CI. The review
JSON's SHA-256 was already recorded in the audit, contrary to the critic's
suggestion that it was absent. Absolute durable-log paths remain a portability
limitation, not an assertion of remotely accessible evidence. No additional
frozen-code comment change was needed for these nits.

## Remaining qualification boundary

This is a full CI pass for the stated fixed candidate, not a fresh complete
HAQP campaign, full mutation/fuzz qualification, proof of production invariants,
suite ratification, or Phase 0/M17 GO. The remaining read-only GraphStore
migration, checked admission, durable provenance recovery, finalization permits
and scoped committed receipts remain open under AM-17.12. Documentation commits
after the tested candidate do not acquire its source identity retroactively.
