# M17.8 meter observation — 2026-09-17

Status: observation only. This record does not amend the M17 work order,
close Phase 0, qualify the HAQP packet, or authorize Phase 1.

## Source and command

- Repository: `/home/brianklam/Desktop/liminal`
- Commit: `af396bf3ad1d1546b061cebc870b3a1f1716eb1b`
- Command: `cargo run -p liminal-conformance --bin gates`
- Working tree: one untracked, user-owned nested project
  (`liminal-5.3-spark/`); no tracked modifications

The meter now uses repository-tracked Rust files when invoked inside a Git
worktree. The untracked nested project therefore cannot change this result.
Non-repository scratch roots retain the filesystem fallback used by conformance
fixtures.

## Observed result

```text
Phase 0 M17     2 deferred
Phase 1        31 deferred
Phase 1+        1 deferred
Phase 10        2 deferred
Phase 11        1 deferred
Phase 4         4 deferred
Phase 6         4 deferred
Phase 7         2 deferred
(passing now)  634 active

total backlog: 47 tests
```

The same candidate's clean full `just ci` run produced a nextest JUnit summary
of `tests="634" failures="0" errors="0"` and passed inventory and all 33
canaries.

## Reconciliation boundary

M17.8 still contains the earlier expected value of 356 active / 43 backlog.
The live result is 634 active / 47 backlog. The difference must be reconciled
against the post-AM-17.4 test additions and their phase ownership by the T1
owner. No value is copied into the work order from this observation, and no
phase checkbox is flipped here.

