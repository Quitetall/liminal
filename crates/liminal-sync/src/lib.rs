//! Live synchronization over semantic operations.
//!
//! Live sync operates over semantic operations and specialized collaborative
//! sequences: offline edits, eventual convergence, device identities,
//! encrypted transport, partial workspace sync, resource chunking, conflict
//! diagnostics, and presence as ephemeral view state (v4 §88).
//!
//! **Prior-art commitment (load-bearing): evaluate Automerge, Peritext, and
//! other existing operation models and CRDT engines before writing ANY custom
//! CRDT** (v4 §88 "existing CRDT engines should be evaluated before custom
//! implementation", §118A, §123; R4 §11.7). Rich text, block structure,
//! tables, canvas objects, and ordinary source files may require different
//! merge types; the semantic graph must not be forced to equal one vendor's
//! CRDT layout, and a domain must declare whether it is replicated,
//! file-merged, or single-writer (v4 §123). A Federated Jurisdiction profile
//! delegates to an actual merge runtime — Jurisdiction names and constrains
//! that runtime; it cannot replace it (R4 §11.7).
//!
//! **RESERVED — implementation begins Phase 6** (v4 Part XXII "history,
//! sync"). Law 14 and the Phase -1 what-NOT-to-build list forbid any CRDT or
//! OT implementation until the falsification gates pass; in Phase -1 the
//! `MergeRuntimeRef` in `liminal-jurisdiction` is named but never implemented.
//!
//! This crate exists now so the constitutional vocabulary has a compiled,
//! greppable home: the [`Replica`] seam lets the §112 convergence law compile
//! as a generic conformance test today.

/// The convergence seam over one sync replica (v4 §88, §112).
///
/// SHAPE PROVISIONAL — exists only so the §112 law compiles as a generic
/// conformance test (and the §113 sync-simulation/partition class can be
/// named) now; Phase 6 may reshape it freely, most likely into a thin
/// adapter over an evaluated engine such as Automerge (v4 §118A, §123).
///
/// Law (v4 §112): sync replicas converge under supported operation
/// schedules — after any supported interleaving of `apply` and pairwise
/// `merge` delivering the same operation set, all replicas report equal
/// `state`. "Supported" is deliberate: each domain declares its merge model,
/// and unsupported schedules surface as conflict diagnostics, never silent
/// divergence (v4 §88, §123).
pub trait Replica {
    /// One semantic operation (v4 §86, §88).
    type Op;
    /// Observable replica state; `PartialEq` so convergence is checkable.
    type State: PartialEq;

    /// Apply a local or delivered remote operation.
    fn apply(&mut self, op: Self::Op);

    /// Merge another replica's knowledge into this one.
    fn merge(&mut self, other: &Self);

    /// The observable state used to check convergence.
    fn state(&self) -> Self::State;
}
