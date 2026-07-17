//! Text subsystem — the physically privileged representation of text (v4 §46).
//!
//! RESERVED — no code lives here yet. This crate will eventually own UTF-8
//! storage with grapheme-aware navigation, fast line lookup, efficient
//! insertion/deletion, stable anchors, shared immutable slices, interval
//! marks, minimal-copy split/join, and undo summaries (v4 §46). Candidate
//! structures include ropes, piece trees, persistent B-trees, and
//! small-buffer specializations; the spec's directive is to "benchmark
//! rather than commit ideologically" (v4 §46), so no structure is chosen
//! until the §116 harness in `benches/` can measure real candidates.
//!
//! During Phase -1 the falsification laboratory (Law 14, v4 Part XXII)
//! deliberately carries text as plain `String` payloads inside
//! `liminal-graph` (`PayloadRef::Text`) — a real rope here would be
//! forbidden optimization. Implementation begins with the Phase 1 vertical
//! slice at the earliest, driven by `keystroke_to_cst_update` benchmarks.
//!
//! Individual characters must never become ordinary graph allocations
//! (v4 §46). Note: a production rope will likely require a documented
//! per-crate exception to the workspace-wide `unsafe_code = "forbid"`
//! lint; that exception must arrive with its own ADR and
//! `undocumented_unsafe_blocks`-clean justifications.
//!
//! Related crates: `liminal-graph` (payload vocabulary), `liminal-cst`
//! (L0 consumers of incremental reparse, v4 §9).
