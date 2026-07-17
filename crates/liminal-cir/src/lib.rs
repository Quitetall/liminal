//! L2 — Liminal Resolved Graph IR: the normalized Node-and-Relation graph
//! at an explicit Workspace Basis (v4 §11).
//!
//! RESERVED — implementation begins Phase 1 (v4 Part XXII). L2 is Liminal's
//! semantic normal form, but normal form does not imply ownership: in
//! external-file Jurisdiction domains it is a DERIVED semantic
//! materialization, while in graph-native domains the graph itself holds
//! Jurisdiction and L2 is GOVERNING (v4 §11, §7.6). That asymmetry is the
//! whole point of the Jurisdiction complexity sink (v4 §7).
//!
//! When built, this layer will carry expanded macros, canonical symbols,
//! materialized semantic defaults, validated domain constraints, resolved
//! local references, namespaced unknown extensions, explicit resource
//! identities, provenance, relation policies, and effect requests. It is
//! the preferred layer for semantic diffing, search/indexing, sync,
//! history snapshots, conversion, AI lowering, and static analysis (v4 §11).
//!
//! What this crate deliberately does NOT do yet: Phase -1 (Law 14) has no
//! IR stack — the toy format maps straight to `liminal-graph` operations,
//! and every Basis question is exercised through `liminal-revision`
//! (v4 §7.5) instead. Related crates: `liminal-hir` (L1 above, v4 §10),
//! `liminal-graph` (kernel vocabulary), `liminal-revision` (the explicit
//! Basis every L2 view is pinned to).
