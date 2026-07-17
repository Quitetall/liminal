//! Basis Perspectives (v4 §7.5; R4 §8) — the anti-chimera selector.

use liminal_id::{ClientId, FederationId, PublicationId};
use serde::{Deserialize, Serialize};

use crate::basis::CausalFrontier;

/// Which working truth a computation is allowed to see (Law 3J; R4 §8).
///
/// Resolution rules (v4 §7.5):
/// - An interactive preview, query, or AI request defaults to the requesting
///   client's [`ClientScoped`](Self::ClientScoped) Basis.
/// - A background build, reproducible export, CI job, or publication defaults
///   to [`DurableOnly`](Self::DurableOnly) or an explicit
///   [`Published`](Self::Published) Basis.
/// - A shared collaborative surface may use [`Federated`](Self::Federated) only
///   when a declared merge runtime supplies one frontier.
/// - Two unrelated dirty buffers over the same durable subject are branch-like
///   working claims; they are never combined into one Basis.
/// - A consumer without a requester and without an explicit mode must fail
///   closed to `DurableOnly`, not guess among dirty clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BasisPerspective {
    /// Use the requesting client's selected dirty buffers where available,
    /// durable fallbacks elsewhere. Never another client's dirty buffer.
    ClientScoped {
        /// The requesting client.
        client: ClientId,
    },
    /// Ignore all provisional dirty buffers.
    DurableOnly,
    /// One immutable outward-facing revision.
    Published {
        /// The publication revision.
        revision: PublicationId,
    },
    /// One accepted causal frontier supplied by a declared merge runtime
    /// (R4 §11.7 — Jurisdiction names the runtime, never replaces it).
    Federated {
        /// The federated merge domain.
        domain: FederationId,
        /// The accepted frontier.
        frontier: CausalFrontier,
    },
}
