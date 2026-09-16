# Assurance maintenance implementation

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md),
AM-17.14. Baseline: `54b6450c15f161978ad63cc0f999d5090bc577f4`.

## Ordered delivery

- [x] S1: Record contract and maintenance links; independently review commit.
- [ ] S2: Family catalog and `assurance check`: reject missing mandatory gates,
  unknown/duplicate IDs, uncovered targets, stale references and workflow drift.
  Disposable fixtures never open locked corpora.
- [ ] S3: `assurance run <profile>` and `assurance report`: fixed command registry,
  durable incomplete/failure receipts, environment/cost observations and advisory
  impact. Selection never bypasses merge requirements.
- [ ] S4: Generated workflow wiring; Linux parity, portability/job names,
  nextest/threaded diversity, weekly advisories, informational beta. Integrate
  bounded resource harness run/lint, refusing missing limits. Hosted execution
  evidence is separate from local generation checks.
- [ ] S5: `assurance amend propose|check|apply`: parser-backed unchanged exact
  targets, independent review, externally pinned SSH policy/tool trust, isolated
  apply and producer-derived refresh. Disposable-key controls cover signatures,
  namespace, scope, revision, source/patch drift, ambiguity, protected fields and
  interruption. No human key or trust activation needed for tests.
- [ ] S6: Complete merge checks, inventory, canaries and resource controls; named
  test deltas and independent diff/commit findings verified at cited lines. Human
  enrollment and hosted execution remain explicit external boundaries.

## Public test seams

Approved CLI seams: `assurance check`, `run`, `report`, and
`amend propose|check|apply`. Test observable exits, receipts and filesystem effects;
no mock-only subprocess-isolation or signature claims. Red before green per slice.

## Evidence and limits

Record exact revision, commands, exits and durable logs per slice. Missing evidence
remains unexecuted. No local pass proves hosted operation, HAQP qualification,
formal obligations or Phase 1 authority. Historical receipts are never rewritten.

Implementation started in isolated `assurance-maintenance` worktree. This plan
does not establish code, execution or signing capability.

S1 commit `5809e3763837af03b337cd57bcfadfb588a2dd9a`: LAMU MiMo V2.5 Pro
returned PASS WITH NITS. Verified documentation-only scope and references. Kept
existing conversation-attribution convention and protocol note placement; this
follow-up closes the S1 checkbox. Receipt:
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-5809e376-review.stdout`.

S2 classification slice: 13 registered families; nine new active CLI tests, zero
ignore flips. Targeted tests pass (9/9); full suite not yet rerun. Evidence root
`/mnt/4tb/liminal-formal-evidence/reviews/`: `assurance-red-2` proves missing CLI,
`assurance-red-3` proves missing Python discovery, `assurance-red-4` proves
duplicate-key acceptance, and `assurance-green-4` passes all nine controls.
Each has `.stdout` and `.stderr`; bounded systemd runs retained actual exits.
`assurance-red-1` and `assurance-green-1` were infrastructure launch failures,
not red/green test evidence. Workflow drift checking remains S4 work; S2 stays
open until that dependent validation exists.
