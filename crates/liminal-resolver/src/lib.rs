//! Effectful external resolution — deliberately OUTSIDE the query engine.
//!
//! A Relation never performs network work merely because it is inspected
//! (v4 §39). Resolution follows the effect-isolation pipeline: declarative
//! relation → resolver request plan → capability-mediated effect →
//! transaction → materialized graph update (v4 §39). A resolver observes the
//! outside world and submits a new Jurisdiction transaction; it must not run
//! invisibly inside a tracked query — that is what makes invalidation,
//! replay, testing, and reproducible freezing tractable (v4 §8.6, §24).
//!
//! Resolution exposes distinct dimensions — [`Availability`] and
//! [`Freshness`] — because identity, local possession, and currentness must
//! never be conflated (v4 §40).
//!
//! **RESERVED — implementation begins Phase 7** (v4 Part XXII "Intelligent
//! Relations, resolvers, effect isolation, freeze"). Law 14 forbids real
//! resolvers, capability runtimes, and `lim freeze` until the Phase -1 gates
//! pass.
//!
//! This crate exists now so the constitutional vocabulary — the §40 enums,
//! verbatim — has a compiled, greppable home, and so the §112 law
//! ("resolver replay with frozen inputs is deterministic") compiles as a
//! generic conformance test over the [`ReplayableResolver`] seam today.

use liminal_id::Timestamp;
use serde::{Deserialize, Serialize};

/// How much of an external value is locally possessed (verbatim v4 §40).
///
/// Local possession only — says nothing about identity or currentness
/// (v4 §40: the three must never be conflated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    /// Nothing is locally materialized (v4 §40).
    Absent,
    /// Only metadata about the value is held locally (v4 §40).
    MetadataOnly,
    /// Some of the value is held locally (v4 §40).
    Partial,
    /// The full value is locally materialized (v4 §40).
    Materialized,
}

/// How current a locally held value is believed to be (verbatim v4 §40).
///
/// Currentness only — says nothing about identity or possession (v4 §40:
/// the three must never be conflated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Freshness {
    /// Believed current with the external source (v4 §40).
    Current,
    /// Known stale since a recorded time (v4 §40).
    Stale {
        /// When the value was last known current.
        since: Timestamp,
    },
    /// Deliberately frozen — e.g. by `lim freeze` for publication, archival,
    /// or deterministic builds (v4 §40, §42).
    Frozen,
    /// Currentness cannot be determined (v4 §40).
    Unknown,
}

/// The replay seam for effectful resolvers (v4 §8.6, §112).
///
/// SHAPE PROVISIONAL — exists only so the §112 law compiles as a generic
/// conformance test now; Phase 7 may reshape it freely (real resolvers add
/// request plans, capabilities, and transaction submission, v4 §39).
///
/// Law (v4 §112): resolver replay with frozen inputs is deterministic —
/// `replay` over previously frozen observations must reproduce the recorded
/// observation exactly, byte-identically, with no new effects. `observe` is
/// the only effectful entry point, and it runs outside tracked queries
/// (v4 §8.6). Frozen graphs record source identity, retrieval time, content
/// hash, resolver version, and provenance (v4 §42).
pub trait ReplayableResolver {
    /// A resolution request.
    type Request;
    /// One recorded observation of the outside world; `PartialEq` so the
    /// determinism law is checkable.
    type Observation: PartialEq;

    /// Perform the effect: observe the outside world for `request`.
    fn observe(&mut self, request: &Self::Request) -> Self::Observation;

    /// Re-answer `request` purely from `frozen` observations — deterministic,
    /// effect-free (v4 §112).
    fn replay(&self, request: &Self::Request, frozen: &[Self::Observation]) -> Self::Observation;
}
