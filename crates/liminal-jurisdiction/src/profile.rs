//! Standard Jurisdiction profiles (v4 §7.6). Phase -1 implements exactly two;
//! immutable-resource, external-service, derived, federated, and
//! annotated-source profiles arrive in later phases (annotated-source stays
//! experimental until Phase -1.2 proves anchor recovery — v4 §7.6).

use std::time::Duration;

use liminal_graph::{GraphStore, kind};
use liminal_id::{IdentityGrade, JurisdictionSubject, NodeId, PathId};
use liminal_source::merge::{self, MergeOutcome};
use serde::{Deserialize, Serialize};

use crate::blob;
use crate::contract::{
    ContinuityPolicy, DanglingPolicy, ForeignEditPolicy, HolderResolution, JurisdictionContract,
    LifecyclePolicy, MutationPolicy, RepairAuthorization, SafetyRequirement, SubjectSelector,
};
use crate::holder::Holder;
use crate::repair::{RepairOperation, RepairPlan, ReviewReason, SafetyEvidence, StatePredicate};

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
    /// NOT sufficient for automatic acceptance (Law 3G; R4 §6). Takes the store
    /// (AM-4.1) so file predicates can recompute disjointness from content-
    /// addressed `SYS_BLOB` blobs (DG-4.1 reading (a): no disk read — the plan's
    /// prestate hash is the authoritative current durable state).
    fn safety_check(
        &self,
        plan: &RepairPlan,
        store: &GraphStore,
    ) -> Result<SafetyEvidence, Vec<ReviewReason>>;
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

    fn safety_check(
        &self,
        plan: &RepairPlan,
        store: &GraphStore,
    ) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        // M04 Algorithm B (ExternalFileProfile). Structural disjointness at
        // toy-paragraph granularity (R4 §6): a UNIQUE merge is necessary but
        // NOT sufficient — the merge must not touch any block the durable file
        // already changed. Overlap is computed from (base, current, merged)
        // via `merge::overlap_slots` (DG-4.1 reading (a): the step prestate
        // hash IS the current durable bytes; no disk read).
        let mut descriptions: Vec<String> = Vec::new();
        let mut reasons: Vec<ReviewReason> = Vec::new();

        for step in plan.steps.values() {
            match &step.operation {
                RepairOperation::WriteFile { path, contents } => {
                    // base = the buffer's base blob = the plan basis component
                    // for this path; current = the step prestate hash's blob;
                    // merged = the WriteFile contents.
                    let current_hash = match &step.expected_prestate {
                        StatePredicate::FileContent { hash, .. } => Some(*hash),
                        StatePredicate::FileAbsent { .. } => None,
                        _ => {
                            reasons.push(ReviewReason(format!(
                                "unexpected prestate for file step on {path}"
                            )));
                            continue;
                        }
                    };
                    let base_hash = basis_file_hash(plan, path);

                    let base = base_hash.and_then(|h| blob::get(store, h).ok().flatten());
                    let current = current_hash.and_then(|h| blob::get(store, h).ok().flatten());
                    let merged = String::from_utf8_lossy(contents).into_owned();

                    match (base, current) {
                        (Some(base), Some(current)) => {
                            let over = merge::overlap_slots(&base, &current, &merged);
                            if over.is_empty() {
                                // Confirm a unique three-way result exists too.
                                match merge::three_way(&base, &current, &merged) {
                                    MergeOutcome::Conflict => reasons.push(ReviewReason(
                                        "no unique result at the captured basis".into(),
                                    )),
                                    _ => descriptions
                                        .push(format!("{path}: structurally disjoint save")),
                                }
                            } else {
                                reasons.push(ReviewReason(format!(
                                    "not structurally disjoint: both edits touch {over:?}"
                                )));
                            }
                        }
                        // First-write or missing base blob: nothing to overlap
                        // against — a fresh file save is structurally disjoint.
                        _ => descriptions.push(format!("{path}: new-file save")),
                    }
                }
                RepairOperation::InsertSourceId { path, .. } => {
                    // Marker-only insertion is content-preserving.
                    descriptions.push(format!("{path}: id-insert content-preserving"));
                }
                // Graph/External steps are not governed by this file profile.
                _ => {}
            }
        }

        if reasons.is_empty() {
            Ok(SafetyEvidence::StructurallyDisjoint {
                description: descriptions.join("; "),
            })
        } else {
            Err(reasons)
        }
    }
}

/// The base file hash a plan captured (its `WorkspaceBasis` `ObjectContent`
/// component — the buffer's base blob, distinct from the current `Path`
/// component), if any. `_path` is unused in the single-file toy but kept for
/// the multi-file M6 signature.
fn basis_file_hash(plan: &RepairPlan, _path: &PathId) -> Option<liminal_id::ContentHash> {
    use liminal_revision::BasisComponent;
    plan.basis.components.values().find_map(|c| match c {
        BasisComponent::ObjectContent { hash } => Some(*hash),
        _ => None,
    })
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

    fn safety_check(
        &self,
        plan: &RepairPlan,
        store: &GraphStore,
    ) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        // M04 Algorithm B §3b (GraphNativeProfile): every RetargetRelation step
        // must leave the relation's identity requirement satisfied. The new
        // target's effective grade is raised to Explicit when a sibling
        // InsertSourceId step in the same plan serializes that target's id
        // (the reattach chains after the marker insertion); otherwise it is the
        // durable grade_of the target — heuristic/Inferred when no marker exists.
        let mut reasons: Vec<ReviewReason> = Vec::new();

        // Targets that a same-plan InsertSourceId step promotes to Explicit.
        let insert_targets: std::collections::BTreeSet<NodeId> = plan
            .steps
            .values()
            .filter_map(|s| match &s.operation {
                RepairOperation::InsertSourceId { .. } => match s.subject {
                    JurisdictionSubject::Node(n) => Some(n),
                    JurisdictionSubject::Relation(_) => None,
                },
                _ => None,
            })
            .collect();

        for step in plan.steps.values() {
            let RepairOperation::Graph(liminal_graph::Operation::RetargetRelation { id, target }) =
                &step.operation
            else {
                continue;
            };
            let Ok(Some(relation)) = store.relation_at(store.head().unwrap_or_default(), *id)
            else {
                continue;
            };
            let Some(req) = relation.requires else {
                continue;
            };
            let target_node = target.node();
            let effective = if insert_targets.contains(&target_node) {
                IdentityGrade::Explicit
            } else {
                grade_of(JurisdictionSubject::Node(target_node), store)
            };
            if !effective.satisfies(req.minimum) {
                reasons.push(ReviewReason(format!(
                    "reattachment relies on a heuristic match ({effective:?}) but the \
                     relation requires {:?}",
                    req.minimum
                )));
            }
        }

        if reasons.is_empty() {
            Ok(SafetyEvidence::DomainValidator {
                validator: "graph-native/identity-requirement".into(),
                report: "targets satisfy declared identity requirements at basis".into(),
            })
        } else {
            Err(reasons)
        }
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
