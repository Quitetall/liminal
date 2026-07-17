//! Graph rewrite calculus and declared pass contracts.
//!
//! The transformation runtime is the Turing-complete operational core of
//! editing: it matches Nodes and Relations, binds values, creates and deletes
//! Nodes, adds and removes Relations, replaces payloads, moves ordered
//! children, branches, iterates, recurses, composes, and requests effects
//! through capabilities (v4 §28). It is imperative infrastructure, not a third
//! semantic primitive — programs and their results are represented as graph
//! structures (v4 §28; Law 1: the kernel stays Node + Relation).
//!
//! Every pass declares a [`TransformContract`] up front, and the compiler must
//! warn before a destructive conversion unless the user explicitly accepts the
//! contract (v4 §14; Law 8: conversion honesty).
//!
//! **RESERVED — implementation begins Phase 3** (v4 Part XXII "transformation
//! and macro layers"). Law 14 forbids transforms, macros, and schemas until
//! the Phase -1 falsification gates pass.
//!
//! This crate exists now so the constitutional vocabulary — the §14 contract
//! that every future pass must declare — has a compiled, greppable home that
//! conversion-loss conformance fixtures (v4 §114) can reference today.

use serde::{Deserialize, Serialize};

/// The contract every pass declares before it runs (v4 §14, field-for-field).
///
/// SHAPE PROVISIONAL — the eight fields transcribe the §14 list verbatim, but
/// their element type is a plain `String` until Phase 3 decides the real
/// property vocabulary (typed property atoms, schema references, or capability
/// identifiers). Phase -1/0 may reshape this freely.
///
/// Law 8 (conversion honesty, v4 §14): the compiler must warn before a
/// destructive conversion — one whose `may_discard` is non-empty for content
/// present in the input — unless the user explicitly accepts the contract.
/// Conformance class "differential conversion tests" (v4 §113, §114) checks
/// declared loss against observed loss.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformContract {
    /// Properties the input must already satisfy (v4 §14 `requires`).
    pub requires: Vec<String>,
    /// Properties guaranteed to survive the pass (v4 §14 `preserves`).
    pub preserves: Vec<String>,
    /// Properties the pass adds (v4 §14 `introduces`).
    pub introduces: Vec<String>,
    /// Properties the pass is allowed to lose — the honesty field a
    /// destructive-conversion warning is computed from (v4 §14 `may_discard`).
    pub may_discard: Vec<String>,
    /// Whether the pass is a deterministic function of its declared inputs
    /// (v4 §14 `is_deterministic`). Determinism is not safety (R4 §6).
    pub is_deterministic: bool,
    /// Whether the pass records enough to be inverted (v4 §14 `is_reversible`).
    pub is_reversible: bool,
    /// Whether the pass requests effects; effects run through capabilities,
    /// outside pure queries (v4 §14 `has_effects`, §8.6, §28).
    pub has_effects: bool,
    /// Capabilities the pass must hold for its effects (v4 §14
    /// `required_capabilities`, §98).
    pub required_capabilities: Vec<String>,
}
