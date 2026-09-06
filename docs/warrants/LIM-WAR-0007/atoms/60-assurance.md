---
schema: oh.war/atom/v1
warrant_uuid: 01a07471-b7df-7062-8c69-bd19f6e47bf5
role: assurance
jurisdiction: authored
order: 60
classification: internal
---

# Assurance

## Acceptance Obligations

### OBL-001 — Every production fuzz target and benchmark has complete evidence

- **scope:** Exactly seven D23.1 fuzz targets and seven D23.3 benchmarks on their declared inventories/environment.
- **evidence:** 217 sanitizer target-minutes, seed hashes, raw logs, retained/replayed minimized cases, exactly 30 samples per benchmark, recomputed median/p95, Brian's actual accepted baseline and both inclusive regression comparisons.

### OBL-002 — Missing evidence and regressions block the gate

- **scope:** Missing target/seed/metadata/sample, hash or environment mismatch, extra benchmark, boundary and above-boundary regressions, seeded recurrent fuzz defect.
- **evidence:** Observed refusals, exact-threshold passing controls, missing-pair/hash rejection and regression replay failure; candidate cannot rewrite its accepted baseline or activate migration.

### OBL-003 — Independent evidence and scoped decisions control completion

- **scope:** Only this M23 intervention and its cited source obligations; no broader SAS claim.
- **evidence:** Actual independent verifier findings and their disposition, complete bounded positive/refusal evidence, source-to-evidence reconciliation, and scoped human authorization and deliverable-specific decisions when received. An absent reviewer, unmet dependency or performer assertion cannot satisfy this obligation.

## Gate Adequacy

Adversarial question: could this intervention appear complete while omitting a source obligation, required refusal, independent oracle, real decision or qualifying artifact? The controls above must demonstrate that it cannot within the declared scope.

Contract-adequacy review and execution of these controls are pending. No reviewer, attack result, outcome or disposition is asserted. Controlled-assurance checks may report these missing records as blockers; do not downgrade assurance or invent records to make the checker green.

## Residual Risk

Draft dependency and evidence references describe required future observations. Deterministic compilation establishes document structure only. Unavailable evidence remains unknown; independent review and actual authorized decisions remain required.
