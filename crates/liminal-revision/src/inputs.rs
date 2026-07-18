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
    // M06 Algorithm A — every v4 §7.5 resolution rule.
    let components = match perspective {
        // DurableOnly ignores ALL working claims (fail-closed default for
        // requester-less consumers — exports, background jobs).
        BasisPerspective::DurableOnly => inputs.durable.clone(),

        // ClientScoped reads ONLY the requesting client's working map, durable
        // elsewhere — never another client's buffer (Law 3J by construction,
        // D06.1). The published `working` map holds at most one component per
        // key (D06.2 handles divergence at publish time), so the common path
        // simply overlays it onto the durable base.
        BasisPerspective::ClientScoped { client } => {
            let mut base = inputs.durable.clone();
            if let Some(working) = inputs.working.get(client) {
                let selection = inputs.working_selection.get(client);
                for (key, comp) in working {
                    match selection.and_then(|m| m.get(key)) {
                        // No selection: the publisher guarantees uniqueness.
                        None => {
                            base.insert(key.clone(), comp.clone());
                        }
                        // A selection that names THIS buffer: publish it.
                        Some(sel) if is_selected_buffer(comp, *sel) => {
                            base.insert(key.clone(), comp.clone());
                        }
                        // A selection naming a different buffer than the one
                        // published: an inconsistent state, surfaced loudly.
                        Some(_) => {
                            return Err(PerspectiveError::AmbiguousWorkingHolder {
                                key: key.clone(),
                                client: *client,
                            });
                        }
                    }
                }
            }
            base
        }

        // No publication machinery exists in Phase -1 — fail closed with a
        // distinct reason rather than lie (AM-6.1).
        BasisPerspective::Published { .. } => {
            return Err(PerspectiveError::PublishedUnavailable);
        }
        BasisPerspective::Federated { .. } => {
            return Err(PerspectiveError::NoFederationFrontier);
        }
    };

    Ok(WorkspaceBasis {
        transaction: at,
        perspective: perspective.clone(),
        components,
    })
}

/// Whether a working component is the `BufferGeneration` for the selected
/// buffer id.
fn is_selected_buffer(comp: &BasisComponent, selected: BufferId) -> bool {
    matches!(
        comp,
        BasisComponent::BufferGeneration { buffer, .. } if *buffer == selected
    )
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
    /// `Published` was requested but no publication machinery exists in
    /// Phase -1 — fail closed with a distinct reason (AM-6.1).
    #[error("no publication revision available")]
    PublishedUnavailable,
    /// `Federated` was requested but no declared merge runtime supplied a
    /// frontier (v4 §7.6; R4 §11.7).
    #[error("no federation frontier available")]
    NoFederationFrontier,
}

#[cfg(test)]
mod tests {
    use liminal_id::{ContentHash, PathId, SessionEpoch};

    use super::*;

    fn path_key(p: &str) -> JurisdictionKey {
        JurisdictionKey::Path(PathId(p.into()))
    }

    fn file_component(p: &str, bytes: &[u8]) -> BasisComponent {
        BasisComponent::FileContent {
            path: PathId(p.into()),
            hash: ContentHash::of(bytes),
        }
    }

    fn buffer_component(client: ClientId, buffer: BufferId, generation: u64) -> BasisComponent {
        BasisComponent::BufferGeneration {
            client,
            buffer,
            epoch: SessionEpoch(1),
            generation,
            content_hash: Some(ContentHash::of(format!("gen{generation}").as_bytes())),
            base_file_hash: Some(ContentHash::of(b"base")),
        }
    }

    /// Two clients each with a dirty buffer over the same path. Build the input
    /// map the daemon would publish (one component per client per key).
    fn two_client_inputs() -> (AvailableInputs, ClientId, ClientId, BufferId, BufferId) {
        let (neovim, phone) = (ClientId::new(), ClientId::new());
        let (nbuf, pbuf) = (BufferId::new(), BufferId::new());
        let key = path_key("notes.md");
        let mut inputs = AvailableInputs::default();
        inputs
            .durable
            .insert(key.clone(), file_component("notes.md", b"v0"));
        inputs
            .working
            .entry(neovim)
            .or_default()
            .insert(key.clone(), buffer_component(neovim, nbuf, 1));
        inputs
            .working
            .entry(phone)
            .or_default()
            .insert(key.clone(), buffer_component(phone, pbuf, 2));
        (inputs, neovim, phone, nbuf, pbuf)
    }

    #[test]
    fn durable_only_ignores_all_working_claims() {
        let (inputs, ..) = two_client_inputs();
        let basis = resolve(
            &inputs,
            &BasisPerspective::DurableOnly,
            TransactionId::new(),
        )
        .unwrap();
        // Zero buffer components: only the durable file survives.
        for comp in basis.components.values() {
            assert!(
                !matches!(comp, BasisComponent::BufferGeneration { .. }),
                "DurableOnly must not carry a buffer component"
            );
        }
        assert_eq!(basis.components.len(), 1);
    }

    #[test]
    fn client_scoped_sees_only_its_own_buffer() {
        let (inputs, neovim, phone, nbuf, _pbuf) = two_client_inputs();
        let basis = resolve(
            &inputs,
            &BasisPerspective::ClientScoped { client: neovim },
            TransactionId::new(),
        )
        .unwrap();
        let comp = &basis.components[&path_key("notes.md")];
        // It is neovim's buffer, never phone's (Law 3J).
        match comp {
            BasisComponent::BufferGeneration { client, buffer, .. } => {
                assert_eq!(*client, neovim);
                assert_eq!(*buffer, nbuf);
            }
            other => panic!("expected neovim's BufferGeneration, got {other:?}"),
        }
        let _ = phone;
    }

    #[test]
    fn no_chimera_every_component_belongs_to_the_requester() {
        // The core Law 3J property: under ClientScoped(c), no component may
        // carry another client's identity.
        let (inputs, neovim, phone, ..) = two_client_inputs();
        for client in [neovim, phone] {
            let basis = resolve(
                &inputs,
                &BasisPerspective::ClientScoped { client },
                TransactionId::new(),
            )
            .unwrap();
            for comp in basis.components.values() {
                if let BasisComponent::BufferGeneration { client: c, .. } = comp {
                    assert_eq!(*c, client, "chimeric basis: foreign buffer leaked in");
                }
            }
        }
    }

    #[test]
    fn selection_naming_a_different_buffer_is_ambiguous() {
        let (mut inputs, neovim, _phone, _nbuf, _pbuf) = two_client_inputs();
        // Select a buffer id that is NOT the published one → inconsistent.
        inputs
            .working_selection
            .entry(neovim)
            .or_default()
            .insert(path_key("notes.md"), BufferId::new());
        let err = resolve(
            &inputs,
            &BasisPerspective::ClientScoped { client: neovim },
            TransactionId::new(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            PerspectiveError::AmbiguousWorkingHolder { .. }
        ));
    }

    #[test]
    fn matching_selection_publishes_the_named_buffer() {
        let (mut inputs, neovim, _phone, nbuf, _pbuf) = two_client_inputs();
        inputs
            .working_selection
            .entry(neovim)
            .or_default()
            .insert(path_key("notes.md"), nbuf);
        let basis = resolve(
            &inputs,
            &BasisPerspective::ClientScoped { client: neovim },
            TransactionId::new(),
        )
        .unwrap();
        assert!(matches!(
            &basis.components[&path_key("notes.md")],
            BasisComponent::BufferGeneration { buffer, .. } if *buffer == nbuf
        ));
    }

    #[test]
    fn published_and_federated_fail_closed_distinctly() {
        use liminal_id::{FederationId, PublicationId};
        let inputs = AvailableInputs::default();
        let pub_err = resolve(
            &inputs,
            &BasisPerspective::Published {
                revision: PublicationId::new(),
            },
            TransactionId::new(),
        )
        .unwrap_err();
        assert!(matches!(pub_err, PerspectiveError::PublishedUnavailable));

        let fed_err = resolve(
            &inputs,
            &BasisPerspective::Federated {
                domain: FederationId::new(),
                frontier: crate::CausalFrontier(vec![]),
            },
            TransactionId::new(),
        )
        .unwrap_err();
        assert!(matches!(fed_err, PerspectiveError::NoFederationFrontier));
    }
}
