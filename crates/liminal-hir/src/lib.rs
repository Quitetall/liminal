//! L1 — Liminal Human IR: the layer that preserves authorial intent and
//! domain sugar (v4 §10).
//!
//! RESERVED — implementation begins Phase 1 (v4 Part XXII). The Human IR
//! will contain semantic Node identities at their declared identity grades
//! (v4 §19.1), source anchors, high-level kinds, unexpanded macros kept for
//! diagnostics, symbolic defaults, unresolved references, authorship
//! provenance, and domain-specific structures. It is the main plugin- and
//! editor-facing semantic layer (v4 §10).
//!
//! What this crate deliberately does NOT do yet: Phase -1 is a
//! falsification laboratory (Law 14, v4 Part XXII) whose toy paragraph
//! format is hand-parsed directly into graph operations — there is no IR
//! stack to lower through, no parser, and no macro machinery. Building L1
//! before the Jurisdiction/ILRP/Basis killer experiments pass their gates
//! would be premature commitment.
//!
//! Position in the pipeline (v4 §21): sits above `liminal-cst` (L0 bytes
//! and Concrete Syntax Tree, v4 §9) and lowers into `liminal-cir` (L2
//! Resolved Graph IR, v4 §11) at an explicit `WorkspaceBasis` from
//! `liminal-revision` (v4 §7.5).
