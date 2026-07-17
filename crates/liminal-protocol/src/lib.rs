//! Liminal Document Protocol (LDP) — the editor-neutral protocol boundary
//! (v4 §57–58).
//!
//! RESERVED — implementation begins Phase 2 (v4 Part XXII), when the real
//! daemon lands. The future method surface, verbatim from v4 §58:
//!
//! - `open_document`, `apply_text_edit`, `apply_graph_transaction`
//! - `get_diagnostics`, `get_semantic_tokens`
//! - `resolve_reference`, `find_references`
//! - `format_document`, `format_range`
//! - `run_command`, `compile_preview`, `insert_resource`
//! - `query_graph`, `inspect_node`, `subscribe_revision`
//!
//! Planned transports (v4 §58): embedded Rust API, Unix socket,
//! MessagePack RPC, JSON-RPC debug mode, WebSocket, browser worker
//! messages, and mobile FFI.
//!
//! What this crate deliberately does NOT do yet: no wire types, no
//! transport, no server — Phase -1 (Law 14, v4 Part XXII) explicitly
//! forbids real editors, persistent daemons, and LDP; the toy harness in
//! `liminal-daemon` is process-per-scenario with no sockets. Every method
//! above ultimately resolves against an explicit `WorkspaceBasis`
//! (`liminal-revision`, v4 §7.5) — `subscribe_revision` is the seam where
//! Basis Perspectives (R4 §8) reach editors. Related crates:
//! `liminal-daemon` (future host), `liminal-cli` (everyday-vocabulary
//! surface, R4 §3).
