//! WebAssembly component plugin host — the default third-party plugin
//! runtime (v4 §95).
//!
//! RESERVED — implementation begins Phase 11 (v4 Part XXII). The spec's
//! requirements for this runtime: sandboxed, portable, browser
//! compatible, capability controlled, language independent, and versioned
//! through a component interface — using the Wasm component model and
//! explicit interfaces where practical (v4 §95). Every capability a
//! component holds comes from the vocabulary in `liminal-plugin-api`
//! (v4 §98); nothing is ambient.
//!
//! What this crate deliberately does NOT do yet: no runtime dependency,
//! no host functions, no component loading — Phase -1 (Law 14) forbids
//! plugin machinery, and choosing a Wasm engine now would be premature
//! commitment. Note: embedding a production Wasm engine will almost
//! certainly require a documented per-crate exception to the
//! workspace-wide `unsafe_code = "forbid"` lint; that exception must
//! arrive with its own ADR when this crate becomes real.
//!
//! Related crates: `liminal-plugin-api` (capability vocabulary and effect
//! attribution), `liminal-lua` (the trusted, non-sandboxed counterpart
//! for personal customization, v4 §94).
