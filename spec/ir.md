# IR — normative representations, projections, and incremental boundaries

**Status:** normative Phase 0 chapter. Canonical v4/R4 text remains authority.
No parser, production query engine, pass runtime, or compiled Jurisdiction plan
is authorized by this publication.

## IR levels

Liminal uses a deliberate representation tower (v4 §§9–14, 21–27):

- **L0 Bytes/CST** preserves exact bytes, trivia, malformed input, and
  revision-aware ranges. It MUST be lossless and error-tolerant.
- **L1 Human IR** represents authorial intent, source anchors, declared identity,
  unresolved references, and unexpanded domain sugar.
- **L2 Resolved Graph IR** is the normalized Node/Relation graph at one explicit
  Workspace Basis. It is derived for external-file domains and governing for
  graph-native domains.
- **L3 Domain IR** supplies prose, code, table, media, layout, and other derived
  analyses without adding semantic primitives.
- **L4 Backend IR** represents target-specific output such as HTML/DOM, terminal
  cells, PDF, Pandoc AST, accessibility, search, AI, execution, or sync plans.

Lowering MUST preserve each level's declared invariants until a TransformContract
explicitly declares loss. Demand-driven parsing MAY retain opaque regions, but an
opaque region remains byte-preserving and cannot silently acquire semantics.

## Projection laws

Projection claims are scoped by direction, supported subset, profile,
capability level, and Workspace Basis (v4 §§8.1–8.5, 124). No universal
bidirectional guarantee exists.

**IR-PROJ-01 — canonical source law.** For a declared supported graph subset
`S`, `parse(emit(graph))` MUST equal canonical graph semantics for `S` at the
same Basis. For declared accepted source `T`, `emit(parse(source))` MUST equal
the canonical source form for `T`, not necessarily original bytes. Exact-source
preservation is a separate claim and requires an exact-byte channel.

**IR-PROJ-02 — edit equivalence and boundary law.** For supported operations,
incremental text edit then reparse MUST equal corresponding graph transaction
then emit. Selection, mark, and anchor boundaries; block restructure; undo;
malformed input; and foreign edits are independent conjuncts. One failed
conjunct caps capability below Level 3. Concurrent merge requires a declared
merge runtime and cannot be inferred from deterministic local replay.

Capability levels are evidence labels:

| Level | Minimum demonstrated behavior |
|---|---|
| 0 | no supported conversion |
| 1 | import/export with explicit measured loss |
| 2 | canonical round-trip for declared subset |
| 3 | incremental edit equivalence plus boundary, malformed, undo, and recovery bars |
| 4 | Level 3 plus declared concurrent merge semantics |

ADR-0010 freezes annotated-source at Level 2 and Pandoc at Level 1. A new frozen
measurement may lower or raise a later declaration; documentation alone cannot.

## Transform contracts

Every transformation declares exactly eight fields before execution (v4 §14;
Law 8).

**IR-XFORM-01 — TransformContract shape.** `requires | preserves | introduces |
may_discard | is_deterministic | is_reversible | has_effects |
required_capabilities`.

The first four and `required_capabilities` are unique non-empty string arrays in
Phase 0; the other three are booleans. Strings are provisional vocabulary, not
Phase 3 typed property atoms. `spec/fixtures/transform-contract.schema.json`
freezes the JSON field names under draft 2020-12 with unknown fields forbidden.
`spec/fixtures/transform-contract.examples.json` contains nondestructive and
explicitly accepted destructive witnesses.

Non-empty `may_discard` makes a transform destructive for matching input.
Execution MUST require explicit acceptance and MUST record the accepted
contract. Determinism does not prove safety. `has_effects` requires every named
capability and forces effect execution outside tracked queries. Reversibility
means enough information is retained for a checked inverse; it does not promise
unconditional time reversal.

## Incremental runtime

Pure expensive computation is a deterministic tracked query over immutable
syntax/graph values and an explicit Workspace Basis (v4 §§21–25, 122). Required
boundaries remain fixed regardless of the engine selected by the §122 ADR:

- immutable syntax and graph values;
- deterministic queries over declared inputs;
- all mutation and effects outside queries;
- component-granular and durability-informed invalidation;
- demand-driven computation and interned symbols;
- diagnostic accumulation as data;
- Datomic-like as-of/history reads;
- no hidden selection of a fresher or different Basis.

The production engine remains undecided between Salsa and a Liminal-specific
engine. Phase 0's `liminal-query` and `liminal-revision` are reference-scale
measurements, not that decision.

## Serialization and optimization boundary

Stable debug JSON is the only Phase 1 interim graph interchange candidate
permitted by v4 §121. It MUST be versioned, deterministic, migration-tested,
human-inspectable, and explicitly non-final. No packed binary, snapshot ABI, or
memory-mappable layout may freeze before in-memory and migration evidence.

Interpretive Contract dispatch, RepairPlan evaluation, Workspace Basis
selection, and ILRP remain conformance oracles (v4 §125). Compiled or indexed
paths require differential tests covering ordinary behavior, crash recovery,
and concurrent-buffer Basis cases. Phase 0 publishes this boundary; it does not
authorize optimization.

## Requirement-to-evidence map

| Requirement | Evidence |
|---|---|
| exact eight-field transform shape | `transform_contract_schema_roundtrips_all_fields` |
| malformed/unknown transform data fails closed | `transform_contract_schema_rejects_malformed_cases` |
| identity and projection claims equal evidence | `projection_laws_and_identity_ceilings_are_frozen` |
| annotated-source Level 2 | `anchor_recovery_report_golden`, `declared_level_matches_report` |
| Pandoc Level 1 | `pandoc_roundtrip_loss_report_golden`, `declared_level_matches_loss_report` |
| pure deterministic replay | `query_paths_are_effect_free`, `replay_from_frozen_basis_is_deterministic` |
| effect reactor outside queries | `reactor_is_effectful_and_outside_queries` |
