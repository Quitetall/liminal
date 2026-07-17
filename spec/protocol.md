# Protocol — editor boundary, Liminal Document Protocol, and deployment

**Scope.** The line between what editors own and what Liminal owns, the
editor-neutral Liminal Document Protocol (LDP) with its methods and
transports, the client surfaces from Neovim to headless CI, and the
deployment modes (Unix tool pipeline, library-fused binary, persistent
daemon) those clients connect to. Corresponds to v4 Part XI (§57–§65) and
Part X §55–§56.

This file is a curated index into the canonical text in `spec/v4/`. On any
conflict the canonical text wins. Citation forms: `v4 §N`, `R4 §N`.

## Part X — Deployment surface

- **Unix tools and fused paths** — standalone composable tools
  (`lim-parse`, `lim-fmt`, `lim-check`, `lim-render`, …) for shell use, and
  a fused `lim build` that calls the same library crates directly, shares
  caches, and fuses passes (Law 11, v4 §3). v4 §55.
- **Deployment modes** — library-fused (one optimized binary), persistent
  daemon (`liminald` owns graph revisions, parsed trees, watchers, caches,
  indexes, resolver/plugin/build/sync state), and process pipeline (shell
  composition, debugging, isolation). v4 §56.

## Part XI — Editors, protocols, and interaction modes

- **Editor boundary** — editors own modes, motions, selections, buffers,
  keymaps, UI, and native macros; Liminal owns the semantic graph, parsing,
  cross-document references, formatting, graph commands, preview
  compilation, history, relation resolution, and AI projections. v4 §57.
- **Liminal Document Protocol** — the editor-neutral method surface
  (`open_document`, `apply_text_edit`, `apply_graph_transaction`,
  `get_diagnostics`, `get_semantic_tokens`, `resolve_reference`,
  `find_references`, `format_document`, `format_range`, `run_command`,
  `compile_preview`, `insert_resource`, `query_graph`, `inspect_node`,
  `subscribe_revision`) over multiple transports (embedded Rust API, Unix
  socket, MessagePack RPC, JSON-RPC debug mode, WebSocket, browser worker
  messages, mobile FFI). v4 §58.
- **Neovim** — the first-class text frontend: a thin Lua plugin connecting
  to `liminald`; Liminal never owns the editor setup. v4 §59.
- **Helix and other modal editors** — the same protocol serves Helix,
  Emacs, VS Code, and future modal editors without changing the graph or
  compiler. v4 §60.
- **Rich web editor** — a mature rich-editing transaction engine as an
  adapter mapping rich edits to graph transactions; the browser must not
  become the only runtime. v4 §61.
- **Desktop workspace mode** — separate tools (modal editor, pen canvas,
  audio timeline) coordinated through one graph and daemon. v4 §62.
- **Phone mode** — capture-first; organization must never block capture
  (Law 3B, v4 §3). v4 §63.
- **Tablet mode** — page notes, infinite canvas, ink, PDF annotation, and
  lecture capture linked to ordinary text Nodes. v4 §64.
- **Headless and CI mode** — the full compiler, checker, formatter,
  renderer, indexer, and test runner work without a GUI (Law 16, v4 §3).
  v4 §65.

Phase -1 note: there is no daemon, no socket, no LDP implementation, and no
editor integration yet — Law 14 (v4 §3) defers them (persistent daemon and
Neovim are v4 Part XXII, Phase 2; rich clients Phase 9). The Phase -1 toy
harness in `crates/liminal-daemon` is an in-process stand-in that exists
only to exercise Jurisdiction, repair, and crash recovery (R4 §10);
`crates/liminal-protocol` reserves the §58 method names as future surface.

Status: index only — becomes a self-contained chapter at Phase 0.
