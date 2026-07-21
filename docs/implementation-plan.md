# Liminal implementation plan

**Status:** committed deliverable, maintained for the life of the project.
**Current phase:** Phase 0 — constitution, profiles, corpus, and conformance laws
(v4 Part XXII), authorized by ADR-0013 after the Phase -1 falsification gate.
**Canonical spec:** [`spec/v4/liminal_master_architecture_plan_v4.md`](../spec/v4/liminal_master_architecture_plan_v4.md) (cited as "v4 §N") and [`spec/v4/liminal_architecture_revision_4.md`](../spec/v4/liminal_architecture_revision_4.md) (cited as "R4 §N").

---

## 1. Purpose and how to read this document

Liminal is a multi-year project. This document is the on-ramp: it says what gets built, in what order, what is deliberately **not** built, and how "done" is decided at every step.

**Every Phase -1 milestone has an execution-grade work order** in
[`docs/execution/`](execution/00-protocol.md) — algorithms, schemas, constants,
output formats, and step-by-step checklists, pre-decided so an executor
implements without design work. This document stays the WHAT/WHY; the work
orders are the HOW. Phases 0–12 are decomposed in
[`docs/execution/phases.md`](execution/phases.md).

Three rules govern how to read it:

1. **The tests ARE the backlog.** The entire future test surface the spec enumerates (v4 §112 laws, §113 classes, §114 conformance suite, R4 §2.3 SLOs, R4 §10 gates) already exists in `conformance/` as named, compiled tests. Tests whose subject does not exist yet are `#[ignore = "Phase N: <what must exist>"]` with the exact assertion spelled out. There is no separate issue tracker for spec work: if the spec requires it, a named test already demands it.
2. **A milestone is done when its named tests flip from `#[ignore]` to passing.** Nothing else counts. Each milestone below lists the exact test names that constitute its exit gate. Un-ignoring a test without making it pass is meaningless; making it pass by weakening its assertion is a spec change and requires an ADR.
3. **The spec-debt meter is just gates.** `just gates` parses nextest output into per-phase `passed / ignored` counts. That number is the project's only progress bar. It goes down by doing the work, never by deleting tests.

Ordering follows **Law 14 (v4 §3): risk-retirement order precedes dependency order.** The build graph may want a parser before an editor, but the research program runs the architecture-killing experiments first. Phase -1 exists to *disprove* the architecture cheaply; everything in it is interpretive reference semantics, and everything that smells like production engineering is forbidden (see §4 below).

---

## 2. Phase -1 milestones M1–M12 (~18–20 weeks)

Milestones map onto the spec's Phase -1 subsections (v4 Part XXII, -1.0 through -1.5). The four **killer experiments** — the ones most likely to falsify the architecture, run in information-gain order per Law 14 — are M2, M4, M6, and M7 and are marked as such. If a killer experiment fails, the Contract is revised *before* any further milestone is attempted (v4 Part XXII, final Phase -1 gate).

Durations are honest estimates, not commitments; M9 is the only hard time-box.

| # | Weeks | Spec | Title | Work order |
|---|-------|------|-------|-----------|
| M1 | 1 | v4 §92 | Skeleton + honest durability substrate | [M01](execution/M01.md) |
| M2 | 2 | v4 §7.8, R4 §7 | **Killer #1** — ILRP coordinator + full crash matrix | [M02](execution/M02.md) |
| M3 | 1.5 | v4 §7.3, §7.9 | Interpretive Jurisdiction, two profiles, silent checker | [M03](execution/M03.md) |
| M4 | 2 | v4 §7.7, R4 §4–6 | **Killer #2** — repair DAGs, determinism ≠ safety, revert, Promotion | [M04](execution/M04.md) |
| M5 | 1 | v4 §7.10, R4 §9 | Overlay lifecycle + Reconciliation Queue + offline Holder | [M05](execution/M05.md) |
| M6 | 1.5 | v4 §7.5, R4 §8 | **Killer #3** — Basis Perspectives, anti-chimera, component invalidation | [M06](execution/M06.md) |
| M7 | 2 | v4 §-1.1, §19 | **Killer #4** — persistent-identity torture corpus | [M07](execution/M07.md) |
| M8 | 2 | v4 §-1.3, §8.6 | Revision, Basis selection, and invalidation prototype | [M08](execution/M08.md) |
| M9 | 2 (hard) | v4 §-1.2, §8.5, R4 §11.6 | Annotated-source + rich-editing spikes | [M09](execution/M09.md) |
| M10 | 1 | v4 §-1.4, §14 | Pandoc adapter-boundary spike | [M10](execution/M10.md) |
| M11 | 2 | v4 §-1.5, §7.4, R4 §2 | Held-out ergonomics + adversarial trace harness | [M11](execution/M11.md) |
| M12 | 1 | v4 Part XXII final gate | Final gate review, ADRs, go/no-go on Phase 0 | [M12](execution/M12.md) |

### M1 — Skeleton + honest durability substrate (1 week)

- **Goal:** everything that will later be crashed on purpose must first be honestly durable. No milestone builds on a store whose fsync discipline is untested.
- **Builds:** `liminal-graph/src/store/` — hand-rolled append-only NDJSON log (`<crc32-hex> <canonical-json>\n` framing, fsync-before-return, recovery truncates at the first bad line = longest checksummed prefix), atomic snapshot replacement (`tmp → fsync → rename → fsync(dir)`, v4 §92), fs4 single-writer advisory lock. `liminal-daemon/src/crash.rs` — `EnvCrashInjector` (see §3 below). First slice of the `conformance` `ToyRun` harness.
- **Exit gate:** `prop_torn_tail_truncates_cleanly` (proptest: truncate log at a random byte → reopen → exact checksummed prefix, no panic), `snapshot_replace_atomic_under_abort`, `fault_abort_exits_with_sigabrt` (proves the injector kills with a real SIGABRT, no unwinding, no flushes).

### M2 — **Killer #1**: ILRP coordinator + full crash matrix on single-step file promotion (2 weeks)

- **Goal:** falsify the Intent-Logged Repair Protocol (v4 §7.8, R4 §7) — the claim that a cross-Holder repair can be crash-resumed at *every* durable boundary and always terminate in `Committed`, `NeedsReview`, or `Aborted` with no hidden half-state, no duplication, and no lost bytes.
- **Builds:** `liminal-jurisdiction/src/ilrp.rs` made real — `IntentState` transitions, `IlrpDriver::{prepare, run, recover_all}`, `PrestateMatch::{Prestate, Poststate, Neither}` (`Neither` → stop, **never guess**). `liminal-source` staging (`stage()/commit_if(expected_pre)` with scannable `.lim-staged` siblings). The `promote_single_step` scenario driven through `liminald exec`, killed at every registered fault point per the crash-matrix machinery of §3.
- **Exit gate:** `crash_matrix_promote_single_step` (every `(point, occurrence)` from the baseline hit trace → crash → recover → terminal state, world digest correct, recovery idempotent), `recovery_never_guesses` (a third-party mutation between apply and recovery — the "third-state file" — must land in `NeedsReview` with zero bytes lost, never silently reconciled).
- **Failure consequence:** revises ILRP's contract (R4 §7) before anything else is built. This is why it runs second, not last.

### M3 — Interpretive Jurisdiction: two profiles, Contracts as data, silent checker (1.5 weeks)

- **Goal:** demonstrate Law 3E — the common case requires zero Jurisdiction authoring and zero sound-state noise — with an interpreter, not a compiler.
- **Builds:** `liminal-jurisdiction` `contract.rs` (Contracts as plain data, v4 §7.3), `profile.rs` (exactly two profiles: `ExternalFileProfile`, `GraphNativeProfile`), `checker.rs` (the eight questions of v4 §7.9 as methods). The toy paragraph format: blank-line-separated blocks plus optional `{#id}` suffix — a **~50-line hand parser that is the ENTIRE Phase -1 grammar** (no CST, no rope, no error recovery machinery).
- **Exit gate:** `sound_workspace_is_silent` (asserts exit 0 **and empty stdout and empty stderr byte strings** — not "no errors"), `eight_questions_golden` (checker answers for the toy workspace, insta-snapshotted), `capture_never_rejected` (Law 3B: no edit path returns a rejection).

### M4 — **Killer #2**: repair DAGs, determinism ≠ safety, revert, Promotion unification (2 weeks)

- **Goal:** falsify R4 §§4–6 — that Promotion is literally a `RepairPlan` through the one interpreter, that a *unique* textual merge result is not automatically *safe* (R4 §6), that repair mutations form dependency DAGs (R4 §5), and that accepted automatic repairs are one-command revertible.
- **Builds:** `liminal-jurisdiction/src/repair.rs` made real — `RepairPlan` execution over `topo_order()`, `SafetyRequirement::{StructuralDisjointness, DomainValidator, HumanApproval}`, `RepairDecision::{AutoApply, NeedsReview}` (no Reject variant exists), full `RepairRecord` evidence (all six R4 §6 fields), `plan_undo()` as a new Basis-checked plan (R4 §11.3 — never stale-byte restoration). `ClientSession::save` routed through the same interpreter: **save IS Promotion IS a RepairPlan** (R4 §4). The two-step DAG scenario: source-ID insertion must precede Relation reattachment (v4 §-1.0).
- **Exit gate:** `unique_unsafe_candidate_not_autoapplied`, `disjoint_safe_repair_autoapplies_logged_undoable`, `dag_id_insertion_precedes_reattach`, `crash_matrix_two_step_dag` (the full §3 crash matrix over the multi-step DAG), `promotion_observationally_equiv_repairplan`.
- **Failure consequence:** reshapes the repair/Promotion contract (R4 §§4–6) and possibly the Operation set, before Overlays or Bases are layered on top.

### M5 — Overlay lifecycle + Reconciliation Queue + offline Holder (1 week)

- **Goal:** demonstrate Law 3I — nonblocking capture never becomes buried data loss — using only the core queue, with the agenda domain provably absent (R4 §9).
- **Builds:** `liminal-jurisdiction` `overlay.rs` (`OverlayState::{Active, RepairProposed, Contested, Archived, Discarded}`; leaves the queue only via accepted repair, explicit discard, or searchable archive — v4 §7.10) and `reconcile.rs` (`ReconciliationItem` keyed by `root_cause`, R4 §9). The unavailable-write-route scenario producing a durable Overlay; `lim overlays [--all]` surfacing it.
- **Exit gate:** `offline_write_durable_and_visible`, `sound_session_zero_unexpected_items`, `auto_repair_on_holder_return`; plus the grep-test `no_agenda_symbols` (no agenda vocabulary exists anywhere in the workspace — enforced textually, not by convention).

### M6 — **Killer #3**: Basis Perspectives, three snapshots, component-granular invalidation (1.5 weeks)

- **Goal:** falsify Law 3J and R4 §8 — that two independently dirty client buffers over one durable file **never** merge into a chimeric Workspace Basis, and that invalidation follows selected Basis components rather than the whole input map.
- **Builds:** `liminal-revision` made real — `AvailableInputs::resolve()` with the anti-chimera rules (never another client's buffer; ambiguity → `DurableOnly` + one coalesced reconciliation item; requester-less computations fail closed to `DurableOnly`), per-client `epoch + generation` buffer components, `ComponentDeps::invalidated_by()`. The Neovim-buffer + phone-buffer + durable-file scenario from v4 §-1.0.
- **Exit gate:** `three_perspectives_three_snapshots` (`ClientScoped(neovim)`, `ClientScoped(phone)`, `DurableOnly` produce three *intentional* snapshots), `prop_no_chimeric_basis` (proptest over random edit/save/read interleavings of both clients — no reachable state exposes both dirty buffers to one computation), `invalidation_component_scoped` (bumping one buffer generation invalidates only dependent queries). Completing M6 un-ignores the **`gate::minus1_0`** suite — the twelve R4 §10 required-result tests.
- **Failure consequence:** reshapes the Basis Perspective model (v4 §7.5) before the revision prototype (M8) builds on it.

### M7 — **Killer #4**: persistent-identity torture corpus (2 weeks)

- **Goal:** falsify the identity-grade claims (v4 §19, Law 12, Law 13) empirically: no strategy may claim stronger continuity than the corpus demonstrates (v4 §-1.1).
- **Builds:** the identity lab in `conformance/` — **6 strategies** (inline IDs, sidecars, immutable content hashes, structural matching, revision anchors, managed graph IDs) × **11 operations** (rename/move, split, merge, copy/paste, duplicate identical blocks, delete+recreate, formatter rewrites, arbitrary external-editor changes, git merge, git rebase, git cherry-pick with conflict resolution) × seeds → an identity guarantee matrix. Git operations use the **real `git` CLI in hermetic temp repos** (`GIT_CONFIG_GLOBAL=/dev/null`, fixed dates/author, pinned version) — explicitly **not** git2/gitoxide, because they do not reproduce merge-ort behavior, and merge-ort is what users' files actually go through.
- **Exit gate:** `matrix_matches_golden` (byte-exact golden of the guarantee matrix), `claims_never_exceed_evidence` (every declared `IdentityGrade` ≤ the worst demonstrated outcome for that strategy — the spec's "no strategy may claim stronger continuity than the corpus demonstrates" made executable).
- **Failure consequence:** demotes identity grades in the standard profiles (v4 §7.6) and feeds directly into what M9 and Phase 0 may honestly promise.

### M8 — Revision, Basis selection, and invalidation prototype (2 weeks)

- **Goal:** v4 §-1.3 — prove that as-of reads, a pure query layer, explicit durability/freshness inputs, and an effect reactor compose over the toy store while two clients hold different dirty buffers.
- **Builds:** as-of reads over the toy store's transaction log; `liminal-query` promoted from stub to a tiny **pure** incremental query layer (effects live outside queries, v4 §8.6); an effect reactor that injects observations as transactions; durability classes on inputs. Four consumers exercised against two dirty clients: an AI context build, a cross-document query, an interactive preview, and a durable export (each pinned to its correct Perspective).
- **Exit gate:** `four_consumers_two_dirty_buffers`, `replay_from_frozen_basis_is_deterministic` (pure queries replay byte-identically from a frozen Workspace Basis — a final-gate requirement).

### M9 — Annotated-source + rich-editing spikes (2 weeks, **hard time-box**)

- **Goal:** v4 §-1.2 / R4 §11.6 — measure, not assume, the highest-risk projection: `source bytes ↔ annotations/HIR ↔ Resolved Graph IR`, and independently `graph operations ↔ rich editor transaction model`. Exercise inline marks, links, comments, list restructuring, selections, undo, malformed source, concurrent edits, and foreign edits.
- **Builds:** two throwaway spikes (not production code, not kept APIs) plus an honest anchor-recovery report per projection capability level (v4 §8.4).
- **Exit gate:** `anchor_recovery_report_golden`, `declared_level_matches_report` (the capability level a profile declares equals what the report demonstrates).
- **Failure consequence:** **demotes the annotated-source profile, not the constitution** — the profile stays experimental (v4 §8.5) and Phase 9 clients expose only the proven level. The time-box is hard because this spike can absorb unlimited effort; its purpose is a measurement, not a product.

### M10 — Pandoc adapter-boundary spike (1 week)

- **Goal:** v4 §-1.4 — lower representative Resolved Graph IR into a Pandoc AST and back via the `pandoc` CLI, recording semantic loss, ID survival, foreign-node preservation, and the exact projection capability level achieved (Law 8: every conversion declares loss).
- **Builds:** a thin adapter spike in `conformance/` fixtures; the first real entry in `tests/classes/conversion_loss.rs`.
- **Exit gate:** `pandoc_roundtrip_loss_report_golden`.

### M11 — Held-out ergonomics + adversarial trace harness (2 weeks)

- **Goal:** v4 §-1.5 / §7.4 / R4 §2 — make Jurisdiction ergonomics *measurable* over operations and sessions the profiles were never tuned on. A profile cannot enter the stable set by passing only self-authored fixtures.
- **Builds:** NDJSON trace format; importers/generators for real Git histories and merge resolutions, consented/anonymized editor sessions, foreign formatter rewrites, offline/online transitions, and independent adversarial mutations; the v4 §7.4 denominators implemented exactly (semantic-transaction coalescing, 30-minute session inactivity timeout, root-cause incident coalescing); dev/held-out corpus split with a `MANIFEST.sha256`; `slo.rs` scorecard computing all six R4 §2.3 per-profile metrics.
- **Exit gate:** `denominator_counts_golden` (**freezes the denominators BEFORE any profile tuning** — changing a denominator afterward is a corpus-version bump, not an edit), `heldout_manifest_locked` (CI hash check; already real day one), `scorecard_runs_end_to_end`.

### M12 — Final gate review (1 week)

- **Goal:** answer the only question Phase -1 exists to answer: does the architecture survive its own falsification laboratory?
- **Builds:** `gate::minus1_final` — one assertion per bullet of the spec's final Phase -1 gate (v4 Part XXII): every toy subject names its Holder; facets reified as Nodes/Relations; no false identity promises for anonymous text; at least one canonical round-trip projection with richer lens limits measured honestly; cross-Holder repair ordered/crash-resumable/idempotent/revertible; no chimeric Basis; Overlay aging surfaced agenda-free; deterministic replay from frozen Bases; frozen denominators and a locked acceptance split. Plus: an ADR for **every** Contract revision the lab forced, and the explicit go/no-go decision on Phase 0.
- **Exit gate:** `gate::minus1_final` passes; ADR trail complete. Failure of any bullet revises the Contract before any production parser, daemon, or compiled Jurisdiction optimization begins — that sentence is the spec's, and it is binding.

---

## 3. Crash-injection architecture

Crash testing is the backbone of M1–M6, so its rules are fixed here (v4 §92 requires daemon-kill tests at every repair transition; R4 §10 requires termination after every ILRP durable boundary).

**Mechanism — in-process fault points, real subprocess death (primary).**

- Fault points are registered at the ILRP durable boundaries: `ilrp/before_intent_commit`, `ilrp/after_intent_commit`, `ilrp/before_external_apply`, `ilrp/after_external_apply`, `ilrp/before_ack`, `ilrp/after_ack`, `ilrp/before_finalize`, `ilrp/after_finalize_before_notify` (matching `CrashPoint` in `liminal-jurisdiction::ilrp`). Points are **occurrence-counted, not name-exploded**: a two-step DAG hits `after_external_apply` twice, addressed as occurrence 1 and 2 — new steps never require new point names.
- Arming: the conformance harness spawns `liminald exec <scenario>` with `LIMINAL_CRASHPOINT=<point>[:<occurrence>]` (occurrence defaults to 1). `EnvCrashInjector` (`liminal-daemon/src/crash.rs`) calls `std::process::abort()` on match — a real SIGABRT, no unwinding, no destructor flushes, so nothing "accidentally durable" survives.
- **Hit-trace proves firing.** Every fault-point hit (matched or not) is appended (O_APPEND) to the file named by `LIMINAL_CRASH_TRACE`. `ToyRun::run_to_crash` asserts both the SIGABRT exit *and* that the armed point appears in the trace — a crash test whose point never fired is a broken test, not a passing one.
- **The crash matrix is DERIVED, never hand-listed.** `ToyRun::baseline` runs the scenario with no injector and records the full hit trace; `crash_matrix()` turns every observed `(point, occurrence)` into a crash case automatically. Adding a durable boundary to a scenario therefore *cannot* be silently untested.
- **Recovery idempotence.** Every matrix case recovers twice; the world digests after first and second recovery must be equal (recovery that only works once is not recovery).
- **Meta-test:** every fault point registered in code must appear in at least one scenario's baseline trace — an unreachable fault point is dead spec.

**Deferred alternatives (recorded so they are not re-litigated):**

- *Real-daemon SIGKILL handshake* — deferred to **Phase 2**, when a persistent `liminald` exists. It reuses the same boundary names, so the Phase -1 matrix transfers instead of being rewritten.
- *HolderIo write-reordering simulation* (testing against filesystems that reorder writes without fsync barriers) — deferred until after Phase -1; SIGABRT-at-boundary already tests the fsync discipline the toy store claims (v4 §92).

---

## 4. What NOT to build

Fourteen spec-anchored exclusions. Each is a standing prohibition for all of Phase -1; several are mechanically enforced. Violating one is an ADR-level event, not a judgment call.

| # | Excluded | Spec | Why |
|---|----------|------|-----|
| 1 | `CompiledJurisdictionPlan`, indexed dispatch, generated policy code | v4 §125, Law 14 | The interpreters are the conformance oracles; compiling semantics that Phase -1 exists to reshape would freeze bugs into an optimization layer. |
| 2 | Any CRDT or OT implementation | R4 §11.7, v4 §88 | A Federated profile *delegates* to a real merge runtime; hand-rolling collaboration machinery is precisely the complexity Jurisdiction exists to name, not absorb. |
| 3 | Real parser, CST, or rope | v4 §9, Part XXII Phase 1 | The ~50-line toy grammar (M3) is the entire Phase -1 surface; parser engineering is known territory and would postpone falsification (Law 14). |
| 4 | Agenda subsystem | R4 §9 | The core Reconciliation Queue must precede agenda so surfacing never depends on a later domain — enforced by the `no_agenda_symbols` grep-test. |
| 5 | Sync, cloud, or Git-as-sync | v4 §88, Part XXII Phase 6 | Replica convergence is meaningless before a merge model is chosen, and Git-as-sync smuggles in exactly the merge semantics M7 exists to measure. |
| 6 | Real editors or the Liminal Document Protocol | v4 §58, Part XXII Phase 2 | Scripted `ClientSession`s in the toy harness exercise every Basis and repair path without an editor's worth of accidental complexity. |
| 7 | Persistent socket daemon | Part XXII Phase 2 | `liminald` is exec-and-exit by design: the crash matrix needs process *death* at boundaries, not uptime engineering. |
| 8 | Production store | v4 §92, ADR-0007 | The toy store exists to be crashed and inspected (NDJSON, segments never GC'd); making it fast or general is optimization before the model survives. |
| 9 | Performance work, salsa, benchmarks as gates | Law 14, v4 §116, §125 | Baselines are recorded from day one, but optimization and CI regression gating begin only when the Phase 1 vertical slice exists. |
| 10 | `FacetId` or any third primitive | v4 §7.2, Law 1 | A facet is schema shorthand; anything needing independent identity, history, or Jurisdiction must be reified as a Node or Relation — the final Phase -1 gate audits exactly this. |
| 11 | Rich UI, badges, notifications | R4 §3, Part XXII Phase 9 | Phase -1's required surfaces are the CLI and contextual queue in domain-native vocabulary; UI polish cannot falsify anything. |
| 12 | Schemas, macros, transforms, plugins, AI, HTML backend | v4 §93, Part XXII Phases 1/3/10/11 | None of these can kill the kernel questions; each is a phase-gated deliverable with its own ignored tests already waiting. |
| 13 | Overlay GC or archival UI | v4 §7.10, Law 3I | Overlays are never silently garbage-collected; "archive" is a searchable state transition, and building deletion tooling now invites burying debt. |
| 14 | Tuning against the held-out corpus | v4 §7.4, R4 §2.2 | Scores-seen burns the corpus version (new `heldout/vN`, old scores retained); `heldout_manifest_locked` makes this CI-enforced, not honor-system. |

---

## 5. Defer list with activation triggers

Infrastructure that is deliberately absent, with the exact event that activates it. Nothing on this list is added early "while we're at it."

| Deferred item | Activation trigger |
|---|---|
| Production `fuzz/` targets (`cst_parse`, `format_idempotent`) | First parser crate lands (`liminal-cst`, Phase 1). AM-17.2 separately permits Phase-0-only HAQP reference-harness targets before authorization; they cannot depend on or instantiate production parser code. |
| Benchmark regression gating (CodSpeed via `codspeed-divan-compat`) | Phase 1 vertical slice lands |
| `cargo-semver-checks` + reproducible-package verification | First `crates.io` publish |
| SBOM / `cargo-auditable` | First distributed binary |
| `wasm32` CI target | First wasm crate (`liminal-wasm-host`, Phase 9 track) |
| Migration tests (populate `fixtures/migration/`) | First persisted-format ADR |
| Code coverage in CI | Phase 0 |
| `xtask` (replacing pure-justfile automation) | Corpus tooling at -1.5 (M11) needs real logic |
| mdBook docs site | Phase 0 exit |
| `CHANGELOG` / `release-plz` | First tag |
| Branch protection rules | First external contributor |
| Lint escalation (`missing_docs`, `unwrap_used` → deny) | Per-crate, at that crate's phase gate |

---

## 6. Phase 0–12 summary

The multi-year arc, one row per phase (compressed from v4 Part XXII; the spec text is authoritative). Every deliverable and gate below already has ignored named tests in `conformance/` tagged with its phase.

| Phase | Headline deliverables | Exit gate (compressed) |
|---|---|---|
| **-1** | Falsification laboratory (this document, §2) | Final gate bullets pass (`gate::minus1_final`); any failure revises the Contract first |
| **0** | Kernel spec (Node/Relation); Jurisdiction Contract interpreter + Workspace Basis spec; tiered vocabulary; held-out profile-coverage conformance; mutation-local repair law; unified RepairPlan/Promotion; ILRP spec + recovery fixtures; Overlay aging + queue rules; Basis Perspectives; identity grades; projection levels + lens laws; transform contracts; fixture format; torture corpus; ADR process; threat model; prior-art decisions | Kernel represents prose/code/table/image/audio/external value with **no new primitive**; every example resolves via a standard profile or marked boundary Contract with a declared identity guarantee; stable profiles auto-resolve ≥99% held-out operations, leave ≥99% sessions intervention-free; sound sessions produce zero diagnostics, zero unexpected reconciliation items, zero requests for user-authored Contracts |
| **1** | External-file source→HTML vertical slice: rope-backed Holder source, error-tolerant CST, annotation/source-map layer, Human IR, derived resolved graph, compact + explicit syntax, `lim fmt/check/expand`, native HTML backend, incremental-vs-full equivalence tests, basic CLI | Paragraph edit updates only its semantic/HTML dependents; canonical formatting deterministic + idempotent; graph explicitly derived for this profile; interpreters remain the reference semantics — optimized plans still deferred |
| **2** | `liminald` persistent daemon; Liminal Document Protocol; buffer/file-hash/Git-basis tracking; file watching; revisioned pure query engine; diagnostics; incremental HTML preview; Neovim Lua client; node inspection; identity-grade reporting; references/backlinks | Markdown-compatible file edited in Neovim with preview + indexes synchronized to the exact buffer/file basis; save, external modification, and Git checkout produce explicit Promotions or Holder changes |
| **3** | Graph rewrite IR; declarative macros; Lua host; semantic command API; pseudo-stenographic macro packs (Rust + prose); expansion tracing; structural edits; pass-fusion prototype | Same semantic macro invocable from Neovim, CLI, and test harness without keymap dependence; accepted semantic edits round-trip through source and reparse to the intended result |
| **4** | `Liminal.toml` + lockfile; multi-document workspace graph; build dependency graph; Cargo, LSP virtual-document, and Pandoc AST adapters; content-addressed resources; search/relation indexes; `lim build/serve/query` | One workspace holds Rust code, docs, images, and an academic note, compiles to HTML, links source symbols to prose Nodes, preserving each domain's Jurisdiction Contract |
| **5** | Prose/outline, org/task/agenda, academic (citations/equations), table/formula, notebook-metadata, and book domains; one graph-native document profile; conversion-loss diagnostics | Notes + tasks + paper + code + computed tables coexist without undeclared competing Holders; graph-native pilot satisfies its declared projection capability level |
| **6** | Immutable graph revisions + as-of/history queries; semantic transaction log; snapshots/compaction; undo/redo + revision browsing; Git snapshot integration; replica identity; Automerge/Peritext evaluation + adapter where appropriate; encrypted resource sync; conflict diagnostics; backup/recovery | Two offline replicas edit supported profiles, reconnect, converge, retain auditable history; unsupported cross-profile merges fail **visibly**, never pretend safety |
| **7** | Effect reactor + resolver runtime; capability model; pointer/snapshot/cache/mirror/live/materialized/replicated/derived policies; freshness + availability; Basis recording; freeze command; DB + HTTP resolvers; demo external Relations | Documents useful offline with honest staleness; declared-policy sync with no hidden effects; frozen resolver inputs replay deterministically |
| **8** | Image/region model; PDF annotation Relations; audio recording/chunking; transcript Nodes + timeline Relations; lecture mode; raw ink store; SVG preview; page/canvas models; voice/dictation adapter | One lecture session combines notes, ink, audio, transcript, slides, timestamps in one graph, reconstructable offline; raw-media vs derived-recognition Jurisdictions stay distinct |
| **9** | Core to Wasm where practical; worker-based compiler; rich graph transaction adapter; incremental DOM patches; capture-first phone client; pen-first tablet client; partial graph/resource sync | Supported graph-native documents edit across web/phone/tablet without semantic divergence; external-file documents expose **only the projection level proven in Phase -1** |
| **10** | AI MIR; model profiles; context planner; Markdown/tagged/JSON/graph projections; multimodal bundles; semantic-operation output; provenance + approval workflow; evaluation harness; local + remote model adapters | AI receives a task-scoped subgraph with exact Workspace Basis and returns revision-safe operations validated under the target domain's Jurisdiction Contract |
| **11** | Versioned Wasm component API; stable Lua semantic API; trusted-native guide; process plugin protocol; package resolution + lockfile; conformance certification; migration framework; docs + examples | A third party adds a dialect, resolver, renderer, Jurisdiction profile, or transformation without depending on internal Rust layouts |
| **12** | Large-scale table engine; constraint layout; slide animation timeline; diagram routing; advanced handwriting recognition; data visualizations; optional Cranelift acceleration | Performance-critical domains prove semantic uniformity coexists with radically specialized execution and explicit Jurisdiction |

---

## 7. Store fallback note (ADR-0007)

The Phase -1 durable store is **hand-rolled** per v4 §92 — checksummed append-only NDJSON log plus atomic snapshot replacement — and deliberately not redb/rusqlite, because the ILRP crash matrix must exercise *Liminal's own* fsync discipline: a battle-tested embedded database would make the crash tests measure someone else's durability, not the protocol under falsification.

**Recorded fallback trigger** (ADR-0007, [`docs/adr/0007-hand-rolled-phase-minus-1-toy-store.md`](adr/0007-hand-rolled-phase-minus-1-toy-store.md)): if the log + snapshot + recovery implementation exceeds **~1 week** of effort (the M1 budget), switch to `rusqlite` for storage while keeping the ILRP fault points at exactly the same durable boundaries — the crash matrix, boundary names, and hit-trace machinery of §3 are store-agnostic by construction. The fallback trades fsync-discipline coverage for schedule and is taken only by amending ADR-0007, never silently.
