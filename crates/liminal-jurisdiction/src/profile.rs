//! Standard Jurisdiction profiles (v4 §7.6). Phase -1 implements exactly two;
//! immutable-resource, external-service, derived, federated, and
//! annotated-source profiles arrive in later phases (annotated-source stays
//! experimental until Phase -1.2 proves anchor recovery — v4 §7.6).

use std::time::Duration;

use liminal_graph::{GraphStore, kind};
use liminal_id::{IdentityGrade, JurisdictionSubject, NodeId, PathId};
use serde::{Deserialize, Serialize};

use crate::contract::{
    ContinuityPolicy, DanglingPolicy, ForeignEditPolicy, HolderResolution, JurisdictionContract,
    LifecyclePolicy, MutationPolicy, RepairAuthorization, SafetyRequirement, SubjectSelector,
};
use crate::holder::Holder;
use crate::repair::{RepairPlan, ReviewReason, SafetyEvidence};

/// Stable profile identity (used in Overlay records and conformance reports).
/// `Cow` so built-in profiles are const-constructible while deserialized
/// records own their string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProfileId(pub std::borrow::Cow<'static, str>);

impl ProfileId {
    /// Const constructor for built-in profiles.
    #[must_use]
    pub const fn of(name: &'static str) -> Self {
        Self(std::borrow::Cow::Borrowed(name))
    }
}

/// A domain profile: supplies Contracts so ordinary work needs zero
/// Jurisdiction authoring (Law 3E), and the domain safety predicate that
/// separates deterministic candidates from safe ones (R4 §6).
pub trait JurisdictionProfile {
    /// This profile's identity.
    fn id(&self) -> ProfileId;

    /// The Contract governing `subject` under this profile.
    fn contract_for(
        &self,
        subject: JurisdictionSubject,
        store: &GraphStore,
    ) -> JurisdictionContract;

    /// The profile-specific safety predicate: a unique result is necessary but
    /// NOT sufficient for automatic acceptance (Law 3G; R4 §6).
    fn safety_check(&self, plan: &RepairPlan) -> Result<SafetyEvidence, Vec<ReviewReason>>;
}

/// External-file profile (v4 §7.6): the requesting client's buffer is the
/// working Holder, file bytes the durable Holder, a Git object optionally the
/// published Holder. Auto-acceptance requires structural disjointness at the
/// toy-paragraph level (M3/M4 implement).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExternalFileProfile;

impl JurisdictionProfile for ExternalFileProfile {
    fn id(&self) -> ProfileId {
        ProfileId::of("external-file")
    }

    fn contract_for(
        &self,
        subject: JurisdictionSubject,
        store: &GraphStore,
    ) -> JurisdictionContract {
        // Determine the owning file's path from the node's containment chain.
        // In the toy world, the file path is stored in the SYS_BLOB namespace
        // keyed by "file/<path>".
        let path = match subject {
            JurisdictionSubject::Node(node_id) => {
                find_file_path(store, node_id).unwrap_or_else(|| PathId("".into()))
            }
            JurisdictionSubject::Relation(_) => PathId("".into()),
        };
        let holder = Holder::File { path };

        JurisdictionContract {
            scope: SubjectSelector::Exact(subject),
            resolution: HolderResolution {
                candidates: vec![holder.clone()],
                read_precedence: vec![holder.clone()],
                fallback: None,
                merge_runtime: None,
            },
            mutation: MutationPolicy {
                write_route: holder,
                foreign_edits: ForeignEditPolicy::Accept,
                repair_authorization: RepairAuthorization::Automatic,
                safety: SafetyRequirement::StructuralDisjointness,
            },
            continuity: ContinuityPolicy {
                required_identity: IdentityGrade::Anchored,
                on_dangling: DanglingPolicy::PreserveAndSurface,
            },
            lifecycle: LifecyclePolicy {
                surface_after: Duration::ZERO,
                // D03.6 fixes these as spec constants in seconds; a "larger unit"
                // rewrite would obscure the spec value.
                #[allow(clippy::duration_suboptimal_units)]
                escalate_after: Duration::from_secs(3600),
                declared_transient: true,
            },
        }
    }

    fn safety_check(&self, _plan: &RepairPlan) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        todo!("Phase -1 M4: structural disjointness at toy-paragraph granularity (R4 §6)")
    }
}

/// Graph-native profile (v4 §7.6): graph transactions hold Jurisdiction; text
/// is a managed projection or deterministic serialization.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphNativeProfile;

impl JurisdictionProfile for GraphNativeProfile {
    fn id(&self) -> ProfileId {
        ProfileId::of("graph-native")
    }

    fn contract_for(
        &self,
        subject: JurisdictionSubject,
        _store: &GraphStore,
    ) -> JurisdictionContract {
        let holder = Holder::Graph;

        JurisdictionContract {
            scope: SubjectSelector::Exact(subject),
            resolution: HolderResolution {
                candidates: vec![holder.clone()],
                read_precedence: vec![holder.clone()],
                fallback: None,
                merge_runtime: None,
            },
            mutation: MutationPolicy {
                write_route: holder,
                foreign_edits: ForeignEditPolicy::Review,
                repair_authorization: RepairAuthorization::Automatic,
                safety: SafetyRequirement::StructuralDisjointness,
            },
            continuity: ContinuityPolicy {
                required_identity: IdentityGrade::Managed,
                on_dangling: DanglingPolicy::PreserveAndSurface,
            },
            lifecycle: LifecyclePolicy {
                surface_after: Duration::ZERO,
                // D03.6 fixes these as spec constants in seconds; a "larger unit"
                // rewrite would obscure the spec value.
                #[allow(clippy::duration_suboptimal_units)]
                escalate_after: Duration::from_secs(3600),
                declared_transient: false,
            },
        }
    }

    fn safety_check(&self, _plan: &RepairPlan) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        todo!("Phase -1 M4: graph-native safety predicate (R4 §6)")
    }
}

/// The registry of active profiles, keyed by [`ProfileId`]. Phase -1 holds
/// exactly the two above; selection logic (file type, schema, workspace
/// config, explicit override — v4 §7.4) arrives with M3.
#[derive(Default)]
pub struct ProfileSet {
    profiles: Vec<Box<dyn JurisdictionProfile>>,
}

impl std::fmt::Debug for ProfileSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<_> = self.profiles.iter().map(|p| p.id().0).collect();
        f.debug_struct("ProfileSet")
            .field("profiles", &ids)
            .finish()
    }
}

impl ProfileSet {
    /// The Phase -1 set: external-file + graph-native, nothing else (R4 §10).
    #[must_use]
    pub fn phase_minus_1() -> Self {
        Self {
            profiles: vec![Box::new(ExternalFileProfile), Box::new(GraphNativeProfile)],
        }
    }

    /// Look up a profile by id.
    #[must_use]
    pub fn get(&self, id: &ProfileId) -> Option<&dyn JurisdictionProfile> {
        self.profiles
            .iter()
            .find(|p| p.id() == *id)
            .map(AsRef::as_ref)
    }

    /// All registered profiles.
    pub fn iter(&self) -> impl Iterator<Item = &dyn JurisdictionProfile> {
        self.profiles.iter().map(AsRef::as_ref)
    }

    /// Dispatch by subject kind (D03.4): FILE/PARAGRAPH → external-file;
    /// COMMENT → graph-native; Relations → graph-native; other → None.
    #[must_use]
    pub fn for_subject(
        &self,
        subject: JurisdictionSubject,
        store: &GraphStore,
    ) -> Option<&dyn JurisdictionProfile> {
        match subject {
            JurisdictionSubject::Node(node_id) => {
                let node = store.node_at(store.head().ok()?, node_id).ok()??;
                match node.kind {
                    kind::FILE | kind::PARAGRAPH => self.get(&ProfileId::of("external-file")),
                    kind::COMMENT => self.get(&ProfileId::of("graph-native")),
                    _ => None,
                }
            }
            JurisdictionSubject::Relation(_) => self.get(&ProfileId::of("graph-native")),
        }
    }
}

/// Compute the identity grade of a subject (Algorithm B).
#[must_use]
pub fn grade_of(subject: JurisdictionSubject, store: &GraphStore) -> IdentityGrade {
    match subject {
        JurisdictionSubject::Node(node_id) => {
            let Ok(head) = store.head() else {
                return IdentityGrade::Ephemeral;
            };
            let Ok(Some(node)) = store.node_at(head, node_id) else {
                return IdentityGrade::Ephemeral;
            };
            // Object payload → ContentAddressed (overrides).
            if matches!(node.payload, liminal_graph::PayloadRef::Object(_)) {
                return IdentityGrade::ContentAddressed;
            }
            match node.kind {
                kind::PARAGRAPH => {
                    if node
                        .flags
                        .contains(liminal_graph::NodeFlags::HAS_DURABLE_ID)
                    {
                        IdentityGrade::Explicit
                    } else {
                        IdentityGrade::Anchored
                    }
                }
                kind::FILE => IdentityGrade::Anchored,
                kind::COMMENT => IdentityGrade::Managed,
                _ => IdentityGrade::Ephemeral,
            }
        }
        JurisdictionSubject::Relation(_) => IdentityGrade::Managed,
    }
}

/// Find the file path for a node by scanning the SYS_BLOB namespace for
/// "file/<path>" keys. Returns the first match (toy-scale linear scan).
pub(crate) fn find_file_path(store: &GraphStore, _node_id: NodeId) -> Option<PathId> {
    // In the toy world, we look for any file blob key.
    let blobs = store.scan_aux(liminal_graph::ns::SYS_BLOB).ok()?;
    for (key, _) in blobs {
        if let Some(path) = key.strip_prefix("file/") {
            return Some(PathId(path.into()));
        }
    }
    None
}
