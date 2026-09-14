# Liminal Software Architecture Specification

Revision: `0.1.0-proposed.1`. State: **proposed; acceptance pending**.

This is the proposed single governing SAS for Liminal, namespace `LIM`. It
preserves the complete Phase -1–12 program. The first distributable milestone
is a qualified OpenWarrant compiler; that release does not fulfill this SAS.

## 1. Authority and incorporation

Before explicit human acceptance, the existing v4/R4 constitution and its
accepted amendments remain governing. Merely creating this document, proposing
a revision, compiling Warrants, passing checks, or merging this migration does
not accept it. After acceptance of its exact revision and SHA-256 digest, this
document is the sole governing specification. The incorporated clauses below
are part of this document; original files become immutable historical sources
and citation targets, not a competing editable specification.

Normative clauses retain their original scope, phase, condition, modality,
thresholds, and exceptions. The requirement index identifies complete source
sections, including every obligation within each section. A compound
requirement is fulfilled only when all applicable clauses are evidenced.
Context, examples, alternatives, measurement reports, recorded decisions,
status lines, checked or unchecked boxes, and predictions in retained excerpts
are not new requirements or evidence of current completion. A historical
decision remains an attributed historical decision; migration never repeats
its authorization act. Open-gap classifications remain unresolved obligations,
not waivers. The machine crosswalk records these distinctions and source bytes.

Precedence is scoped, not a blanket newest-file rule:

1. This migration's authority, sequencing, and delivery clauses govern the
   migration and first compiler delivery after acceptance.
2. Accepted RFCs change spec-observable meaning only within their stated scope.
   RFC-0001 adds the Jurisdiction crate to v4 §117.
3. R4 refines v4 at its explicit revision points. Accepted ADR decisions and
   explicitly accepted amendments bind their recorded repository, measurement,
   qualification, or phase scope. ADRs do not implicitly authorize unrelated
   changes to constitutional meaning.
4. Normative Phase 0 chapters (`kernel.md`, `syntax.md`, `ir.md`) elaborate
   v4/R4; an unresolved conflict does not silently override the constitution.
   `transforms.md` and `protocol.md` remain navigation indexes.
5. Execution orders preserve their frozen contracts and tier boundaries,
   subject to the explicit reconciliations below. Neither implementation
   existence nor an obsolete prediction overrides acceptance criteria.

Any conflict not disposed by an exact accepted coordinate remains an explicit
gap: record the conflicting clauses, affected requirements, owner, evidence
needed, and blocked milestone. Neither keyword classification nor byte
coverage proves semantic consistency. Independent migration review and human
acceptance remain required.

## 2. Reconciliations and unresolved authority

| Gap | Source coordinates | Operative interpretation / remaining action |
|---|---|---|
| GAP-001 | README status; spec/README topical-file description | README is navigation. Phase 0 remains open at M17.5 at baseline `0d8c32a`. Three topical chapters are normative Phase 0 text; two remain indexes. No retrospective checkbox changes. |
| GAP-002 | M17.9 and M18–M24 introductory suite-ratification prerequisite; ADR-0021 Decision; phase0-amendments AM-17.4 | M17.5 HAQP-1a establishes eligibility for Brian's separate M17.6 GO. That GO authorizes Phase 1 execution. HAQP-1b executes at M24; both stages and independent review precede final suite ratification. Ratification cannot be a prerequisite for the work that qualifies it. |
| GAP-003 | M17 and M24 meter predictions; ADR-0014 Decision; AM-17.4 | Historical absolute totals describe their authored inventories. Preserve named tests, ignore policy, exact per-step deltas, and no-retry rules. At dispatch, record the actual baseline inventory and reconcile every delta against named requirements; unexplained differences block. Do not manufacture replacement future totals or weaken a test to match a number. This proposed migration explicitly reconciles the stale numeric projection, not semantic gate coverage. |
| GAP-004 | phase0-amendments open ledger; M17 Amendments; ADR-0021 Consequences | Explicit user-ratified rows retain their attributed scope. Other open ledger rows remain pending M17's disposition; incorporation does not ratify them. Any disputed implementation/authority mismatch blocks the affected qualification claim. |
| GAP-005 | M24 Scope/Data schemas/No new test language; ADR-0021 HAQP-1b | The seven-test aggregate remains exact. HAQP-1b is additional required qualification work at M24; old aggregate-only wording cannot erase it. Any missing killing test, changed frozen API, or persisted-format activation needs its own explicit amendment before execution. |
| GAP-006 | historical work-order checkboxes, reports, accepted prior phase decisions | Preserve all records. Requirement status derives from linked Warrants and exact evidence, never boxes or source-code existence. Prior Phase -1 acceptance does not accept later phases. |
| GAP-007 | integration profile, process protocol, canonical IR, pinned corpus | Interfaces and corpus observables must be designed and reviewed before implementation/parity. No invented version pins, byte channels, semantic field lists, or permissive compatibility promises. Named design deliverables block dependent work until accepted. |
| GAP-008 | active HAQP campaign and migration branch | Campaign source checkout and evidence remain untouched. Preserve final campaign commit and raw evidence. Evaluate the merged candidate against exact HAQP parent/tree and metadata rules; source changes that invalidate eligibility require a fresh full qualification. Migration CI does not qualify HAQP. |

## 3. Acceptance and qualification

SAS acceptance, M17.6 Phase 1 GO, and final Phase 1 suite ratification are three
separate human decisions. No agent may supply their signatures, acting roles,
dispositions, or resolutions. Proposed Warrants describe work; they do not
authorize phase execution. A required unknown result blocks acceptance of the
affected delivery claim.

HAQP thresholds remain those of accepted ADR-0020 as staged by ADR-0021:

- Fixed clean source commit/tree, two execution lanes, full rerun after the last
  verified defect, exact command/toolchain/seed/artifact provenance.
- Bidirectional requirement/test coverage with positive, negative, adversarial,
  Basis/provenance, replay, and applicable fault/recovery evidence.
- At least 64 semantic mutants; at least eight per critical family; no operator
  above 25%; 100% applicable nonduplicate nonequivalent kills; one survivor
  blocks; exclusions need proof and both review concurrences. Mutation clauses
  execute at HAQP-1b/M24; gate canaries remain in HAQP-1a.
- Five families each have at least 100,000 accepted deterministic generated
  cases, discard rate at most 1%, independent metamorphic evidence, a
  30-minute sanitizer fuzz campaign and at least 16 predeclared seeds; at least
  150 target-minutes total. The existing stricter packet inventory survives.
- Independent oracles; exhaustive registered reconciliation crash boundaries
  injected before and after, recovery repeated twice; runtime/registry equality.
  ADR-0021's explicit reconciliation-boundary criterion is retained, never
  broadened into a claim about every durable write.
- Two blind adversarial passes, distinct identities and model families when
  automated, at least 12 attempts each, exact prompt/identity provenance, zero
  unresolved verified findings; retain false-positive evidence.
- Eight-hour intended campaign ceiling; exceeding it twice blocks ratification
  and requires the recorded corrective process. Fully coordinated residual risks
  and protected held-out corpora; no new corpus access permission.

## 98. Implementation phases

These are Liminal phases; OpenWarrant's phases retain their own namespace and
declarations. Dependencies and current evidence belong in the central roadmap.
Later phases receive mechanical Warrants at their preceding gates, after the
earlier experiments have established their interfaces.

### Phase -1 — Jurisdiction, identity, repair, and projection falsification

Exit:
- Preserve the complete v4 Phase -1 experiment obligations, R4 killer experiments, locked evidence, amendment dispositions, and attributed M12 decision without rewriting history.

### Phase 0 — Constitution, profiles, corpus, and conformance laws

Exit:
- Complete M13–M17 obligations, qualified HAQP-1a packet, fresh blind-review dispositions, repository gates, amendment reconciliation, and Brian's separate M17.6 GO/NO-GO.

### Phase 1 — External-file source-to-HTML compiler

Exit:
- Complete M18–M23 source/CST, full source maps/HIR/explicit syntax/graph, formatter/CLI, incremental compiler, HTML, fuzz and benchmarks; M24 aggregate, HAQP-1b, independent review, both-stage evidence and separate suite ratification.

### Phase 2 — Persistent daemon and Neovim workflow

Exit:
- Prove exact Basis synchronization through editing, saving, foreign modification and Git checkout, with persistent daemon/LDP, Neovim workflow and real-daemon crash handshake.

### Phase 3 — Transform and macro infrastructure

Exit:
- Prove shared semantic macros across Neovim, CLI and harness, source reparse equivalence, declared transform contracts, structural editing and expansion tracing.

### Phase 4 — Workspace and build graph

Exit:
- Deliver manifest/lockfile, multi-document dependencies, Cargo/LSP/Pandoc adapters, resources and indexes; prove declared conversion loss, demand-driven elaboration and corruption refusal.

### Phase 5 — Core document domains and graph-native pilot

Exit:
- Deliver prose, planning, academic, table, notebook and long-form domains; prove one Holder per subject and measured graph-native projection; agenda projects the queue only after its explicit gate amendment.

### Phase 6 — History and local-first synchronization

Exit:
- Prove transaction/history semantics, declared replica convergence, partition recovery, explicit unsupported-merge refusal, migration and backup; decide collaboration and retention before implementation.

### Phase 7 — Intelligent external Relations

Exit:
- Deliver effect-isolated resolvers, relation policies, freshness, Basis recording and freeze; prove deterministic replay and effect-free relation traversal.

### Phase 8 — Multimodal resources and lecture stack

Exit:
- Reconstruct a lecture offline across media, ink and transcript relations, preserving separate raw-media and derived-recognition Jurisdictions.

### Phase 9 — Rich web, phone and tablet clients

Exit:
- Prove semantic agreement across clients, expose only measured projection capabilities, and deliver capture-first phone and pen-first tablet workflows with revisioned resource synchronization.

### Phase 10 — AI compiler and semantic operation loop

Exit:
- Deliver AI MIR, model profiles, context planning, projections, multimodal bundles and evaluation; refuse stale operations and retain AI output as derived until approved.

### Phase 11 — Stable plugin ecosystem

Exit:
- Version Wasm/Lua/native/process interfaces, package locks, certification and migrations; prove third-party dialect integration and denied-capability refusal.

### Phase 12 — Advanced spatial, table and presentation runtimes

Exit:
- Deliver specialized table/layout/timeline/diagram/ink/visualization runtimes while preserving semantic uniformity and Jurisdiction; compiled policy needs differential crash and concurrent-buffer evidence before admission.

## 100. First compiler distribution acceptance

After Phase 1 qualification, the first release must include restricted
OpenWarrant document/frontmatter support, a pinned compiler profile, a versioned
process boundary and canonical IR, with their interfaces accepted before pins
and observables are frozen. Compare the existing OpenWarrant compiler oracle
and the Liminal adapter over every document in the pinned OpenWarrant corpus.
Declare byte observables and semantic IR fields before running the comparison.
Require zero byte differences **and** zero semantic differences, full corpus
coverage, reproducible replay, and refusal for any difference or omission.
Semantic diagnostics never replace byte parity.

Release requires reproducible builds, exact version/source/dependency pins,
usage instructions, compatibility and unsupported-input limits, retained
qualification evidence, and independent verification. Design-only interfaces,
simulated parity, unit tests of a parity data structure, or an unresolved
Warrant do not qualify a compiler distribution. OpenWarrant owns governance;
Liminal owns document semantics and Basis. This milestone does not authorize
the daemon, later domains, clients, AI or plugin phases.

## 101. Governance of this SAS

One document is selected through `openwarrant.toml`. Proposed revision records
pin its exact bytes and requirement index. Authoritative phase extraction must
use the selected accepted revision's matching source path and digest; missing,
ambiguous, malformed or mismatched authority returns unavailable. Draft
inspection is explicitly draft and cannot report authoritative completion.

Requirement IDs are stable and append-only. Future semantic changes require a
decision record, proposed SAS revision, impact analysis and amended/superseding
Warrants while retaining original requirements and evidence. Completion status
derives only from Warrants, independent evidence and authorized resolutions.
Normative text is never edited to tick a box. T1 decisions remain Brian's;
T2 design stays centrally reviewed; T3/T4 transcription and checks are
delegated. External LAMU review is required after every implementation commit.

## 106. Migration and delivery requirements

| Requirement | Obligation |
|---|---|
| LIM-SAS-RQ-001 | Accept exactly one digest-bound SAS revision without conflating SAS acceptance, Phase GO, or suite ratification. |
| LIM-SAS-RQ-002 | Preserve every original source obligation, source bytes, stable IDs, scope, tiers and explicit conflict/gap crosswalk. |
| LIM-SAS-RQ-003 | Derive completion only from Warrants and evidence; preserve historical records and refuse unevidenced completion. |
| LIM-SAS-RQ-004 | Support all fourteen declared Liminal phases with canonical namespace-correct references and digest-bound authority. |
| LIM-SAS-RQ-005 | Close M17 with HAQP-1a, independent reviews, qualified provenance, amendment reconciliation and separate M17.6 human GO. |
| LIM-SAS-RQ-006 | Deliver M18 source and lossless error-tolerant CST under its complete frozen work order. |
| LIM-SAS-RQ-007 | Deliver full M19 source maps, HIR, explicit syntax and derived graph under its complete frozen work order. |
| LIM-SAS-RQ-008 | Deliver M20 formatter and CLI under its complete frozen work order. |
| LIM-SAS-RQ-009 | Deliver M21 incremental compiler and independent full-reparse equivalence under its complete frozen work order. |
| LIM-SAS-RQ-010 | Deliver M22 HTML and incremental rendering under its complete frozen work order. |
| LIM-SAS-RQ-011 | Deliver M23 fuzz and benchmark evidence under its complete frozen work order. |
| LIM-SAS-RQ-012 | Complete M24 aggregate and unchanged HAQP-1b thresholds, independent review, both stages and separate final ratification. |
| LIM-SAS-RQ-013 | Specify and accept restricted OpenWarrant document support, versioned process protocol and canonical IR before adapter implementation. |
| LIM-SAS-RQ-014 | Freeze a compiler profile and exact source/toolchain/dependency/corpus pins only after its accepted interfaces exist. |
| LIM-SAS-RQ-015 | Predeclare byte and semantic observables and prove zero differences over the entire pinned OpenWarrant corpus with refusal controls. |
| LIM-SAS-RQ-016 | Distribute a reproducible, version-pinned qualified compiler with usage docs, compatibility limits and retained evidence. |
| LIM-SAS-RQ-017 | Preserve active campaign checkout/evidence and requalify the final baseline whenever eligibility is invalidated. |
| LIM-SAS-RQ-018 | Retain every Phase -1–12 commitment; first compiler distribution does not satisfy the complete program. |
