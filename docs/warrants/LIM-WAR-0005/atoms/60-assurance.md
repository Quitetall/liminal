---
schema: oh.war/atom/v1
warrant_uuid: 01a07471-b7d3-7772-84b2-e3576e7e6073
role: assurance
jurisdiction: authored
order: 60
classification: internal
---

# Assurance

## Acceptance Obligations

### OBL-001 — Incremental output equals the independent full compiler

- **scope:** Declared M21 source/Basis edits and deterministic M19 JSON/ordered diagnostics; include HTML after M22.
- **evidence:** incremental_equals_full_compile_law_holds and all existing M21 milestone gates, dependency traces, replay, exact byte comparison and an oracle-independence audit.

### OBL-002 — Stale or hidden inputs cannot pass equality

- **scope:** Relevant and irrelevant Basis-component edits, stale source revisions, malformed inputs and planted missing-dependency/effect cases.
- **evidence:** Required invalidation or exact refusal, unchanged irrelevant results/cache dependencies, and observed mismatch detection for a seeded stale-cache result; no pointer/Salsa-ID oracle.

### OBL-003 — Independent evidence and scoped decisions control completion

- **scope:** Only this M21 intervention and its cited source obligations; no broader SAS claim.
- **evidence:** Actual independent verifier findings and their disposition, complete bounded positive/refusal evidence, source-to-evidence reconciliation, and scoped human authorization and deliverable-specific decisions when received. An absent reviewer, unmet dependency or performer assertion cannot satisfy this obligation.

## Gate Adequacy

Adversarial question: could this intervention appear complete while omitting a source obligation, required refusal, independent oracle, real decision or qualifying artifact? The controls above must demonstrate that it cannot within the declared scope.

Contract-adequacy review and execution of these controls are pending. No reviewer, attack result, outcome or disposition is asserted. Controlled-assurance checks may report these missing records as blockers; do not downgrade assurance or invent records to make the checker green.

## Residual Risk

Draft dependency and evidence references describe required future observations. Deterministic compilation establishes document structure only. Unavailable evidence remains unknown; independent review and actual authorized decisions remain required.
