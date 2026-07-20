# Kernel — normative constitutional chapter

**Status:** normative Phase 0 chapter. Canonical authority remains
`spec/v4/liminal_master_architecture_plan_v4.md` and
`spec/v4/liminal_architecture_revision_4.md`; a conflict is a defect and the
canonical v4/R4 text wins until an accepted ADR resolves it.

This chapter states the smallest semantic model and the ownership rules every
implementation must preserve. Normative words use their RFC 2119 meanings.

## Constitution

Liminal MUST remain a local-first, multimodal, revisioned graph runtime under
one semantic model (v4 §1). It is not a universal database, a replacement for
domain merge algorithms, a mandatory editor, or a license to hide conversion
loss (v4 §2).

Sixteen laws bind every phase (v4 §3):

1. Only Nodes and Relations are semantically fundamental.
2. Semantic minimalism does not require physical uniformity, and a small kernel
   does not make the whole system simple.
3. Jurisdiction resolves every independently governable semantic address at one
   immutable Workspace Basis. Capture is never rejected; Holder changes are
   never silent; ordinary sound work requires zero policy authoring and emits
   zero Jurisdiction noise; repair authorization is mutation-local; Promotion
   is repair; cross-Holder repair is ordered and recoverable; Overlay debt stays
   visible; independent dirty buffers never form a chimeric Basis.
4. Personalization may change representation, never interpretation.
5. Effects are explicit.
6. Abstraction towers disappear before hot execution.
7. Existing tools may hold Jurisdiction rather than merely integrate.
8. Every conversion declares loss.
9. Offline operation is normal.
10. Human and AI projections are separate compiler targets.
11. Unix modularity is an interface property, not process overhead.
12. Identity strength matches Relation durability.
13. Logical entity identity differs from immutable version identity.
14. Risk-retirement order precedes dependency order.
15. Generated information carries provenance.
16. State remains recoverable without the richest runtime.

No optimization, parser, compiler, or product surface may weaken these laws.
Phase -1 evidence is the current reference-semantic oracle; optimized paths
remain forbidden until differential evidence satisfies v4 §125.

## Semantic kernel

A **Node** is one identifiable unit of state (v4 §4). Its kind, revision,
physical flags, and payload association are physical fields, not additional
semantic primitives. A **Relation** is one identifiable typed association,
dependency, order, invariant, or contract between Nodes (v4 §5). Binary edges
MAY reify n-ary relationships as Nodes; containment and order MAY receive
specialized storage; anchored targets MUST remain revision-aware.

Documents, workspaces, text, resources, schemas, transformations, views,
history, synchronization, domain rows, blocks, media regions, and external
observations MUST be represented as Nodes and Relations or physical payloads of
them (v4 §§4–6). A schema address becomes a Node or Relation when it needs its
own identity, history, Relation, or Jurisdiction. No third subject type exists.

Production state MUST NOT introduce an independently governable facet object.
`liminal_graph::Node`, `liminal_graph::Relation`, and
`liminal_id::JurisdictionSubject` are the current executable shapes. The
Phase 0 representation fixture independently tests six domain topologies and
scans production Rust for a prohibited third primitive.

## Jurisdiction Contract

Jurisdiction is the sole ownership and repair policy model (v4 §7; R4 §1).
Every independently governable Node or Relation MUST resolve through exactly one
profile to a Contract at the selected Workspace Basis. An ungoverned subject is
invalid stable state.

A Contract is data with five policy groups (v4 §7.3):

- `scope` selects the governed subject;
- `resolution` names candidate Holders, read precedence, fallback, and any
  declared merge runtime;
- `mutation` names the write route, foreign-edit policy, repair authorization,
  and safety requirement;
- `continuity` names required identity and dangling behavior;
- `lifecycle` names visibility/escalation policy and whether debt is declared
  transient.

Contracts MUST be supplied by profiles in the ordinary case. User-authored
Contract data is an advanced escape hatch, never a prerequisite for routine
work. `liminal_jurisdiction::Checker` is the interpretive oracle and MUST answer
Holder, write-route, merge-runtime, identity, Overlay, Perspective,
authorization, and repair-safety questions. Sound state MUST produce empty
diagnostic byte strings.

Repair authority is conjunctive and mutation-local (Law 3F): every mutated
subject's own Contract authorizes its mutation; one subject cannot confer
authority over another. Capture remains nonblocking. Unsafe or unavailable
mutations become reviewable durable state rather than rejection or silent loss.

## Workspace Basis

Every computation MUST read one immutable `WorkspaceBasis` (v4 §7.5; R4 §8).
Its transaction and selected components record the exact graph, file, object,
buffer, external revision, and observation inputs used. Dependency tracking may
be component-granular, but the logical Basis is one coherent input vector.

Four Perspectives constrain selection:

- `ClientScoped(client)` MAY select working buffer components only from that
  client;
- `DurableOnly` selects no unsaved client buffer;
- `Published` selects declared published state;
- `Federated(domain)` delegates causal selection to that domain's declared
  merge runtime.

Ambiguous or requester-less computation MUST fail closed to `DurableOnly`.
Graph-native state remains Graph-held under client-scoped reads. Two dirty
clients over one file form two intentional working snapshots plus durable state,
never one chimeric snapshot. Component changes invalidate only computations that
recorded those components.

## Profiles

A Jurisdiction profile supplies Contracts and domain safety predicates. Stable
profile claims MUST be measured against frozen operation/session denominators
and locked acceptance evidence (v4 §§7.4, 7.6; R4 §§2, 10). Passing self-authored
fixtures alone is insufficient.

Phase 0 has exactly two executable profiles:

| Subject | Profile | Durable Holder | Minimum grade |
|---|---|---|---|
| `FILE` Node | external-file | owning file | Anchored |
| `PARAGRAPH` Node | external-file | owning file | Anchored |
| `COMMENT` Relation | graph-native | graph | Managed |
| `EXTERNAL_VALUE` Node | graph-native | graph | Managed |
| any Relation in the Phase -1 toy | graph-native | graph | Managed |

Any other live kind is `Ungoverned` until an accepted profile decision supplies
its Contract. `EXTERNAL_VALUE` is graph-native because its source of truth is a
graph transaction produced by the observation reactor; its Managed grade lasts
only while that Holder observes and maintains it (ADR-0009, ADR-0014 and the
ADR-0008 amendment trail).

## Identity

Entity identity, immutable version identity, anchors, and aliases are distinct
(v4 §19; Laws 12–13). Content hashes identify immutable bytes or semantic
versions; they do not prove continuing entity identity. Heuristic recovery may
produce evidence and confidence but MUST NOT claim logical continuity.

Identity grades form the Phase 0 evidence order:

`Ephemeral < Anchored < Inferred < Explicit < Managed < External < ContentAddressed`.

Relations declare their minimum acceptable target grade. A subject's declared
grade MUST NOT exceed its worst demonstrated outcome. ADR-0009 freezes current
ceilings: anonymous external-file subjects are Anchored; graph-native subjects
and external observations are Managed. Explicit source identifiers MAY reach
Explicit only where durable source evidence exists. A content-addressed payload
does not upgrade its continuing entity identity.

Delete/recreate, duplication, foreign rewrites, and history operations may make
continuity unknowable. Such cases MUST surface ambiguity or loss; they MUST NOT
be converted into false certainty. Phase -1 identity goldens remain the evidence
oracle until a later accepted measurement supersedes them without rewriting
history.

## Representation

Logical graph uniformity permits specialized physical stores (v4 §§43–48).
Typed Node columns, specialized Relation families, privileged text structures,
and content-addressed bulk resources are valid physical optimizations when they
preserve Node/Relation semantics. Individual characters need not be graph
allocations. Physical kind, revision, flag, and payload slots remain
associations on a Node or Relation, not new semantic entities.

`conformance/fixtures/phase0/representation-examples.toml` version 1 freezes six
governed examples: prose, Rust code, table, image reference, audio interval, and
synchronized external value. Each example MUST provide its exact topology,
payload constraints, profile, concrete Holder, and minimum identity. Domain
labels alone prove nothing. Relations lower semantic roles through `COMMENT`
with role text in payload until future domain kinds are accepted. Node roles are
fixture-only labels and MUST NOT be persisted as primitives.

Canonical JSON payloads MUST use stable key ordering and exact declared fields.
Resource hashes are 64 lowercase hexadecimal characters. Audio intervals use
unsigned `start_ms < end_ms`. External observations name source, revision,
observation time, and value. Unknown fixture fields, kinds, profiles, grades,
domains, duplicate aliases, and dangling endpoints fail closed.

## Vocabulary

Routine user surfaces SHOULD say draft, save, sync, apply, publish, file,
document, repository, service, device, recovered edit, and pending change
(v4 §7.1; R4 §3). Advanced surfaces MAY say Jurisdiction, Holder, Overlay, and
Promotion. Contract and Workspace Basis vocabulary is reserved for explain
commands, specifications, diagnostics requiring precision, and maintainer tools.

Promotion is not a second lifecycle or state machine. It is user-facing
vocabulary for an accepted `RepairPlan` whose result moves an Overlay into its
intended durable Holder or merge domain (R4 §4). Overlay aging changes
visibility, not durability. Reconciliation Queue is the only Phase 0 debt
surface; no agenda subsystem exists.

### Requirement-to-evidence map

| Requirement | Executable type or authority | Conformance evidence |
|---|---|---|
| only Nodes and Relations | `liminal_graph::{Node, Relation}`; v4 §§4–6 | `kernel_represents_six_examples_without_new_primitive` |
| every subject governed | `JurisdictionSubject`, `ProfileSet`, `Checker` | `every_phase0_example_resolves_with_declared_identity`, `every_toy_subject_names_its_holder` |
| strict representation schema | Phase 0 fixture v1; M13 schema | `representation_fixture_rejects_malformed_cases` |
| immutable coherent Basis | `WorkspaceBasis`, `BasisPerspective` | `three_perspectives_three_snapshots`, `prop_no_chimeric_basis` |
| component invalidation | `BasisComponent`, `ComponentDeps` | `invalidation_component_scoped` |
| capture never rejected | Law 3B; interpretive Checker | `capture_never_rejected` |
| sound state silent | Law 3E; R4 §2 | `sound_workspace_is_silent` |
| identity claims bounded | ADR-0009; `IdentityGrade` | `claims_never_exceed_evidence`, `identity_grade_ceilings_match_adr` |
| conversion loss declared | Law 8; projection reports | `pandoc_roundtrip_loss_report_golden`, `canonical_roundtrip_and_richer_limits_measured` |
| effects outside queries | v4 §8.6; reactor transaction boundary | `query_paths_are_effect_free`, `replay_from_frozen_basis_is_deterministic` |
| repair is mutation-local and recoverable | v4 §§7.7–7.8; R4 §§4–7 | `cross_holder_repair_is_ordered_resumable_idempotent_revertible` |
| Overlay debt visible without agenda | v4 §7.10; R4 §9 | `overlay_debt_visible_without_agenda`, `no_agenda_symbols` |

Later Phase 0 milestones add normative repair, recovery, projection, and security
tables to this chapter. They may clarify these rules but cannot weaken them
without an immutable ADR.
