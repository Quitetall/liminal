//! Standard Jurisdiction profiles (v4 §7.6). Phase -1 implements exactly two;
//! immutable-resource, external-service, derived, federated, and
//! annotated-source profiles arrive in later phases (annotated-source stays
//! experimental until Phase -1.2 proves anchor recovery — v4 §7.6).

use liminal_graph::GraphStore;
use liminal_id::JurisdictionSubject;
use serde::{Deserialize, Serialize};

use crate::contract::JurisdictionContract;
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
        let _ = (subject, store);
        todo!("Phase -1 M3: external-file Contract (v4 §7.6)")
    }

    fn safety_check(&self, plan: &RepairPlan) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        let _ = plan;
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
        store: &GraphStore,
    ) -> JurisdictionContract {
        let _ = (subject, store);
        todo!("Phase -1 M3: graph-native Contract (v4 §7.6)")
    }

    fn safety_check(&self, plan: &RepairPlan) -> Result<SafetyEvidence, Vec<ReviewReason>> {
        let _ = plan;
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
}
