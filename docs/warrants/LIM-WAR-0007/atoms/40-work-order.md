---
schema: oh.war/atom/v1
warrant_uuid: 01a07471-b7df-7062-8c69-bd19f6e47bf5
role: work_order
jurisdiction: authored
order: 40
classification: internal
---

# Work Order

## Deliverables

1. Execute all seven D23.1 targets with at least 16 predeclared non-locked seeds each and sanitizer-enabled 31-minute campaigns: 217 target-minutes total.
2. Retain every minimized failure with exact hash/metadata and replay it through fuzz_regressions_stay_fixed.
3. Capture exactly 30 independent raw samples for each of seven D23.3 benchmarks on the recorded reference environment; obtain separate T1 baseline acceptance and enforce inclusive median +10% and p95 +15% limits.

## Frozen Surfaces

Full M23 work order including AM-23.2 inventory: cst_parse, format_idempotent, canonical_round_trip, incremental_full_equivalence, html_render, graph_interchange_codec, ilrp_recovery. No locked-corpus access, post-failure exclusion, retry or implicit persisted-format activation.

The complete source obligation is retained even when this summary does not repeat its field list. Any missing implementation, test or accepted amendment is a recorded prerequisite gap, not permission to narrow scope.

## Stage Responsibilities

| Stage | Source coordinate | Tier | Work |
|---|---|---|---|
| STAGE-001 | M23.1 | T2 | Implement the seven frozen fuzz targets and seed manifest |
| STAGE-002 | M23.2 | T3 | Run the `cst_parse` sanitizer campaign |
| STAGE-003 | M23.3 | T3 | Run the `format_idempotent` sanitizer campaign |
| STAGE-004 | M23.4 | T3 | Run the `canonical_round_trip` sanitizer campaign |
| STAGE-005 | M23.5 | T3 | Run the `incremental_full_equivalence` sanitizer campaign |
| STAGE-006 | M23.6 | T3 | Run the `html_render` sanitizer campaign |
| STAGE-007 | M23.6a | T3 | Run the `graph_interchange_codec` sanitizer campaign |
| STAGE-008 | M23.6b | T3 | Run the `ilrp_recovery` sanitizer campaign |
| STAGE-009 | M23.7 | T2 | Replay committed minimized failures and flip the existing regression test |
| STAGE-010 | M23.8 | T2 | Implement the real Phase 1 cold-start benchmark |
| STAGE-011 | M23.9 | T2 | Implement the real Phase 1 memory benchmark |
| STAGE-012 | M23.10 | T2 | Implement the sampler, baseline verifier, and comparator recipes |
| STAGE-013 | M23.11 | T3 | Capture 30-sample candidate benchmark evidence |
| STAGE-014 | M23.12 | T1 | Accept the initial Phase 1 benchmark baseline after inspecting all samples |
| STAGE-015 | M23.13 | T2 | Enforce the frozen benchmark regression policy |
| STAGE-016 | M23.14 | T2 | Run the complete repository continuous lane after the last fix |
| STAGE-017 | M23.15 | T4 | Reconcile the milestone meter without activating migration |

## Autonomy and Escalation

Executor kind and responsibility tier are orthogonal (execution protocol §7). A non-decision T1 stage is planned for the most capable agent under the highest scrutiny, independent second review and required human sign-off; this neither lowers its tier nor grants power to approve. Human executor assignments identify actual Brian-owned decisions or golden/baseline acceptance. An agent may prepare a decision request and record a decision actually received, but may not supply that decision, authorize work, ratify a suite or verify its own work.

T1 semantic, constitutional, golden/baseline and phase/release decisions remain Brian's; central/root review owns high-scrutiny reasoning and finding verification. T2 implementation remains centrally reviewed. T3/T4 agents may transcribe, run approved commands and check inventory within frozen scope. The source step's tier is retained; a mechanical executor cannot assume its decider's authority. Independent reviewers must record real identity and separation facts when selected; none are invented here.

This draft authorizes no dispatch. After real authorization, stop on a frozen-API change, unresolved source conflict, missing qualifying evidence, locked-corpus access need or undeclared scope. Keep active campaign checkout untouched; qualification-invalidating changes require a fresh eligible baseline and full rerun.

## Rollback

Use an isolated authorized implementation branch. Revert only this intervention's implementation changes when required; retain historical source bytes, raw campaign/qualification evidence and adverse findings. Supersede or amend any subsequently authorized Warrant rather than editing its immutable contract.
