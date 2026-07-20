# 0013. Phase 0 go/no-go after the falsification laboratory

- **Status:** accepted
- **Date:** 2026-07-19
- **Deciders:** Brian
- **Related:** v4 Part XXII final Phase -1 gate; `docs/execution/M12.md`

## Context

Phase -1 has completed its falsification laboratory: every final-gate assertion
is born-passing, the constitutional audit has an accepted ADR trail, discovered
gaps were repaired or amended explicitly, and the user issued GO at Checkpoint
3. The decision now controls whether Phase 0 work orders may be authored.

## Final gate results

| Test | Result |
|---|---|
| `every_toy_subject_names_its_holder` | PASS |
| `no_facet_primitive_exists` | PASS |
| `anonymous_text_identity_grade_is_honest` | PASS |
| `one_projection_reaches_canonical_roundtrip_and_lenses_are_measured` | PASS |
| `crash_gate_matrix_and_revert_hold` | PASS |
| `no_chimeric_basis_at_gate` | PASS |
| `overlay_debt_visible_without_agenda` | PASS |
| `frozen_basis_replay_is_deterministic_at_gate` | PASS |
| `denominators_frozen_and_split_locked` | PASS |
| `go_no_go_adr_recorded` | PASS |

Final spec-debt meter after gate-test creation: **238 active tests**, **0 Phase
-1 deferred tests**, **21 deferred tests total**.

## Decision

**GO**: Phase 0 begins; Phase 0 work orders are authored now from docs/execution/template.md

**What would falsify or reverse this:** any Phase 0 gating test contradicting a
Phase -1 measurement requires a superseding ADR before implementation proceeds.

## Consequences

- **Positive:** Phase 0 may consume the frozen Contract, measurements, and
  accepted amendment trail as its constitutional baseline.
- **Negative:** this decision does not promote Phase -1 toy implementations to
  production components or ratify a future Phase 1 test suite.
- **Follow-ups:** author Phase 0 work orders at this gate; submit any later Phase
  1 suite to a separate adversarial review and explicit ratification decision.

## Alternatives considered

- **NO-GO pending Contract revision** — rejected because all ten final gates are
  green and no surviving killer experiment requires a Contract shape revision.
- **GO without authored work orders** — rejected because `phases.md` requires
  Phase 0 mechanical orders to be authored at this gate.

## Appendix A — killer-experiment outcomes

| Milestone | Outcome | Evidence conclusion |
|---|---|---|
| M2 ILRP/crash boundaries | Survived | Recovery remains crash-resumable and idempotent; M12 closed a vacuous crash-matrix assertion without revising the Contract. |
| M4 repair DAG | Survived | Deterministic ordering remains distinct from safety; exact undo preimages and non-empty recovery terminals are now asserted. |
| M6 multi-buffer Basis | Survived | No chimeric Basis was found; Q1 capture was corrected to require the owning external-file path. |
| M7 identity | Survived | External-file remains Anchored; graph-native and EXTERNAL_VALUE remain Managed, capped by measured evidence. |

## Appendix B — identity evidence

- `conformance/golden/identity_matrix.md` — SHA-256
  `20294974b3c1c36857ab5eb9ea1ff9c57f1428364dd1242e698ba686f799bc4e`
- `conformance/golden/anchor_recovery.md` — SHA-256
  `84a987e96c12e65e7d7d5b5d6cc411323d4ed1db1a78a3e24f1976d38d84f484`

## Appendix C — projection and held-out scorecard evidence

- `conformance/golden/pandoc_loss.md` — SHA-256
  `b83318659df74968aeb2609a9dd666129156ea60ae1875625d5885359167e8a0`
- `conformance/golden/scorecard-heldout-v1.json` contains the recorded
  external-file and graph-native profile scorecards — SHA-256
  `379561d61cdeda85ff9d5b5d1b76b9cc42639fe7815ef6aded2cbc3aa596e6bd`
- Gate 9 mechanically verified the locked, non-empty held-out manifest. This
  ADR records verifier output and scorecard evidence, not corpus contents.

## Appendix D — amendment-ledger disposition

ADR-0012 ratifies every M1-M11 amendment recorded by ADR-0008. M12 amendments
AM-12.1 through AM-12.5 were explicitly user-ratified on 2026-07-19 and remain
recorded in `docs/execution/M12.md`; none is silently folded into the baseline.

## Appendix E — constitutional shape audit

| Audited surface | Revised? | Disposition |
|---|---|---|
| `JurisdictionKey` | No | Shared `Path` grammar survived; AM-12.5 corrected the M08 assertion. |
| `StatePredicate` / `PrestateMatch` | No | Fail-closed freeze behavior was implemented behind the existing shape. |
| `SubjectSelector` | No | Existing selector set covered the laboratory. |
| `LifecyclePolicy` defaults | No | Existing defaults survived. |
| `RepairDecision` / `SafetyRequirement` | No | Existing set survived; no `Reject` variant was introduced. |
| `IntentState` plus five ILRP boundaries | No | Existing states and boundaries survived. |
| `liminal_graph::Operation` | No | No enum variant changed; M08 added EXTERNAL_VALUE as a node kind, and AM-12.3 completed its Holder dispatch. |
| `BasisPerspective` | No | Existing perspective variants survived. |
| Identity-grade ceilings and profile defaults | Yes, decision frozen | ADR-0009 caps external-file at Anchored and graph-native/EXTERNAL_VALUE at Managed. |
| Annotated-source and Pandoc capability levels | Yes, decision frozen | ADR-0010 freezes M09 at Level 2 and M10 at Level 1. |
| Denominator constants | Yes, decision frozen | ADR-0011 freezes 2000 ms, 30 min, key grammar, event-to-operation table, and denominator arithmetic. |
| M1-M11 amendment ledger | Yes, disposition frozen | ADR-0012 ratifies every recorded M1-M11 amendment; Appendix D records M12 separately. |
