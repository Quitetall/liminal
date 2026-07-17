//! Plugin ABI and capability vocabulary (v4 §93, §98).
//!
//! RESERVED — implementation begins Phase 11 (v4 Part XXII). This crate
//! will define what plugins may contribute — dialects, syntax sugar,
//! schemas, macros, transformations, renderers, importers/exporters,
//! resolvers, indexers, AI projections, editor commands, build adapters,
//! device integrations (v4 §93) — and the capability vocabulary that
//! gates every one of those contributions: read graph / read selected
//! subgraph, write derived vs authored Nodes, add Relations, read/write
//! resources, filesystem paths, network access to allowed domains,
//! microphone, camera, process execution, credentials by named handle,
//! and database access (v4 §98).
//!
//! The two constitutional properties this crate exists to enforce:
//! capabilities are EXPLICIT (a plugin without a capability cannot perform
//! the effect), and effects are LOGGED and ATTRIBUTABLE (v4 §98). The
//! conformance suite already names the future test
//! (`plugin_capability`) that will hold this crate to that contract.
//!
//! What this crate deliberately does NOT do yet: no ABI, no traits, no
//! capability types — Phase -1 (Law 14) forbids plugin machinery entirely.
//! Related crates: `liminal-wasm-host` (sandboxed third-party runtime,
//! v4 §95), `liminal-lua` (trusted personal scripting, v4 §94).
