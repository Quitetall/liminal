# IR — multi-level intermediate representations and the incremental compiler

**Scope.** The MLIR-like tower of IR levels (L0–L4), the transform contracts
every pass must declare, and the compiler runtime that moves documents
through the tower: pipeline shape, coarse scanning, demand-driven parsing,
the incremental query engine, the pass manager, lazy dialect loading, and
the HTML fast path. Corresponds to v4 Part IV (§9–§14) and Part VI
(§21–§27).

This file is a curated index into the canonical text in `spec/v4/`. On any
conflict the canonical text wins. Citation forms: `v4 §N`, `R4 §N`.

## Part IV — The multi-level IR

- **L0 — Bytes and Concrete Syntax Tree** — a lossless, error-tolerant CST
  preserving exact bytes, trivia, invalid syntax, and source ranges, with
  immutable green trees and structural sharing recommended. v4 §9.
- **L1 — Liminal Human IR** — authorial intent and domain sugar: declared
  identity grades, source anchors, unexpanded macros, unresolved
  references; the main plugin and editor-semantic layer. v4 §10.
- **L2 — Liminal Resolved Graph IR** — the normalized Node-and-Relation
  graph at an explicit Workspace Basis; Liminal's semantic normal form,
  derived in external-file Jurisdiction domains and governing in
  graph-native ones. v4 §11.
- **L3 — Domain dialect IRs** — prose, org/task, academic, code, notebook,
  table, slide, ink, media, resolution, and build dialects as derived views
  over the same resolved graph at a declared basis. v4 §12.
- **L4 — Backend IRs** — per-target lower IRs (HTML/DOM, CSS/layout, PDF,
  Pandoc AST, terminal cells, accessibility, AI MIR, search, sync plan,
  execution, Wasm); preserve abstractions per stage, then lower
  deliberately. v4 §13.
- **Transform contracts** — every pass declares `requires` / `preserves` /
  `introduces` / `may_discard` / `is_deterministic` / `is_reversible` /
  `has_effects` / `required_capabilities`, and destructive conversions warn
  unless the user accepts the contract (Law 8, v4 §3). v4 §14; the rewrite
  calculus that executes transforms is indexed in `spec/transforms.md`.

## Part VI — Compiler and incremental runtime

- **Compilation pipeline** — source bytes → coarse scan → lossless CST →
  Human IR → macro expansion → schema/default resolution → Resolved Graph
  IR → domain analysis → backend lowering. v4 §21.
- **Coarse structural scan** — cheaply identify blocks, headings, fences,
  embedded regions, and potential node boundaries; unneeded regions remain
  `OpaqueBlock` stubs. v4 §22.
- **Demand-driven parsing** — fully parse only what is visible, edited,
  queried, referenced, or required; large workspaces are never eagerly
  elaborated in full. v4 §23.
- **Incremental query engine** — every expensive pure computation is a
  revisioned query over an explicit Workspace Basis; external resolution is
  deliberately *outside* the query engine (effects submit new transactions,
  queries consume accepted inputs). v4 §24; durability and volatility
  classes as an invalidation optimization, never a truth policy, §24.1.
  Prior-art boundary (Salsa) v4 §118A; component-granular Basis
  invalidation R4 §8.
- **Pass manager** — passes declare dependencies, contracts, and fusibility;
  the optimization catalogue (expansion erasure, constant folding, traversal
  fusion, direct dispatch, …). v4 §25.
- **Lazy dialect loading** — a dialect package contributes syntax, kinds,
  rules, defaults, transforms, renderers, and capabilities, and is loaded
  only when the manifest, source, graph, or active target requires it.
  v4 §26.
- **HTML fast path** — the first-class hot path is Markdown-compatible
  source → incremental graph update → HTML/DOM patch; a paragraph edit never
  recompiles the document, and preview never shells out to Pandoc per
  keystroke. v4 §27.

Phase -1 note: none of this tower is built yet. Law 14 (v4 §3) and R4 §10
forbid production parsers, incremental engines, and every §25 optimization
until the interpretive reference semantics survive the falsification gates;
Phase -1 carries only the vocabulary types (e.g. `OpaqueBlock`) and the
Basis-selection prototype (v4 Part XXII, -1.3).

Status: index only — becomes a self-contained chapter at Phase 0.
