//! `liminal-benches` — the v4 §116 performance benchmark charter, held to Law 14.
//!
//! This library target intentionally exports **nothing**. It exists to carry the
//! benchmark policy; the harness itself lives in `benches/liminal.rs`.
//!
//! # Charter (v4 §116)
//!
//! The spec names thirteen benchmarks: cold startup, warm daemon startup,
//! keystroke-to-CST update, keystroke-to-HTML patch, large-file scrolling and
//! lazy loading, Relation traversal, graph query latency, table recalculation,
//! sync merge, resource loading, AI context compilation, macro expansion, and
//! memory per logical Node and Relation. All thirteen are named in the harness
//! today so the charter is code, not prose. "Optimization decisions must be
//! benchmark-driven" (v4 §116) — which presupposes baselines exist before any
//! optimization is proposed.
//!
//! # Law 14 policy — baselines recorded, nothing gated
//!
//! Law 14 (v4 §3): risk-retirement order precedes dependency order. The project
//! is in the Phase -1 falsification laboratory (v4 Part XXII); optimization is
//! forbidden until the Phase -1 gates pass. Accordingly:
//!
//! - Benchmarks whose subject exists (the §92 toy store in `liminal-graph`)
//!   run for real and **record baselines only**. Numbers are informative; they
//!   justify no optimization work and fail no build.
//! - There is **no CI regression gating**. Activation trigger: when the Phase 1
//!   source-to-HTML vertical slice lands (v4 Part XXII), wire continuous
//!   benchmarking via CodSpeed using `codspeed-divan-compat`, and only then may
//!   a regression fail CI.
//! - Benchmarks whose subject subsystem is Phase-gated (parser, daemon, sync,
//!   resource stack, AI compiler, macros) are compiled only behind the
//!   default-off `unimplemented` cargo feature. Running them **panics by
//!   design** via `unimplemented!`; each documents its target metric and the
//!   phase that activates it. A future phase is "done" with a benchmark when
//!   its bench moves out of the feature gate and produces a real baseline.
//!
//! `just bench` runs the implemented set. `cargo check -p liminal-benches
//! --features unimplemented --benches` proves the full charter still compiles.
