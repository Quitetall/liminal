//! The persistent available-input map and perspective resolution (v4 §7.5;
//! Phase -1.3 prototype territory).

use std::collections::BTreeMap;

use liminal_id::{BufferId, ClientId, JurisdictionKey, TransactionId};

use crate::basis::{BasisComponent, WorkspaceBasis};
use crate::perspective::BasisPerspective;

/// Everything the daemon currently knows about durable and working state
/// (v4 §7.5). SHAPE PROVISIONAL — Phase -1.3 reshapes this freely.
#[derive(Debug, Clone, Default)]
pub struct AvailableInputs {
    /// Durable components per subject (files, graph snapshots, objects, …).
    pub durable: BTreeMap<JurisdictionKey, BasisComponent>,
    /// Working (dirty-buffer) components per client per subject.
    pub working: BTreeMap<ClientId, BTreeMap<JurisdictionKey, BasisComponent>>,
    /// A client with several divergent buffers over one subject must select
    /// one in its working-holder map; otherwise that subject falls back to
    /// durable state and the ambiguity becomes one coalesced reconciliation
    /// item (v4 §7.5).
    pub working_selection: BTreeMap<ClientId, BTreeMap<JurisdictionKey, BufferId>>,
}

/// Resolve a perspective into one immutable Basis.
///
/// MUST encode the v4 §7.5 resolution rules:
/// - `ClientScoped`: the requester's selected buffer, durable fallback
///   elsewhere; NEVER another client's buffer.
/// - An ambiguous multi-buffer subject falls back to durable state plus one
///   coalesced reconciliation signal.
/// - A requester-less consumer fails closed to `DurableOnly` — it never
///   guesses among dirty clients.
/// - Two dirty buffers over one durable subject NEVER co-occur in one Basis
///   (Law 3J) — property-tested by `no_computation_sees_a_chimeric_basis`.
pub fn resolve(
    inputs: &AvailableInputs,
    perspective: &BasisPerspective,
    at: TransactionId,
) -> Result<WorkspaceBasis, PerspectiveError> {
    let _ = (inputs, perspective, at);
    todo!("Phase -1 M6: perspective resolution (v4 §7.5 rules)")
}

/// Perspective resolution failure.
#[derive(Debug, thiserror::Error)]
pub enum PerspectiveError {
    /// A client holds several divergent buffers for one subject and has not
    /// selected one (v4 §7.5).
    #[error("client {client} has multiple divergent buffers for {key} and no selection")]
    AmbiguousWorkingHolder {
        /// The subject with competing buffers.
        key: JurisdictionKey,
        /// The client that must select.
        client: ClientId,
    },
    /// `Federated` was requested but no declared merge runtime supplied a
    /// frontier (v4 §7.6; R4 §11.7).
    #[error("no federation frontier available")]
    NoFederationFrontier,
}
