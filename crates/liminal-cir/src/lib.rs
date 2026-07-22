//! L2 derived graph and strict debug interchange (M19.3–M19.4).
//!
//! This crate contains no store or I/O.  Resolution is a pure function of an
//! L1 document and one caller-supplied [`WorkspaceBasis`]; the resulting graph
//! is explicitly marked [`NodeFlags::DERIVED`] and retains source provenance.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use liminal_graph::{Node, NodeFlags, PayloadRef, Relation, RelationFlags, Target};
use liminal_hir::{HirDocument, HirId, HirItemKind, HirReference, HirValue};
use liminal_id::{
    ContentHash, IdentityGrade, JurisdictionKey, KindId, NodeId, RelationId, RevisionId, SourceId,
};
use liminal_revision::{BasisComponent, WorkspaceBasis};
use liminal_source::{SourceBasis, SourceRange};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Source location and identity of a derived graph subject (v4 §§10–11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    /// Durable source identity.
    pub source: SourceId,
    /// Exact source content hash.
    pub content_hash: ContentHash,
    /// Authorial byte range.
    pub range: SourceRange,
    /// Continuity grade assigned by the derived-ID recipe.
    pub identity_grade: IdentityGrade,
}

/// Subject addressed by a provenance record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceSubject {
    /// Node subject.
    Node(NodeId),
    /// Relation subject.
    Relation(RelationId),
}

/// Basis-bound graph materialized from HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedGraph {
    /// Workspace basis used to derive graph.
    pub basis: WorkspaceBasis,
    /// Derived nodes sorted by ID.
    pub nodes: Vec<Node>,
    /// Derived relations sorted by ID.
    pub relations: Vec<Relation>,
    /// Provenance for every graph subject.
    pub provenance: BTreeMap<ProvenanceSubject, SourceProvenance>,
}

/// Non-fatal resolution diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolveDiagnosticCode {
    /// A symbolic reference had no matching explicit ID.
    UnresolvedReference,
    /// More than one item claimed one explicit ID.
    DuplicateExplicitId,
}

/// One non-fatal resolution diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveDiagnostic {
    /// Diagnostic class.
    pub code: ResolveDiagnosticCode,
    /// HIR item responsible, when known.
    pub subject: Option<HirId>,
    /// Human-readable detail.
    pub message: String,
}

/// Result of resolving HIR at one basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGraph {
    /// Materialized derived graph.
    pub graph: DerivedGraph,
    /// Non-fatal diagnostics.
    pub diagnostics: Vec<ResolveDiagnostic>,
}

/// Canonical payload envelope used by debug graph and derived nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedPayloadV1 {
    /// Payload schema version.
    pub schema_version: u32,
    /// Source HIR kind.
    pub kind: HirItemKind,
    /// Effective attributes.
    pub attributes: BTreeMap<String, HirValue>,
}

/// JSON provenance record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugProvenanceV1 {
    /// Graph subject.
    pub subject: ProvenanceSubject,
    /// Source identity.
    pub source: SourceId,
    /// Source content hash.
    pub content_hash: ContentHash,
    /// Source byte range.
    pub range: SourceRange,
    /// Identity continuity grade.
    pub identity_grade: IdentityGrade,
}

/// Stable debug graph JSON v1 object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugGraphV1 {
    /// Schema version; only `1` is accepted.
    pub schema_version: u32,
    /// Exact workspace basis.
    pub basis: WorkspaceBasis,
    /// Selected Holder key.
    pub holder: JurisdictionKey,
    /// Exact source basis.
    pub source: SourceBasis,
    /// Derived nodes sorted by ID.
    pub nodes: Vec<Node>,
    /// Derived relations sorted by ID.
    pub relations: Vec<Relation>,
    /// Provenance sorted by subject ID.
    pub provenance: Vec<DebugProvenanceV1>,
}

/// Fatal graph-resolution failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// A relation endpoint is not present in derived nodes.
    #[error("dangling relation endpoint")]
    DanglingRelation,
    /// Basis does not select compatible Holder component.
    #[error("Basis does not select compatible file/buffer Holder {0}")]
    BasisMismatch(JurisdictionKey),
    /// Distinct declared kinds collided in deterministic KindId space.
    #[error("phase-1 kind collision at {id:?}: {first} vs {second}")]
    KindCollision {
        /// Colliding derived kind ID.
        id: KindId,
        /// First declared name.
        first: String,
        /// Second declared name.
        second: String,
    },
    /// Payload JSON could not be encoded.
    #[error("canonical payload serialization failed: {0}")]
    Payload(String),
}

/// Fatal debug JSON error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DebugJsonError {
    /// Input was not valid JSON.
    #[error("malformed JSON: {0}")]
    Syntax(String),
    /// JSON shape differs from canonical shape.
    #[error("JSON shape mismatch at {path}")]
    Shape {
        /// JSON path where structure diverged.
        path: String,
    },
    /// JSON is semantically valid but not canonical v1 encoding.
    #[error("non-canonical debug JSON")]
    NonCanonical,
    /// Unsupported schema version.
    #[error("unsupported schema version {0}")]
    Version(u32),
    /// Semantic graph invariant failed.
    #[error("invalid debug graph: {0}")]
    Invalid(String),
}

/// Derive basis for exact source bytes held by one external-file Holder.
pub fn basis_for_source(
    base: &WorkspaceBasis,
    holder: &JurisdictionKey,
    hash: ContentHash,
) -> Result<WorkspaceBasis, ResolveError> {
    let Some(component) = base.components.get(holder) else {
        return Err(ResolveError::BasisMismatch(holder.clone()));
    };
    let mut next = base.clone();
    let replacement = match component {
        BasisComponent::FileContent { path, .. } => BasisComponent::FileContent {
            path: path.clone(),
            hash,
        },
        BasisComponent::BufferGeneration {
            client,
            buffer,
            epoch,
            generation,
            base_file_hash,
            ..
        } => BasisComponent::BufferGeneration {
            client: *client,
            buffer: *buffer,
            epoch: *epoch,
            generation: *generation,
            content_hash: Some(hash),
            base_file_hash: *base_file_hash,
        },
        _ => return Err(ResolveError::BasisMismatch(holder.clone())),
    };
    next.components.insert(holder.clone(), replacement);
    Ok(next)
}

/// Convert a resolved graph to stable debug JSON v1.
pub fn to_debug_v1(
    graph: &DerivedGraph,
    holder: JurisdictionKey,
    source: SourceBasis,
) -> DebugGraphV1 {
    let mut provenance: Vec<_> = graph
        .provenance
        .iter()
        .map(|(subject, value)| DebugProvenanceV1 {
            subject: *subject,
            source: value.source,
            content_hash: value.content_hash,
            range: value.range,
            identity_grade: value.identity_grade,
        })
        .collect();
    provenance.sort_by_key(|item| item.subject);
    DebugGraphV1 {
        schema_version: 1,
        basis: graph.basis.clone(),
        holder,
        source,
        nodes: sorted_nodes(&graph.nodes),
        relations: sorted_relations(&graph.relations),
        provenance,
    }
}

/// Serialize debug JSON with fixed pretty-printing and one trailing LF.
pub fn serialize_debug_v1(value: &DebugGraphV1) -> Result<Vec<u8>, DebugJsonError> {
    validate_debug_graph(value)?;
    if value.schema_version != 1 {
        return Err(DebugJsonError::Version(value.schema_version));
    }
    let mut canonical = value.clone();
    canonical.nodes = sorted_nodes(&canonical.nodes);
    canonical.relations = sorted_relations(&canonical.relations);
    canonical.provenance.sort_by_key(|entry| entry.subject);
    let mut bytes = serde_json::to_vec_pretty(&canonical)
        .map_err(|err| DebugJsonError::Syntax(err.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Deserialize strict debug JSON v1, rejecting unknown/missing fields and shape drift.
pub fn deserialize_debug_v1(bytes: &[u8]) -> Result<DebugGraphV1, DebugJsonError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(DebugJsonError::Syntax("input is not UTF-8".to_owned()));
    }
    if !bytes.ends_with(b"\n") || bytes[..bytes.len().saturating_sub(1)].ends_with(b"\n") {
        return Err(DebugJsonError::Syntax(
            "debug JSON must have one trailing LF".to_owned(),
        ));
    }
    let value: DebugGraphV1 =
        serde_json::from_slice(bytes).map_err(|err| DebugJsonError::Syntax(err.to_string()))?;
    if value.schema_version != 1 {
        return Err(DebugJsonError::Version(value.schema_version));
    }
    validate_debug_graph(&value)?;
    let canonical = serialize_debug_v1(&value)?;
    let input: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|err| DebugJsonError::Syntax(err.to_string()))?;
    let expected: serde_json::Value = serde_json::from_slice(&canonical)
        .map_err(|err| DebugJsonError::Syntax(err.to_string()))?;
    compare_shape(&input, &expected, "$")?;
    if canonical != bytes {
        return Err(DebugJsonError::NonCanonical);
    }
    Ok(value)
}

fn sorted_nodes(nodes: &[Node]) -> Vec<Node> {
    let mut out = nodes.to_vec();
    out.sort_by_key(|node| node.id);
    out
}

fn sorted_relations(relations: &[Relation]) -> Vec<Relation> {
    let mut out = relations.to_vec();
    out.sort_by_key(|relation| relation.id);
    out
}

fn validate_debug_graph(value: &DebugGraphV1) -> Result<(), DebugJsonError> {
    let nodes = sorted_nodes(&value.nodes);
    let mut node_ids = BTreeSet::new();
    for node in &nodes {
        if !node.flags.contains(NodeFlags::DERIVED) {
            return Err(DebugJsonError::Invalid(format!(
                "node {} is not derived",
                node.id
            )));
        }
        if !node_ids.insert(node.id) {
            return Err(DebugJsonError::Invalid(format!(
                "duplicate node {}",
                node.id
            )));
        }
    }
    let relations = sorted_relations(&value.relations);
    let mut relation_ids = BTreeSet::new();
    for relation in &relations {
        if !relation_ids.insert(relation.id) {
            return Err(DebugJsonError::Invalid(format!(
                "duplicate relation {}",
                relation.id
            )));
        }
        if !node_ids.contains(&relation.source) || !node_ids.contains(&relation.target.node()) {
            return Err(DebugJsonError::Invalid(
                "dangling relation endpoint".to_owned(),
            ));
        }
    }
    let provenance_subjects: BTreeSet<_> =
        value.provenance.iter().map(|item| item.subject).collect();
    if provenance_subjects.len() != value.provenance.len() {
        return Err(DebugJsonError::Invalid(
            "duplicate provenance subject".to_owned(),
        ));
    }
    if provenance_subjects.len() != nodes.len() + relations.len() {
        return Err(DebugJsonError::Invalid(
            "provenance contains unknown subject".to_owned(),
        ));
    }
    for node in &nodes {
        if !provenance_subjects.contains(&ProvenanceSubject::Node(node.id)) {
            return Err(DebugJsonError::Invalid(format!(
                "missing provenance for {}",
                node.id
            )));
        }
    }
    for relation in &relations {
        if !provenance_subjects.contains(&ProvenanceSubject::Relation(relation.id)) {
            return Err(DebugJsonError::Invalid(format!(
                "missing provenance for {}",
                relation.id
            )));
        }
    }
    let Some(component) = value.basis.components.get(&value.holder) else {
        return Err(DebugJsonError::Invalid(
            "holder absent from basis".to_owned(),
        ));
    };
    let basis_hash = match component {
        BasisComponent::FileContent { hash, .. } => Some(*hash),
        BasisComponent::BufferGeneration { content_hash, .. } => *content_hash,
        _ => None,
    };
    if basis_hash != Some(value.source.content_hash) {
        return Err(DebugJsonError::Invalid(
            "basis/source hash mismatch".to_owned(),
        ));
    }
    if value.provenance.iter().any(|entry| {
        entry.source != value.source.source || entry.content_hash != value.source.content_hash
    }) {
        return Err(DebugJsonError::Invalid(
            "provenance/source mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn compare_shape(
    input: &serde_json::Value,
    canonical: &serde_json::Value,
    path: &str,
) -> Result<(), DebugJsonError> {
    match (input, canonical) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            if a.len() != b.len() || a.keys().any(|key| !b.contains_key(key)) {
                return Err(DebugJsonError::Shape {
                    path: path.to_owned(),
                });
            }
            for (key, value) in b {
                compare_shape(
                    a.get(key).expect("key checked"),
                    value,
                    &format!("{path}.{key}"),
                )?;
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a.len() != b.len() {
                return Err(DebugJsonError::Shape {
                    path: path.to_owned(),
                });
            }
            for (index, (left, right)) in a.iter().zip(b).enumerate() {
                compare_shape(left, right, &format!("{path}[{index}]"))?;
            }
        }
        (a, b) if std::mem::discriminant(a) != std::mem::discriminant(b) => {
            return Err(DebugJsonError::Shape {
                path: path.to_owned(),
            });
        }
        _ => {}
    }
    Ok(())
}

/// Deterministic UUIDv5 identifier for one derived Node.
#[must_use]
pub fn derived_node_id(source: SourceId, form: &str, path: &[u32]) -> NodeId {
    NodeId::from_uuid(derived_uuid(b"node", source, form, path))
}

/// Deterministic UUIDv5 identifier for one derived Relation.
#[must_use]
pub fn derived_relation_id(source: SourceId, tag: &str, form: &str, path: &[u32]) -> RelationId {
    RelationId::from_uuid(derived_uuid(tag.as_bytes(), source, form, path))
}

fn explicit_node_id(source: SourceId, spelling: &str) -> NodeId {
    let mut bytes = Vec::from(&b"liminal:phase1:explicit:v1\0"[..]);
    bytes.extend_from_slice(source.as_uuid().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(spelling.as_bytes());
    NodeId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, &bytes))
}

fn derived_uuid(tag: &[u8], source: SourceId, form: &str, path: &[u32]) -> Uuid {
    let mut bytes = Vec::from(&b"liminal:phase1:derived:v1\0"[..]);
    bytes.extend_from_slice(tag);
    bytes.push(0);
    bytes.extend_from_slice(source.as_uuid().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(form.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&u32::try_from(path.len()).unwrap_or(u32::MAX).to_be_bytes());
    for child in path {
        bytes.extend_from_slice(&child.to_be_bytes());
    }
    Uuid::new_v5(&Uuid::NAMESPACE_URL, &bytes)
}

/// Return source identity from one HIR document.
#[must_use]
pub fn source_basis(document: &HirDocument) -> SourceBasis {
    document.source_map.basis.clone()
}

/// Resolve one HIR document into a deterministic, Basis-bound derived graph.
#[allow(clippy::too_many_lines, clippy::items_after_statements)]
pub fn resolve(hir: &HirDocument, basis: &WorkspaceBasis) -> Result<ResolvedGraph, ResolveError> {
    let source = hir.source_map.basis.clone();
    let source_selected = basis.components.values().any(|component| match component {
        BasisComponent::FileContent { hash, .. }
        | BasisComponent::BufferGeneration {
            content_hash: Some(hash),
            ..
        } => *hash == source.content_hash,
        _ => false,
    });
    if !source_selected {
        return Err(ResolveError::BasisMismatch(JurisdictionKey::Source(
            source.source,
        )));
    }
    let document_id = derived_node_id(source.source, "node-construction", &[]);
    let mut paths = HashMap::<HirId, Vec<u32>>::new();
    let mut parents = HashMap::<HirId, HirId>::new();
    fn walk(
        hir: &HirDocument,
        id: HirId,
        path: &mut Vec<u32>,
        paths: &mut HashMap<HirId, Vec<u32>>,
        parents: &mut HashMap<HirId, HirId>,
    ) {
        paths.insert(id, path.clone());
        if let Some(item) = hir.items.iter().find(|item| item.id == id) {
            for (index, child) in item.children.iter().copied().enumerate() {
                parents.insert(child, id);
                path.push(u32::try_from(index).unwrap_or(u32::MAX));
                walk(hir, child, path, paths, parents);
                path.pop();
            }
        }
    }
    for (index, root) in hir.roots.iter().copied().enumerate() {
        walk(
            hir,
            root,
            &mut vec![u32::try_from(index).unwrap_or(u32::MAX)],
            &mut paths,
            &mut parents,
        );
    }
    let mut symbols = BTreeMap::<String, HirId>::new();
    let mut duplicates = BTreeSet::new();
    for item in &hir.items {
        if let Some(HirValue::String(id)) = item.attributes.get("id")
            && symbols.insert(id.clone(), item.id).is_some()
        {
            duplicates.insert(id.clone());
        }
    }
    for duplicate in &duplicates {
        symbols.remove(duplicate);
    }
    let mut effective_attributes = BTreeMap::<HirId, BTreeMap<String, HirValue>>::new();
    for item in &hir.items {
        effective_attributes.insert(item.id, item.attributes.clone());
    }
    for item in &hir.items {
        if let HirItemKind::AttributeAssignment {
            owner: Some(owner),
            name,
            value,
        } = &item.kind
            && let Some(attributes) = effective_attributes.get_mut(owner)
        {
            attributes
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }
    symbols.clear();
    duplicates.clear();
    for (item_id, attributes) in &effective_attributes {
        if let Some(HirValue::String(id)) = attributes.get("id")
            && symbols.insert(id.clone(), *item_id).is_some()
        {
            duplicates.insert(id.clone());
        }
    }
    for duplicate in &duplicates {
        symbols.remove(duplicate);
    }
    let mut diagnostics = hir
        .items
        .iter()
        .filter_map(|item| {
            match effective_attributes
                .get(&item.id)
                .and_then(|attrs| attrs.get("id"))
            {
                Some(HirValue::String(id)) if duplicates.contains(id) => Some(ResolveDiagnostic {
                    code: ResolveDiagnosticCode::DuplicateExplicitId,
                    subject: Some(item.id),
                    message: format!("duplicate explicit id {id:?}"),
                }),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let mut kind_names = BTreeMap::<KindId, String>::new();
    let mut ensure_kind = |name: &str| -> Result<KindId, ResolveError> {
        let id = kind_id(name);
        if let Some(existing) = kind_names.get(&id)
            && existing != name
        {
            return Err(ResolveError::KindCollision {
                id,
                first: existing.clone(),
                second: name.to_owned(),
            });
        }
        kind_names.insert(id, name.to_owned());
        Ok(id)
    };
    let document_kind = HirItemKind::NodeConstruction {
        name: "document".to_owned(),
    };
    let document_payload = payload_for(&document_kind, &BTreeMap::new())?;
    let mut nodes = vec![Node {
        id: document_id,
        kind: ensure_kind("document")?,
        payload: PayloadRef::Text(document_payload),
        revision: RevisionId(0),
        flags: NodeFlags::DERIVED,
    }];
    let mut node_for = BTreeMap::<HirId, NodeId>::new();
    let mut provenance = BTreeMap::from([(
        ProvenanceSubject::Node(document_id),
        SourceProvenance {
            source: source.source,
            content_hash: source.content_hash,
            range: SourceRange {
                start: 0,
                end: hir
                    .items
                    .iter()
                    .map(|item| item.range.end)
                    .chain(hir.source_map.annotations.iter().map(|item| item.range.end))
                    .max()
                    .unwrap_or(0),
            },
            identity_grade: IdentityGrade::Anchored,
        },
    )]);
    for item in &hir.items {
        let Some(name) = node_kind_name(&item.kind) else {
            continue;
        };
        let path = paths.get(&item.id).map_or(&[][..], Vec::as_slice);
        let attributes = effective_attributes
            .get(&item.id)
            .cloned()
            .unwrap_or_default();
        let explicit = match attributes.get("id") {
            Some(HirValue::String(id)) if !duplicates.contains(id) => Some(id.as_str()),
            _ => None,
        };
        let id = explicit.map_or_else(
            || derived_node_id(source.source, form_name(&item.kind), path),
            |name| explicit_node_id(source.source, name),
        );
        let (flags, grade) = if explicit.is_some() {
            (
                NodeFlags::DERIVED.union(NodeFlags::HAS_DURABLE_ID),
                IdentityGrade::Explicit,
            )
        } else {
            (NodeFlags::DERIVED, IdentityGrade::Anchored)
        };
        nodes.push(Node {
            id,
            kind: ensure_kind(name)?,
            payload: PayloadRef::Text(payload_for(&item.kind, &attributes)?),
            revision: RevisionId(0),
            flags,
        });
        node_for.insert(item.id, id);
        provenance.insert(
            ProvenanceSubject::Node(id),
            SourceProvenance {
                source: source.source,
                content_hash: source.content_hash,
                range: item.range,
                identity_grade: grade,
            },
        );
    }
    let mut relations = Vec::new();
    for item in &hir.items {
        if let Some(&child) = node_for.get(&item.id) {
            let parent = parents
                .get(&item.id)
                .and_then(|parent| node_for.get(parent).copied())
                .unwrap_or(document_id);
            let parent_item = parents
                .get(&item.id)
                .and_then(|id| hir.items.iter().find(|candidate| candidate.id == *id));
            let should_contain = parent_item.is_none_or(|candidate| {
                matches!(
                    candidate.kind,
                    HirItemKind::NodeConstruction { .. } | HirItemKind::OrderedBlock
                )
            });
            if should_contain {
                let index = parent_item
                    .and_then(|candidate| candidate.children.iter().position(|id| *id == item.id))
                    .or_else(|| hir.roots.iter().position(|id| *id == item.id))
                    .unwrap_or(0);
                let path = paths.get(&item.id).map_or(&[][..], Vec::as_slice);
                let relation = Relation {
                    id: derived_relation_id(source.source, "relation:contains", "contains", path),
                    source: parent,
                    target: Target::Node(child),
                    kind: ensure_kind("contains")?,
                    payload: PayloadRef::Text(index.to_string()),
                    revision: RevisionId(0),
                    flags: RelationFlags::default(),
                    requires: None,
                };
                add_provenance(
                    &mut provenance,
                    relation.id,
                    item.range,
                    source.source,
                    source.content_hash,
                );
                relations.push(relation);
            }
        }
        if let HirItemKind::RelationConstruction {
            source: from,
            kind,
            target,
        } = &item.kind
        {
            let Some(from) = resolve_reference(from, &node_for, &symbols) else {
                diagnostics.push(unresolved(item.id, from));
                continue;
            };
            let Some(target) = resolve_reference(target, &node_for, &symbols) else {
                diagnostics.push(unresolved(item.id, target));
                continue;
            };
            let path = paths.get(&item.id).map_or(&[][..], Vec::as_slice);
            let relation = Relation {
                id: derived_relation_id(
                    source.source,
                    "relation:explicit",
                    form_name(&item.kind),
                    path,
                ),
                source: from,
                target: Target::Node(target),
                kind: ensure_kind(kind)?,
                payload: PayloadRef::Text(payload_for(
                    &item.kind,
                    &effective_attributes
                        .get(&item.id)
                        .cloned()
                        .unwrap_or_default(),
                )?),
                revision: RevisionId(0),
                flags: RelationFlags::default(),
                requires: None,
            };
            add_provenance(
                &mut provenance,
                relation.id,
                item.range,
                source.source,
                source.content_hash,
            );
            relations.push(relation);
        }
        if let HirItemKind::Reference { target } = &item.kind {
            let Some(target) = resolve_reference(target, &node_for, &symbols) else {
                diagnostics.push(unresolved(item.id, target));
                continue;
            };
            let owner = nearest_owner(item.id, &parents, &node_for).unwrap_or(document_id);
            let path = paths.get(&item.id).map_or(&[][..], Vec::as_slice);
            let relation = Relation {
                id: derived_relation_id(source.source, "relation:reference", "reference", path),
                source: owner,
                target: Target::Node(target),
                kind: ensure_kind("reference")?,
                payload: PayloadRef::Text(payload_for(&item.kind, &BTreeMap::new())?),
                revision: RevisionId(0),
                flags: RelationFlags::default(),
                requires: None,
            };
            add_provenance(
                &mut provenance,
                relation.id,
                item.range,
                source.source,
                source.content_hash,
            );
            relations.push(relation);
        }
    }
    nodes.sort_by_key(|node| node.id);
    relations.sort_by_key(|relation| relation.id);
    Ok(ResolvedGraph {
        graph: DerivedGraph {
            basis: basis.clone(),
            nodes,
            relations,
            provenance,
        },
        diagnostics,
    })
}

fn add_provenance(
    provenance: &mut BTreeMap<ProvenanceSubject, SourceProvenance>,
    id: RelationId,
    range: SourceRange,
    source: SourceId,
    content_hash: ContentHash,
) {
    provenance.insert(
        ProvenanceSubject::Relation(id),
        SourceProvenance {
            source,
            content_hash,
            range,
            identity_grade: IdentityGrade::Anchored,
        },
    );
}

fn unresolved(item: HirId, reference: &HirReference) -> ResolveDiagnostic {
    ResolveDiagnostic {
        code: ResolveDiagnosticCode::UnresolvedReference,
        subject: Some(item),
        message: format!("unresolved reference {reference:?}"),
    }
}

fn resolve_reference(
    reference: &HirReference,
    node_for: &BTreeMap<HirId, NodeId>,
    symbols: &BTreeMap<String, HirId>,
) -> Option<NodeId> {
    match reference {
        HirReference::Resolved(id) => node_for.get(id).copied(),
        HirReference::Unresolved(name) => {
            symbols.get(name).and_then(|id| node_for.get(id).copied())
        }
    }
}

fn nearest_owner(
    item: HirId,
    parents: &HashMap<HirId, HirId>,
    node_for: &BTreeMap<HirId, NodeId>,
) -> Option<NodeId> {
    let mut cursor = item;
    while let Some(parent) = parents.get(&cursor).copied() {
        if let Some(node) = node_for.get(&parent).copied() {
            return Some(node);
        }
        cursor = parent;
    }
    None
}

fn node_kind_name(kind: &HirItemKind) -> Option<&str> {
    match kind {
        HirItemKind::Literal { .. } => Some("literal"),
        HirItemKind::NodeConstruction { name } => Some(name),
        HirItemKind::OrderedBlock => Some("ordered-block"),
        HirItemKind::Expression { .. } => Some("expression"),
        HirItemKind::MacroInvocation { .. } => Some("macro-invocation"),
        _ => None,
    }
}

fn form_name(kind: &HirItemKind) -> &'static str {
    match kind {
        HirItemKind::Literal { .. } => "literal",
        HirItemKind::NodeConstruction { .. } => "node-construction",
        HirItemKind::RelationConstruction { .. } => "relation-construction",
        HirItemKind::AttributeAssignment { .. } => "attribute-assignment",
        HirItemKind::OrderedBlock => "ordered-block",
        HirItemKind::Reference { .. } => "reference",
        HirItemKind::Expression { .. } => "expression",
        HirItemKind::MacroInvocation { .. } => "macro-invocation",
    }
}

fn payload_for(
    kind: &HirItemKind,
    attributes: &BTreeMap<String, HirValue>,
) -> Result<String, ResolveError> {
    serde_json::to_string(&DerivedPayloadV1 {
        schema_version: 1,
        kind: kind.clone(),
        attributes: attributes.clone(),
    })
    .map_err(|error| ResolveError::Payload(error.to_string()))
}

fn kind_id(name: &str) -> KindId {
    let mut bytes = Vec::from(&b"liminal:phase1:kind:v1\0"[..]);
    bytes.extend_from_slice(name.as_bytes());
    let hash = blake3::hash(&bytes);
    KindId(u32::from_be_bytes(hash.as_bytes()[..4].try_into().expect("four bytes")) | 0x8000_0000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liminal_id::{PathId, TransactionId};
    use liminal_revision::BasisPerspective;

    fn fixture() -> DebugGraphV1 {
        let source = SourceId::from_name("test-source");
        let hash = ContentHash::of(b"fixture");
        let holder = JurisdictionKey::Path(PathId("fixture.md".into()));
        let basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::FileContent {
                    path: PathId("fixture.md".into()),
                    hash,
                },
            )]),
        };
        let id = derived_node_id(source, "literal", &[0]);
        DebugGraphV1 {
            schema_version: 1,
            basis,
            holder,
            source: SourceBasis {
                source,
                content_hash: hash,
            },
            nodes: vec![Node {
                id,
                kind: KindId(1),
                payload: PayloadRef::None,
                revision: RevisionId(0),
                flags: NodeFlags::DERIVED,
            }],
            relations: Vec::new(),
            provenance: vec![DebugProvenanceV1 {
                subject: ProvenanceSubject::Node(id),
                source,
                content_hash: hash,
                range: SourceRange { start: 0, end: 7 },
                identity_grade: IdentityGrade::Anchored,
            }],
        }
    }

    #[test]
    fn derived_ids_are_stable_and_path_sensitive() {
        let source = SourceId::from_name("stable");
        assert_eq!(
            derived_node_id(source, "literal", &[0]),
            derived_node_id(source, "literal", &[0])
        );
        assert_ne!(
            derived_node_id(source, "literal", &[0]),
            derived_node_id(source, "literal", &[1])
        );
    }

    #[test]
    fn debug_json_round_trips_and_enforces_framing() {
        let value = fixture();
        let bytes = serialize_debug_v1(&value).expect("fixture valid");
        assert!(bytes.ends_with(b"\n"));
        assert_eq!(
            deserialize_debug_v1(&bytes)
                .expect("round trip")
                .schema_version,
            1
        );
        assert!(deserialize_debug_v1(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn debug_json_rejects_unknown_field() {
        let value = fixture();
        let bytes = serialize_debug_v1(&value).expect("fixture valid");
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        json.as_object_mut()
            .expect("object")
            .insert("extra".into(), serde_json::Value::Null);
        let mut altered = serde_json::to_vec_pretty(&json).expect("json");
        altered.push(b'\n');
        assert!(deserialize_debug_v1(&altered).is_err());
    }
}
