# 0018. Preserve interpretive Jurisdiction oracles through Phase 1

- **Status:** accepted
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** v4 §§7.9, 125; R4 §10; M02–M08; M14; M15

## Context

Phase -1 and Phase 0 evidence comes from interpretive Contract dispatch,
RepairPlan evaluation, Workspace Basis selection, and ILRP execution. Indexed
dispatch or compiled plans could improve speed but would introduce a second
semantic path before differential equivalence exists.

## Decision

Keep interpretive implementations as conformance oracles throughout Phase 1.
Do not land compiled Contracts, generated repair code, indexed policy dispatch,
or compiled ILRP/Basis selection. Any optimization proposal must first compare
against the oracle across positive, negative, malformed, crash-boundary,
recovery-idempotence, and concurrent-buffer Basis cases.

**What would falsify or reverse this:** measured Phase 1 latency cannot meet its
gate with interpretive execution and a compiled prototype passes the complete
differential suite with no observable divergence. Optimization then requires a
new ADR; this oracle remains available for conformance.

## Consequences

- One semantic authority remains during Phase 1.
- Policy execution may be slower; performance is measured before optimization.
- Compiled paths cannot bypass crash or anti-chimera evidence.
