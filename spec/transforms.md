# Transforms — rewrite calculus and the macro system

**Scope.** The transformation runtime: the graph rewrite calculus that is
the Turing-complete operational core of editing, the six macro levels, the
pseudo-stenographic command grammar, context and precedence, discoverability,
semantic macro recording, implementation tiers, and the optimization
pipeline. Corresponds to v4 Part VII (§28–§35), with the transform contracts
of v4 §14 as the governing interface.

This file is a curated index into the canonical text in `spec/v4/`. On any
conflict the canonical text wins. Citation forms: `v4 §N`, `R4 §N`.

## Part VII — Transformation and macro system

- **Graph rewrite calculus** — a transformation matches, binds, creates,
  deletes, rewires, iterates, composes, and requests effects through
  capabilities; it models every editing operation, compiler pass, formatter,
  formula, query, macro, refactoring, and AI edit, yet is imperative
  infrastructure rather than a third semantic primitive because programs and
  results are graph structures (Law 1, v4 §3). v4 §28.
- **Macro levels** — literal, snippet, syntax-aware, structural, graph, and
  workflow macros, in increasing semantic depth. v4 §29.
- **Pseudo-stenographic command grammar** — compositional
  `construct + modifier + modifier + target` phrases (e.g. `fpr` = public
  Result-returning function); editors own keybindings and chords, Liminal
  owns the portable semantic commands. v4 §30.
- **Context and precedence** — resolution order from buffer override down to
  global fallback, with context from language, syntax node, selection,
  schema, conventions, mode, and view. v4 §31.
- **Discoverability** — an optional Which-Key-style continuation menu ranked
  by context, frequency, recency, and project conventions. v4 §32.
- **Semantic macro recording** — Liminal records semantic command sequences
  (not keystrokes) that replay intent across structurally different
  documents; keystroke macros remain editor-owned. v4 §33.
- **Macro implementation tiers** — declarative rules, Lua, Wasm, native
  Rust, and process plugins, in increasing trust and weight. v4 §34.
- **Macro optimization** — expansion → normalized plan → specialization and
  fusion → compact executable operations; repeated macros may compile to
  Wasm or native code, one-shot operations stay interpreted unless profiling
  proves otherwise. v4 §35.

## Governing interface

- **Transform contracts** — every transformation, like every compiler pass,
  declares the eight-field contract (`requires`, `preserves`, `introduces`,
  `may_discard`, `is_deterministic`, `is_reversible`, `has_effects`,
  `required_capabilities`); destructive conversions warn unless the contract
  is explicitly accepted. v4 §14 (indexed in full in `spec/ir.md`).

Phase -1 note: no macro system, rewrite engine, or §35 optimization exists
yet — Law 14 (v4 §3) defers the whole of Part VII until the Phase -1 gates
pass (R4 §10). Phase -1 carries only the `TransformContract` vocabulary
type; the transform infrastructure phase is v4 Part XXII, Phase 3.

Status: index only — becomes a self-contained chapter at Phase 0.
