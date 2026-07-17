# Phases 0–12 — work breakdown structure

Tiered-fidelity rule: Phase -1 has full mechanical work orders (`M01.md`–`M12.md`).
The phases below are decomposed to task level — enough to size and sequence the
work — and each phase's **mechanical work orders are authored at the preceding
gate, using [`template.md`](template.md)**. Writing them earlier would be fiction:
the falsification law (Law 14) means earlier phases reshape later ones.

Every phase's gating tests either already exist as phase-tagged `#[ignore]`d
tests in `conformance/` (named below) or are authored born-ignored at the
preceding gate. `just gates` tracks both.

Pre-known ADR points come from spec v4 Part XXIII (§119–§131), pinned here to the
phase that must decide them.

---

## Phase 0 — Constitution, profiles, corpus, conformance laws

**Inputs:** the go/no-go ADR + full ADR trail; frozen Contract types
(`liminal-jurisdiction`); the M07 identity matrix + grade ADR; M09/M10 capability
reports + level ADR; the M11 corpus, denominators, and scorecard pipeline;
`template.md`.

**Tasks:**
1. Write the kernel spec (Node/Relation) from the live types into `spec/kernel.md`.
2. Freeze the interpretive Contract interpreter + Workspace Basis spec as
   normative text.
3. Author the tiered-vocabulary doc (domain-native UI ceiling, R4 §3).
4. Extend held-out coverage per stable profile (heldout/v2 if new trace
   categories — v1 scores retained).
5. Publish the mutation-local repair law + unified RepairPlan/Promotion semantics.
6. Publish the ILRP spec + recovery fixtures from `fixtures/ilrp-crash-matrix/`.
7. Publish Overlay-aging + queue rules; identity grades + dual entity/version
   model; projection levels + lens laws.
8. Encode the §14 transform-contract model as data.
9. Publish fixture-format + torture-corpus documentation.
10. Write the threat model (supersedes SECURITY.md's placeholder).
11. Record prior-art adoption decisions (v4 §118A).

**Gating tests:** all of `phase_minus_1.rs` and both
`slo::*_graduates_on_heldout_corpus` stay green; `heldout_manifest_locked`;
born-at-gate: a no-new-primitive representation test (prose/code/table/image/
audio-interval/external-value via Node+Relation only), an
every-example-resolves test, and the sound-session
zero-diagnostic/zero-item/zero-authoring test.

**Pre-known ADRs:** §119 source grammar (decide BEFORE the Phase 1 CST); §122
Salsa-vs-custom (before the Phase 1 query engine); §125 optimization boundary
reaffirmed; §121 interim: stable debug JSON only.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 1 — External-file source→HTML vertical slice

**Inputs:** Phase 0 kernel + grammar ADR (§119); the toy-grammar semantics as the
conformance floor; the interpreters as reference oracles (§125).

**Tasks:** rope-backed Holder source; error-tolerant CST (`liminal-cst`);
annotation/source-map layer (AtJSON-inspired); Human IR (`liminal-hir`); derived
resolved graph snapshot with source basis; compact Markdown-compatible + explicit
syntaxes; `lim fmt` / `lim check` extension / `lim expand` (pre-known amendment:
`lim` subcommand additions); native HTML backend; full-vs-incremental
equivalence harness; activate `fuzz/` targets + benchmark regression gating (the
defer-list triggers fire here).

**Gating tests:** `laws::formatter_idempotence_law_holds`,
`laws::canonical_round_trip_law_holds`,
`laws::incremental_equals_full_compile_law_holds`,
`classes::malformed_source::{malformed_source_never_panics_and_round_trips,
fuzz_regressions_stay_fixed}`,
`classes::golden_render::{full_document_html_matches_golden,
incremental_patch_equals_full_render}`; plus
`classes::migration::old_snapshots_open_after_format_evolution` once the first
persisted-format ADR lands.

**Pre-known ADRs:** §121 (first persisted format — triggers migration fixtures);
§125 (compiled plans still deferred).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 2 — Persistent daemon + Neovim

**Inputs:** the Phase 1 slice; the crash-matrix boundary names (transferring to
the real-daemon SIGKILL handshake per implementation-plan §3); the LDP sketch
(v4 §58).

**Tasks:** persistent `liminald`; Liminal Document Protocol;
buffer-version/file-hash/Git-basis tracking; file watching; revisioned pure
query engine (per the §122 ADR); diagnostics channel; incremental HTML preview;
Neovim Lua client; node inspection + identity-grade reporting;
references/backlinks; the real-daemon SIGKILL crash handshake reusing the
Phase -1 boundary names.

**Gating tests:** authored at the Phase 1 gate; must include: Neovim edit with
preview/indexes synchronized to the exact buffer/file basis; save, external
modification, and Git checkout producing explicit Promotions or Holder changes;
the SIGKILL matrix over daemon boundaries.

**Pre-known ADRs:** LDP wire-format ADR expected.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 3 — Transform + macro infrastructure

**Inputs:** the Phase 2 daemon + semantic command channel; the §14
transform-contract data model (Phase 0).

**Tasks:** graph rewrite IR; declarative macros; Lua host (`liminal-lua`);
semantic command API; pseudo-stenographic packs (Rust + prose); expansion
tracing; structural edit operations; pass-fusion prototype.

**Gating tests:** authored at the Phase 2 gate; must include: the same semantic
macro invocable from Neovim, CLI, and the test harness without keymap
dependence; accepted semantic edits round-tripping through source and reparsing
to the intended result.

**Pre-known ADRs:** none pre-assigned; the macro-ABI draft feeds §127.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 4 — Workspace + build graph

**Inputs:** the M10 spike + its Level-1 loss contract (promoted to the real
Pandoc AST adapter); Phase 3 transforms.

**Tasks:** `Liminal.toml` + lockfile; multi-document workspace graph; build
dependency graph; Cargo adapter; LSP virtual-document adapter; Pandoc AST
adapter honoring the M10 declared-loss contract; content-addressed resources
(`liminal-resource`); search/relation indexes; `lim build`/`lim serve`/
`lim query` (pre-known amendment: `lim` subcommands).

**Gating tests:**
`classes::conversion_loss::{unknown_constructs_are_preserved_opaquely,
destructive_conversion_warns_first}`,
`classes::large_workspace::opening_one_doc_elaborates_only_demanded_regions`,
`classes::object_corruption::corrupt_object_bytes_fail_hash_verification`.

**Pre-known ADRs:** §128 (initial layout delegation per target).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 5 — Core domains + graph-native pilot

**Inputs:** the Reconciliation Queue (R4 §9) as the agenda's projection source;
the M09/M10 declared-vs-measured pattern.

**Tasks:** prose/outline domain; org/task + agenda domain (**agenda PROJECTS the
queue — retire `no_agenda_symbols` by ADR here, never silently**); academic
citations/equations; table/formula; notebook execution metadata; book/long-form;
one graph-native document profile; conversion-loss diagnostics.

**Gating tests:** authored at the Phase 4 gate; must include: a
no-undeclared-competing-Holders workspace test; the graph-native pilot meeting
its declared projection level (declared-vs-measured pattern).

**Pre-known ADRs:** domain schema conventions; feeds §127.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 6 — History + local-first sync

**Inputs:** the M07 matrix (merge-semantics evidence); the Phase -1 transaction
log; the §121 interim format.

**Tasks:** immutable graph revisions + as-of/history queries; semantic
transaction log; snapshots/compaction; undo/redo + revision browsing; Git
snapshot integration; replica identity; Automerge/Peritext evaluation + adapter
where appropriate; encrypted resource sync; conflict diagnostics;
backup/recovery tooling.

**Gating tests:** `laws::semantic_diff_matches_transaction_law_holds`,
`laws::replicas_converge_law_holds`,
`classes::sync_partition::{partitioned_replicas_converge_with_auditable_history,
unsupported_cross_profile_merge_fails_visibly}`,
`classes::migration::old_snapshots_open_after_format_evolution`.

**Pre-known ADRs:** §123 collaboration (Automerge/Peritext vs custom — BEFORE any
CRDT code); §121 final graph serialization; §130 history
retention/compaction/privacy.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 7 — Intelligent external Relations

**Inputs:** the M08 effect-reactor prototype; the frozen-basis replay machinery.

**Tasks:** effect reactor + resolver runtime; capability model; the relation
policies (pointer/snapshot/cache/mirror/live/materialized/replicated/derived);
freshness + availability state; Basis recording; the freeze/reproducibility
command; DB + HTTP example resolvers; stock-price + reading-progress demo
Relations.

**Gating tests:** `laws::resolver_replay_deterministic_law_holds`,
`classes::plugin_capability::relation_traversal_performs_no_effects`.

**Pre-known ADRs:** §126 relation policy language.

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 8 — Multimodal + lecture stack

**Inputs:** content-addressed resources (Phase 4); the timeline requirements of
v4 §§74–77.

**Tasks:** image/region model; PDF annotation Relations; audio
recording/chunking; transcript Nodes + timeline Relations; lecture mode; raw ink
store; SVG preview; page/canvas models; voice/dictation adapter.

**Gating tests:** authored at the Phase 7 gate; must include: one lecture session
reconstructable offline; raw-media vs derived-recognition Jurisdictions distinct.

**Pre-known ADRs:** none pre-assigned (§128 revisit for page/canvas allowed).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 9 — Rich web, phone, tablet

**Inputs:** **the M09 anchor-recovery report + M10 loss report — clients expose
ONLY the proven projection level** (spec gate); the wasm32 CI defer-trigger
fires.

**Tasks:** core to Wasm where practical; worker-based compiler; rich graph
transaction adapter; incremental DOM patches; capture-first phone client;
pen-first tablet client; partial graph/resource sync.

**Gating tests:** authored at the Phase 8 gate; must include: no semantic
divergence across clients for graph-native docs; external-file docs surfacing
exactly the Phase -1-proven capability level.

**Pre-known ADRs:** §124 revisited with conformance evidence (no
universal-bidirectionality promise without it).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 10 — AI compiler + semantic operation loop

**Inputs:** Basis recording (Phase 7); the provenance model (Law 15); the M08
AI-context-build consumer.

**Tasks:** AI MIR; model profiles; context planner; Markdown/tagged/JSON/graph
projections; multimodal bundles; semantic-operation output; provenance +
approval workflow; evaluation harness; local + remote model adapters.

**Gating tests:** `classes::ai_operations::{stale_ai_operations_are_refused,
ai_content_stays_derived_until_approved}`.

**Pre-known ADRs:** §129 AI projection metrics (benchmarks BEFORE
tokenizer-optimized encodings).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 11 — Stable plugin ecosystem

**Inputs:** every internal ABI candidate from Phases 3–10; the conformance suite
as the certification instrument.

**Tasks:** versioned Wasm component API; stable Lua semantic API; trusted-native
guide; process plugin protocol; package resolution + lockfile; conformance
certification; migration framework; docs + examples.

**Gating tests:**
`classes::plugin_capability::plugin_without_capability_is_denied_and_logged`;
a third-party-dialect end-to-end test authored at the Phase 10 gate.

**Pre-known ADRs:** §127 schema/dialect ABI; §131 licensing + governance
(certification rules).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*

## Phase 12 — Advanced spatial/spreadsheet/presentation runtimes

**Inputs:** all declared Jurisdiction profiles; benchmark baselines (recorded
since M1, gating since Phase 1).

**Tasks:** large-scale table engine; constraint layout; slide animation
timeline; diagram routing; advanced handwriting recognition; data
visualizations; optional Cranelift acceleration.

**Gating tests:** authored at the Phase 11 gate; performance benchmarks may gate
here; must include: specialized execution preserving semantic uniformity +
explicit Jurisdiction.

**Pre-known ADRs:** §128 final layout delegation; §125 differential-equivalence
proof if compiled Jurisdiction plans are pursued (including injected-crash +
concurrent-buffer cases).

*Mechanical work orders for this phase are authored at the preceding gate, using
template.md.*
