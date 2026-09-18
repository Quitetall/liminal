# M17.8 meter observation — 2026-09-18

Status: observation only. This record does not amend the M17 work order,
close Phase 0, qualify the HAQP packet, or authorize Phase 1.

## Source and command

- Repository: `/home/brianklam/Desktop/liminal`
- Commit: `2a97be5e524297401754bd8680d3564cf6b5f036`
- Command: `just gates`
- Working tree: one untracked, user-owned nested project (`liminal-5.3-spark/`);
  no tracked modifications

The meter reads repository-tracked Rust files inside this Git worktree. The
untracked nested project is outside that inventory and does not affect counts.

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
(passing now)  638 active

total backlog: 47 tests
```

The same commit's managed `just ci` run completed with nextest summary
`638 tests run: 638 passed`, plus successful deny, inventory, and 33/33
canaries.

## Reconciliation boundary

M17.8 still contains its earlier expected value of 356 active / 43 backlog.
The live result is 638 active / 47 backlog, up from the 634/47 observation in
`verification/meter-reconciliation-2026-09-17.md`. The difference must be
reconciled against post-AM-17.4 test additions and phase ownership by the T1
owner. No value is copied into the work order here, and no phase checkbox is
flipped.
