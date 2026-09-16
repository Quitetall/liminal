# Assurance maintenance implementation

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md),
AM-17.14. Baseline: `54b6450c15f161978ad63cc0f999d5090bc577f4`.

## Ordered delivery

- [ ] S1: Record contract and maintenance links; independently review commit.
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
