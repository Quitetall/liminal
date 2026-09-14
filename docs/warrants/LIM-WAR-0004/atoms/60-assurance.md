---
schema: oh.war/atom/v1
warrant_uuid: 01a07471-b7d1-7081-b846-87cd85197403
role: assurance
jurisdiction: authored
order: 60
classification: internal
---

# Assurance

## Acceptance Obligations

### OBL-001 — Formatter and CLI satisfy exact source and output contracts

- **scope:** M20 selected Holders, both dialects and each declared CLI path.
- **evidence:** formatter_idempotence_law_holds plus all existing M20 milestone coverage; graph equality, second-run no-change evidence, ordered diagnostics and exact CLI byte captures.

### OBL-002 — Preflight and partial failure behavior are observed

- **scope:** Malformed/opaque regions, an invalid later Holder before first write and injected later write failure after an earlier replacement.
- **evidence:** No write on preflight failure; retained opaque bytes; exact per-file partial-failure diagnostics and durable completed-file state; check/expand observed read-only.

### OBL-003 — Independent evidence and scoped decisions control completion

- **scope:** Only this M20 intervention and its cited source obligations; no broader SAS claim.
- **evidence:** Actual independent verifier findings and their disposition, complete bounded positive/refusal evidence, source-to-evidence reconciliation, and scoped human authorization and deliverable-specific decisions when received. An absent reviewer, unmet dependency or performer assertion cannot satisfy this obligation.

## Gate Adequacy

Adversarial question: could this intervention appear complete while omitting a source obligation, required refusal, independent oracle, real decision or qualifying artifact? The controls above must demonstrate that it cannot within the declared scope.

Contract-adequacy review and execution of these controls are pending. No reviewer, attack result, outcome or disposition is asserted. Controlled-assurance checks may report these missing records as blockers; do not downgrade assurance or invent records to make the checker green.

## Residual Risk

Draft dependency and evidence references describe required future observations. Deterministic compilation establishes document structure only. Unavailable evidence remains unknown; independent review and actual authorized decisions remain required.
