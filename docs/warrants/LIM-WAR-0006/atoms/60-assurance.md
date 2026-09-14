---
schema: oh.war/atom/v1
warrant_uuid: 01a07471-b7d6-72e3-a81a-51f22a1ec521
role: assurance
jurisdiction: authored
order: 60
classification: internal
---

# Assurance

## Acceptance Obligations

### OBL-001 — Full and patch rendering produce the required exact bytes

- **scope:** M22 closed IR role mapping, all declared render fixtures and edit/order/deletion/insertion cases.
- **evidence:** Brian-accepted exact snapshots when received, full_document_html_matches_golden, incremental_patch_equals_full_render, all existing M22 gates and independent patch/full byte comparison.

### OBL-002 — Unsafe markup and malformed patch sets are refused

- **scope:** Hostile text/attributes, unsupported roles, raw markup, duplicate blocks, missing targets, mixed/stale Bases and overlapping replacements.
- **evidence:** Exact escaping and explicit render/patch refusals over planted cases; unchanged blocks yield no patch; failed equality never produces a purported valid patch set.

### OBL-003 — Independent evidence and scoped decisions control completion

- **scope:** Only this M22 intervention and its cited source obligations; no broader SAS claim.
- **evidence:** Actual independent verifier findings and their disposition, complete bounded positive/refusal evidence, source-to-evidence reconciliation, and scoped human authorization and deliverable-specific decisions when received. An absent reviewer, unmet dependency or performer assertion cannot satisfy this obligation.

## Gate Adequacy

Adversarial question: could this intervention appear complete while omitting a source obligation, required refusal, independent oracle, real decision or qualifying artifact? The controls above must demonstrate that it cannot within the declared scope.

Contract-adequacy review and execution of these controls are pending. No reviewer, attack result, outcome or disposition is asserted. Controlled-assurance checks may report these missing records as blockers; do not downgrade assurance or invent records to make the checker green.

## Residual Risk

Draft dependency and evidence references describe required future observations. Deterministic compilation establishes document structure only. Unavailable evidence remains unknown; independent review and actual authorized decisions remain required.
