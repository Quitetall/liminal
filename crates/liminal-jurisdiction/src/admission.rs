//! Checked repair admission (AM-17.12). DTOs describe proposals, not authority.
//! The existing interpretive checker supplies the automatic safety decision.

use liminal_graph::{GraphStore, Operation, ns};
use liminal_id::{
    ActorId, ContentHash, GraphRevisionId, JurisdictionKey, JurisdictionSubject, Timestamp,
};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};

use crate::{
    Checker, CheckerError, Holder, RepairDecision, RepairOperation, RepairPlan, ReviewReason,
    SafetyEvidence, StatePredicate,
};

/// Validated selected inputs bound to a live coordinating store and revision (v4 §7.5).
/// The legacy DTO `transaction` label is retained, not used as proof of history.
/// Actual admission revision is recorded independently here.
#[derive(Debug)]
pub struct ValidatedBasis<'s> {
    store: &'s GraphStore,
    revision: GraphRevisionId,
    basis: WorkspaceBasis,
}

impl ValidatedBasis<'_> {
    /// The checked input vector, not a claim about resulting state (v4 §7.5).
    #[must_use]
    pub fn basis(&self) -> &WorkspaceBasis {
        &self.basis
    }
    /// Actual store revision observed during admission (v4 §7.5).
    #[must_use]
    pub fn revision(&self) -> GraphRevisionId {
        self.revision
    }
}

/// Non-serializable authority for one immutable repair in one store (v4 Law 3F; v4 §7.7).
///
/// ```compile_fail
/// use liminal_jurisdiction::AuthorizedRepair;
/// fn forge(value: serde_json::Value) {
///     let _: AuthorizedRepair<'_> = serde_json::from_value(value).unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct AuthorizedRepair<'s> {
    basis: ValidatedBasis<'s>,
    plan: RepairPlan,
    evidence: SafetyEvidence,
}

impl AuthorizedRepair<'_> {
    /// Read the admitted proposal; cloning this DTO does not clone authority (v4 §7.7).
    #[must_use]
    pub fn plan(&self) -> &RepairPlan {
        &self.plan
    }
    /// Evidence produced by the checker or explicit local human acceptance (v4 §7.9; R4 §6).
    #[must_use]
    pub fn evidence(&self) -> &SafetyEvidence {
        &self.evidence
    }
    /// The captured and runtime-validated input vector (v4 §7.5).
    #[must_use]
    pub fn basis(&self) -> &ValidatedBasis<'_> {
        &self.basis
    }

    pub(crate) fn consume(
        self,
        store: &GraphStore,
    ) -> Result<(RepairPlan, SafetyEvidence), AdmissionError> {
        if !std::ptr::eq(self.basis.store, store) {
            return Err(refusal("repair authority belongs to another store"));
        }
        if store.head()? != self.basis.revision {
            return Err(refusal(
                "store changed after repair admission; evaluate again",
            ));
        }
        validate_basis(store, &self.basis.basis, self.basis.revision)?;
        Ok((self.plan, self.evidence))
    }
}

/// Refused admission preserves the proposal; it authorizes no effect (v4 Law 3F; v4 §7.7).
#[derive(Debug, thiserror::Error)]
pub enum AdmissionError {
    /// The existing checker could not establish automatic acceptance (v4 Law 3G; v4 §7.9).
    #[error("repair requires review: {0:?}")]
    Review(Vec<ReviewReason>),
    /// Reading the coordinating store failed (v4 §7.8).
    #[error(transparent)]
    Store(#[from] liminal_graph::StoreError),
    /// A governing profile or checker operation failed (v4 §7.3).
    #[error(transparent)]
    Checker(#[from] CheckerError),
}

fn refusal(reason: impl Into<String>) -> AdmissionError {
    AdmissionError::Review(vec![ReviewReason(reason.into())])
}

impl<'s> Checker<'s> {
    /// Evaluate through the existing conjunction, then seal exactly that plan (v4 §7.9; v4 §125).
    pub fn authorize_repair(
        &self,
        plan: RepairPlan,
    ) -> Result<AuthorizedRepair<'s>, AdmissionError> {
        let basis = self.validate_admission_inputs(&plan)?;
        let evidence = match self.evaluate_repair(&plan)? {
            RepairDecision::AutoApply { evidence } => evidence,
            RepairDecision::NeedsReview { reasons } => return Err(AdmissionError::Review(reasons)),
        };
        if self.store.head()? != basis.revision {
            return Err(refusal("store changed while checking repair"));
        }
        Ok(AuthorizedRepair {
            basis,
            plan,
            evidence,
        })
    }

    /// Explicit local approval is a separate route, requiring caller identity (R4 §6).
    /// It may resolve automatic review requirements, but cannot override invalid
    /// input bindings, subject/operation mismatch, or an unexecutable DAG.
    /// Actor identity is a caller claim, not authentication proof.
    pub fn accept_repair(
        &self,
        plan: RepairPlan,
        actor: ActorId,
    ) -> Result<AuthorizedRepair<'s>, AdmissionError> {
        let basis = self.validate_admission_inputs(&plan)?;
        Ok(AuthorizedRepair {
            basis,
            plan,
            evidence: SafetyEvidence::HumanApproval {
                actor,
                at: Timestamp::now(),
            },
        })
    }

    fn validate_admission_inputs(
        &self,
        plan: &RepairPlan,
    ) -> Result<ValidatedBasis<'s>, AdmissionError> {
        let revision = self.store.head()?;
        crate::repair::topo_order(plan).map_err(|e| refusal(e.to_string()))?;
        if plan.steps.is_empty() {
            return Err(refusal("repair has no mutations"));
        }
        for (key, step) in &plan.steps {
            if *key != step.id {
                return Err(refusal("plan key and step identity disagree"));
            }
            let contract = self.contract_for(step.subject)?;
            match &step.operation {
                RepairOperation::Graph(op) => {
                    if graph_subject(op) != step.subject
                        || !matches!(contract.mutation.write_route, Holder::Graph)
                    {
                        return Err(refusal("graph operation and governing subject disagree"));
                    }
                    for predicate in [&step.expected_prestate, &step.expected_poststate] {
                        let matches = match predicate {
                            StatePredicate::NodeAt { node, .. } => {
                                step.subject == JurisdictionSubject::Node(*node)
                            }
                            StatePredicate::RelationAt { relation, .. } => {
                                step.subject == JurisdictionSubject::Relation(*relation)
                            }
                            StatePredicate::Any => true,
                            _ => false,
                        };
                        if !matches {
                            return Err(refusal("graph predicate and subject disagree"));
                        }
                    }
                }
                RepairOperation::WriteFile { path, contents } => {
                    check_file_route(
                        &contract.mutation.write_route,
                        path,
                        &step.expected_prestate,
                        &step.expected_poststate,
                    )?;
                    if step.expected_poststate
                        != (StatePredicate::FileContent {
                            path: path.clone(),
                            hash: ContentHash::of(contents),
                        })
                    {
                        return Err(refusal("file poststate does not describe proposed bytes"));
                    }
                }
                RepairOperation::InsertSourceId { path, .. } => {
                    check_file_route(
                        &contract.mutation.write_route,
                        path,
                        &step.expected_prestate,
                        &step.expected_poststate,
                    )?;
                }
                RepairOperation::External { .. } => {
                    return Err(refusal(
                        "no qualified external-service repair adapter in this phase",
                    ));
                }
            }
        }
        for dep in &plan.dependencies {
            if matches!(plan.steps[&dep.before].operation, RepairOperation::Graph(_))
                && !matches!(plan.steps[&dep.after].operation, RepairOperation::Graph(_))
            {
                return Err(refusal(
                    "graph-before-file ordering cannot be executed under ILRP",
                ));
            }
        }
        validate_basis(self.store, &plan.basis, revision)?;
        if self.store.head()? != revision {
            return Err(refusal("store changed while validating inputs"));
        }
        Ok(ValidatedBasis {
            store: self.store,
            revision,
            basis: plan.basis.clone(),
        })
    }
}

fn check_file_route(
    holder: &Holder,
    path: &liminal_id::PathId,
    pre: &StatePredicate,
    post: &StatePredicate,
) -> Result<(), AdmissionError> {
    if path.0.as_str().is_empty()
        || !std::path::Path::new(&path.0)
            .components()
            .all(|p| matches!(p, std::path::Component::Normal(_)))
    {
        return Err(refusal(
            "repair path must be workspace-relative without traversal",
        ));
    }
    if !matches!(holder, Holder::File { path: governed } if governed == path) {
        return Err(refusal("file operation and governing Holder disagree"));
    }
    for predicate in [pre, post] {
        if !matches!(predicate, StatePredicate::FileContent {path: p, ..} | StatePredicate::FileAbsent {path: p} if p == path)
        {
            return Err(refusal("file predicate does not name its operation path"));
        }
    }
    Ok(())
}

fn graph_subject(op: &Operation) -> JurisdictionSubject {
    use JurisdictionSubject::{Node, Relation};
    match op {
        Operation::CreateNode { node } => Node(node.id),
        Operation::DeleteNode { id } | Operation::SetPayload { id, .. } => Node(*id),
        Operation::AddRelation { relation } => Relation(relation.id),
        Operation::RemoveRelation { id } | Operation::RetargetRelation { id, .. } => Relation(*id),
        Operation::InsertChild { parent, .. } | Operation::MoveChild { parent, .. } => {
            Node(*parent)
        }
        Operation::AttachResource { node, .. } | Operation::MaterializeExternal { node, .. } => {
            Node(*node)
        }
    }
}

fn require_text_hash(
    store: &GraphStore,
    key: &str,
    expected: ContentHash,
) -> Result<(), AdmissionError> {
    let value = store
        .get_aux(ns::SYS_BLOB, key)?
        .ok_or_else(|| refusal("captured input blob is unavailable"))?;
    let text = value
        .as_str()
        .ok_or_else(|| refusal("captured text blob has wrong shape"))?;
    if ContentHash::of(text.as_bytes()) != expected {
        return Err(refusal("captured input hash mismatch"));
    }
    Ok(())
}

fn require_file_hash(
    store: &GraphStore,
    path: &liminal_id::PathId,
    hash: ContentHash,
) -> Result<(), AdmissionError> {
    if store.get_aux(ns::SYS_BLOB, &hash.to_hex())?.is_some() {
        return require_text_hash(store, &hash.to_hex(), hash);
    }
    require_text_hash(store, &format!("file/{path}"), hash)
}

fn validate_basis(
    store: &GraphStore,
    basis: &WorkspaceBasis,
    head: GraphRevisionId,
) -> Result<(), AdmissionError> {
    if matches!(
        basis.perspective,
        BasisPerspective::Published { .. } | BasisPerspective::Federated { .. }
    ) {
        return Err(refusal(
            "repair perspective has no admitted runtime in this phase",
        ));
    }
    for (key, component) in &basis.components {
        match component {
            BasisComponent::FileContent { path, hash }
                if key == &JurisdictionKey::Path(path.clone()) =>
            {
                require_file_hash(store, path, *hash)?;
            }
            BasisComponent::ObjectContent { hash } if key == &JurisdictionKey::Object(*hash) => {
                require_text_hash(store, &hash.to_hex(), *hash)?;
            }
            BasisComponent::GraphSnapshot { revision }
                if key == &liminal_revision::graph_key() && revision.0 <= head.0 =>
            {
                store.state_at(*revision)?;
            }
            BasisComponent::BufferGeneration {
                client,
                buffer,
                epoch,
                generation,
                content_hash: Some(hash),
                ..
            } if matches!(key, JurisdictionKey::Path(_))
                && matches!(basis.perspective, BasisPerspective::ClientScoped {client: selected} if selected == *client) =>
            {
                if store
                    .get_aux(ns::SYS_EPOCH, "epoch")?
                    .and_then(|v| v.as_u64())
                    != Some(epoch.0)
                {
                    return Err(refusal("buffer epoch is not the current workspace epoch"));
                }
                require_text_hash(store, &format!("buf/{client}/{buffer}/{generation}"), *hash)?;
            }
            BasisComponent::Observation { source, hash, .. }
                if key == &JurisdictionKey::Source(*source) =>
            {
                let blob = store
                    .get_aux(ns::SYS_BLOB, &format!("obs/{source}/{hash}"))?
                    .ok_or_else(|| refusal("observation blob unavailable"))?;
                let bytes = serde_json::to_vec(&blob).map_err(|e| refusal(e.to_string()))?;
                if ContentHash::of(&bytes) != *hash {
                    return Err(refusal("observation hash mismatch"));
                }
                let current = store.get_aux(ns::SYS_BLOB, &format!("obs-current/{source}"))?;
                let expected =
                    serde_json::to_value(component).map_err(|e| refusal(e.to_string()))?;
                if current.as_ref() != Some(&expected) {
                    return Err(refusal("observation metadata is not current"));
                }
            }
            _ => return Err(refusal("unsupported or mismatched Basis component")),
        }
    }
    Ok(())
}
