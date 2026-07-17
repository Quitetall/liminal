//! Lua scripting layer for trusted personal customization (v4 §94).
//!
//! RESERVED — implementation begins Phase 3 (v4 Part XXII). Lua is the
//! trusted, low-ceremony scripting surface: personal customization, macro
//! definitions, quick graph transforms, formatting preferences,
//! editor-facing commands, and configuration logic (v4 §94). It sits
//! opposite `liminal-wasm-host` (v4 §95), which is the sandboxed runtime
//! for UNtrusted third-party code — the trust boundary between the two is
//! deliberate, not incidental.
//!
//! The one constitutional rule this crate will be held to: Lua APIs must
//! expose STABLE SEMANTIC OPERATIONS — graph transactions, queries,
//! commands — never internal Rust pointers or memory layout (v4 §94).
//! Scripts survive engine rewrites because they only ever spoke the
//! semantic vocabulary.
//!
//! What this crate deliberately does NOT do yet: no interpreter
//! dependency, no bindings, no API surface — Phase -1 (Law 14, v4 Part
//! XXII) forbids scripting/plugin machinery, and the semantic operations
//! a script would call are exactly what the falsification lab is still
//! reshaping. Related crates: `liminal-plugin-api` (capability and effect
//! vocabulary, v4 §98), `liminal-transform` (the rewrite calculus scripts
//! will drive, v4 §28).
