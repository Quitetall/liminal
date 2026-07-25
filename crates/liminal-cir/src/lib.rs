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
    use liminal_hir::{Annotation, AnnotationKind, HirDocument, HirItem};
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

    pub(super) fn hir_fixture() -> (HirDocument, WorkspaceBasis) {
        let source = SourceId::from_name("test-source");
        let hash = ContentHash::of(b"fixture");
        let holder = JurisdictionKey::Path(PathId("fixture.md".into()));
        let basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder,
                BasisComponent::FileContent {
                    path: PathId("fixture.md".into()),
                    hash,
                },
            )]),
        };
        let range = SourceRange { start: 0, end: 7 };
        let root = HirItem {
            id: HirId(0),
            range,
            kind: HirItemKind::NodeConstruction {
                name: "root".into(),
            },
            attributes: BTreeMap::from([("id".into(), HirValue::String("root".into()))]),
            children: vec![HirId(1), HirId(2), HirId(3)],
        };
        let literal = HirItem {
            id: HirId(1),
            range,
            kind: HirItemKind::Literal {
                value: "value".into(),
            },
            attributes: BTreeMap::new(),
            children: Vec::new(),
        };
        let relation = HirItem {
            id: HirId(2),
            range,
            kind: HirItemKind::RelationConstruction {
                source: HirReference::Unresolved("root".into()),
                kind: "links".into(),
                target: HirReference::Unresolved("root".into()),
            },
            attributes: BTreeMap::new(),
            children: Vec::new(),
        };
        let reference = HirItem {
            id: HirId(3),
            range,
            kind: HirItemKind::Reference {
                target: HirReference::Unresolved("root".into()),
            },
            attributes: BTreeMap::new(),
            children: Vec::new(),
        };
        (
            HirDocument {
                basis: SourceBasis {
                    source,
                    content_hash: hash,
                },
                roots: vec![HirId(0)],
                items: vec![root, literal, relation, reference],
                source_map: liminal_hir::SourceMap {
                    basis: SourceBasis {
                        source,
                        content_hash: hash,
                    },
                    annotations: vec![Annotation {
                        kind: AnnotationKind::Form,
                        range,
                        hir: Some(HirId(0)),
                    }],
                },
            },
            basis,
        )
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

    #[test]
    fn debug_validation_rejects_duplicate_and_missing_provenance() {
        let mut duplicate = fixture();
        duplicate.nodes.push(duplicate.nodes[0].clone());
        assert!(matches!(
            serialize_debug_v1(&duplicate),
            Err(DebugJsonError::Invalid(message)) if message.contains("duplicate node")
        ));

        let mut missing = fixture();
        missing.provenance.clear();
        assert!(matches!(
            serialize_debug_v1(&missing),
            Err(DebugJsonError::Invalid(message)) if message.contains("provenance")
        ));

        let mut wrong_hash = fixture();
        wrong_hash.source.content_hash = ContentHash::of(b"other");
        assert!(matches!(
            serialize_debug_v1(&wrong_hash),
            Err(DebugJsonError::Invalid(message)) if message.contains("hash mismatch")
        ));
    }

    #[test]
    fn resolve_materializes_relations_and_provenance_at_basis() {
        let (hir, basis) = hir_fixture();
        let resolved = resolve(&hir, &basis).expect("basis-selected HIR resolves");
        assert!(resolved.diagnostics.is_empty());
        assert!(resolved.graph.nodes.len() >= 3);
        assert!(resolved.graph.relations.len() >= 3);
        assert_eq!(
            resolved.graph.provenance.len(),
            resolved.graph.nodes.len() + resolved.graph.relations.len()
        );
        assert!(
            resolved
                .graph
                .nodes
                .iter()
                .all(|node| node.flags.contains(NodeFlags::DERIVED))
        );
    }

    #[test]
    fn resolve_reports_unresolved_and_duplicate_symbols_and_rejects_wrong_basis() {
        let (mut hir, basis) = hir_fixture();
        hir.items[2].kind = HirItemKind::Reference {
            target: HirReference::Unresolved("missing".into()),
        };
        let unresolved = resolve(&hir, &basis).expect("unresolved refs are diagnostic");
        assert!(
            unresolved
                .diagnostics
                .iter()
                .any(|item| item.code == ResolveDiagnosticCode::UnresolvedReference)
        );

        hir.items.push(HirItem {
            id: HirId(4),
            range: SourceRange { start: 0, end: 7 },
            kind: HirItemKind::NodeConstruction {
                name: "other".into(),
            },
            attributes: BTreeMap::from([("id".into(), HirValue::String("root".into()))]),
            children: Vec::new(),
        });
        hir.roots.push(HirId(4));
        let duplicate = resolve(&hir, &basis).expect("duplicate IDs are diagnostic");
        assert!(
            duplicate
                .diagnostics
                .iter()
                .any(|item| item.code == ResolveDiagnosticCode::DuplicateExplicitId)
        );

        let mut wrong = basis;
        let holder = wrong.components.keys().next().cloned().unwrap();
        wrong.components.insert(
            holder,
            BasisComponent::FileContent {
                path: PathId("fixture.md".into()),
                hash: ContentHash::of(b"wrong"),
            },
        );
        assert!(matches!(
            resolve(&hir, &wrong),
            Err(ResolveError::BasisMismatch(_))
        ));
    }
}

/// Mutation kills for M17.5 finding F-08.
///
/// `cargo-mutants` found 34 survivors in this file — every one a validation or
/// projection defect the suite could not see. The headline case:
/// `sorted_relations` returning `vec![]`, so the derived graph reports **no
/// relations at all**, passed the entire 269-test workspace suite.
///
/// These are deliberately NEGATIVE and CONTENT tests: each pins a specific
/// seeded defect. A test here going green against a mutated build means the
/// mutant survived.
#[cfg(test)]
mod mutation_kills {
    use super::tests::hir_fixture;
    use super::*;
    use liminal_graph::{Relation, RelationFlags, Target};
    use liminal_hir::{HirId, HirItemKind, HirReference, HirValue};
    use liminal_id::{PathId, TransactionId};
    use liminal_revision::BasisPerspective;
    use std::collections::HashMap;

    /// A two-node, two-relation graph with complete provenance.
    fn graph_with_relations() -> DebugGraphV1 {
        let source = SourceId::from_name("mutation-kill");
        let hash = ContentHash::of(b"mutation-kill");
        let holder = JurisdictionKey::Path(PathId("kill.md".into()));
        let basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::FileContent {
                    path: PathId("kill.md".into()),
                    hash,
                },
            )]),
        };
        let range = SourceRange { start: 0, end: 4 };
        let a = derived_node_id(source, "literal", &[0]);
        let b = derived_node_id(source, "literal", &[1]);
        let node = |id| Node {
            id,
            kind: KindId(1),
            payload: PayloadRef::None,
            revision: RevisionId(0),
            flags: NodeFlags::DERIVED,
        };
        let r1 = derived_relation_id(source, "links", "relation", &[0]);
        let r2 = derived_relation_id(source, "links", "relation", &[1]);
        let relation = |id, src, dst| Relation {
            id,
            source: src,
            target: Target::Node(dst),
            kind: KindId(2),
            payload: PayloadRef::None,
            revision: RevisionId(0),
            flags: RelationFlags(0),
            requires: None,
        };
        let prov = |subject| DebugProvenanceV1 {
            subject,
            source,
            content_hash: hash,
            range,
            identity_grade: IdentityGrade::Anchored,
        };
        DebugGraphV1 {
            schema_version: 1,
            basis,
            holder,
            source: SourceBasis {
                source,
                content_hash: hash,
            },
            nodes: vec![node(a), node(b)],
            relations: vec![relation(r1, a, b), relation(r2, b, a)],
            provenance: vec![
                prov(ProvenanceSubject::Node(a)),
                prov(ProvenanceSubject::Node(b)),
                prov(ProvenanceSubject::Relation(r1)),
                prov(ProvenanceSubject::Relation(r2)),
            ],
        }
    }

    /// Kills `replace sorted_relations -> Vec<Relation> with vec![]`.
    #[test]
    fn relations_survive_validation_and_round_trip_in_id_order() {
        let graph = graph_with_relations();
        validate_debug_graph(&graph).expect("well-formed graph validates");

        let bytes = serialize_debug_v1(&graph).expect("serializes");
        let back = deserialize_debug_v1(&bytes).expect("deserializes");
        assert_eq!(
            back.relations.len(),
            2,
            "round trip lost relations: {:?}",
            back.relations
        );

        let ordered = sorted_relations(&graph.relations);
        assert_eq!(ordered.len(), 2, "sorted_relations dropped relations");
        assert!(
            ordered[0].id <= ordered[1].id,
            "sorted_relations did not order by id"
        );
        let ids: BTreeSet<_> = ordered.iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), 2, "sorted_relations collapsed distinct ids");
    }

    /// Kills the `delete !` on the duplicate-relation guard.
    #[test]
    fn duplicate_relation_id_is_rejected() {
        let mut graph = graph_with_relations();
        graph.relations[1].id = graph.relations[0].id;
        let err = validate_debug_graph(&graph).expect_err("duplicate relation must be rejected");
        assert!(
            format!("{err}").contains("duplicate relation"),
            "unexpected error: {err}"
        );
    }

    /// Kills `replace || with &&` and the `delete !`s on the dangling-endpoint
    /// guard. Source and target are checked SEPARATELY: with `&&` substituted,
    /// a single dangling side no longer trips the guard.
    #[test]
    fn dangling_relation_source_is_rejected() {
        let mut graph = graph_with_relations();
        graph.relations[0].source = derived_node_id(graph.source.source, "literal", &[99]);
        let err = validate_debug_graph(&graph).expect_err("dangling source must be rejected");
        assert!(
            format!("{err}").contains("dangling"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dangling_relation_target_is_rejected() {
        let mut graph = graph_with_relations();
        graph.relations[0].target =
            Target::Node(derived_node_id(graph.source.source, "literal", &[99]));
        let err = validate_debug_graph(&graph).expect_err("dangling target must be rejected");
        assert!(
            format!("{err}").contains("dangling"),
            "unexpected error: {err}"
        );
    }

    /// Kills `replace + with -` on the provenance-count check.
    #[test]
    fn provenance_must_cover_every_node_and_relation() {
        let mut graph = graph_with_relations();
        graph
            .provenance
            .retain(|entry| matches!(entry.subject, ProvenanceSubject::Node(_)));
        let err =
            validate_debug_graph(&graph).expect_err("provenance must cover nodes AND relations");
        assert!(
            format!("{err}").contains("provenance") || format!("{err}").contains("missing"),
            "unexpected error: {err}"
        );
    }

    /// Kills the duplicate-provenance-subject guard.
    #[test]
    fn duplicate_provenance_subject_is_rejected() {
        let mut graph = graph_with_relations();
        graph.provenance[1].subject = graph.provenance[0].subject;
        let err = validate_debug_graph(&graph).expect_err("duplicate provenance must be rejected");
        assert!(
            format!("{err}").contains("duplicate provenance"),
            "unexpected error: {err}"
        );
    }

    /// Kills `replace || with &&` on the provenance/source consistency check.
    #[test]
    fn provenance_source_mismatch_is_rejected() {
        let mut graph = graph_with_relations();
        graph.provenance[0].source = SourceId::from_name("some-other-source");
        assert!(
            validate_debug_graph(&graph).is_err(),
            "provenance naming a foreign source must be rejected"
        );

        let mut graph = graph_with_relations();
        graph.provenance[0].content_hash = ContentHash::of(b"different bytes");
        assert!(
            validate_debug_graph(&graph).is_err(),
            "provenance carrying a foreign content hash must be rejected"
        );
    }

    /// Kills all three mutants on the duplicate-id guard in `resolve`
    /// (`!duplicates.contains(id)` → true / → false / `delete !`).
    ///
    /// This pins a real identity law, not just a diagnostic: a spelling that
    /// appears twice is AMBIGUOUS, so neither claimant may be promoted to
    /// `Explicit` identity or given the durable-id flag. Promoting a
    /// duplicate would hand two different nodes the same durable identity —
    /// exactly the false promise M07/M09 measured against.
    #[test]
    fn duplicated_explicit_ids_are_never_promoted_to_explicit_identity() {
        let (mut hir, basis) = hir_fixture();
        let source = source_basis(&hir).source;

        // Unique id first: promotion MUST happen, so a guard stuck at `false`
        // (or an inverted `!`) is visible.
        let unique = resolve(&hir, &basis).expect("resolves");
        let promoted = unique
            .graph
            .nodes
            .iter()
            .find(|node| node.id == explicit_node_id(source, "root"))
            .expect("a unique explicit id must produce an explicit node id");
        assert!(
            promoted.flags.contains(NodeFlags::HAS_DURABLE_ID),
            "a unique explicit id must carry the durable-id flag"
        );

        // Now duplicate that spelling on a second item.
        hir.items.push(liminal_hir::HirItem {
            id: HirId(9),
            range: SourceRange { start: 0, end: 7 },
            kind: HirItemKind::NodeConstruction {
                name: "other".into(),
            },
            attributes: BTreeMap::from([("id".into(), HirValue::String("root".into()))]),
            children: Vec::new(),
        });
        hir.roots.push(HirId(9));

        let duplicated = resolve(&hir, &basis).expect("duplicate ids are diagnostic, not fatal");
        assert!(
            duplicated
                .graph
                .nodes
                .iter()
                .all(|node| node.id != explicit_node_id(source, "root")),
            "a duplicated spelling must not yield an explicit node id"
        );
        assert!(
            duplicated
                .graph
                .nodes
                .iter()
                .all(|node| !node.flags.contains(NodeFlags::HAS_DURABLE_ID)),
            "a duplicated spelling must not carry the durable-id flag"
        );
        assert!(
            duplicated
                .graph
                .provenance
                .iter()
                .all(|(_, entry)| entry.identity_grade != IdentityGrade::Explicit),
            "a duplicated spelling must not be graded Explicit"
        );
    }

    /// Kills the deleted `FileContent` / `BufferGeneration` match arms in
    /// `basis_for_source`: each component kind must have its hash rebased to
    /// the exact source bytes, and the other fields preserved.
    #[test]
    fn basis_for_source_rebases_every_component_kind() {
        use liminal_id::{BufferId, ClientId, SessionEpoch};

        let holder = JurisdictionKey::Path(PathId("rebase.md".into()));
        let fresh = ContentHash::of(b"fresh bytes");
        let stale = ContentHash::of(b"stale bytes");

        let file_basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::FileContent {
                    path: PathId("rebase.md".into()),
                    hash: stale,
                },
            )]),
        };
        let rebased = basis_for_source(&file_basis, &holder, fresh).expect("file basis rebases");
        match rebased.components.get(&holder).expect("component present") {
            BasisComponent::FileContent { path, hash } => {
                assert_eq!(*hash, fresh, "FileContent hash was not rebased");
                assert_eq!(path.0, "rebase.md", "FileContent path was not preserved");
            }
            other => panic!("FileContent arm produced {other:?}"),
        }

        let buffer_basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::BufferGeneration {
                    client: ClientId::new(),
                    buffer: BufferId::new(),
                    epoch: SessionEpoch::default(),
                    generation: 7,
                    base_file_hash: Some(stale),
                    content_hash: Some(stale),
                },
            )]),
        };
        let rebased =
            basis_for_source(&buffer_basis, &holder, fresh).expect("buffer basis rebases");
        match rebased.components.get(&holder).expect("component present") {
            BasisComponent::BufferGeneration {
                content_hash,
                generation,
                base_file_hash,
                ..
            } => {
                assert_eq!(
                    *content_hash,
                    Some(fresh),
                    "BufferGeneration hash was not rebased"
                );
                assert_eq!(*generation, 7, "generation was not preserved");
                assert_eq!(
                    *base_file_hash,
                    Some(stale),
                    "base_file_hash was not preserved"
                );
            }
            other => panic!("BufferGeneration arm produced {other:?}"),
        }
    }

    /// Kills all five `compare_shape` mutants directly: the whole-function
    /// `Ok(())`, both deleted match arms, the `|| -> &&` on the object-key
    /// check, and the discriminant guard replaced with `false`. Driving it
    /// through `deserialize_debug_v1` cannot reach these — serde's
    /// `deny_unknown_fields` rejects first — so the function is exercised
    /// directly.
    #[test]
    fn compare_shape_rejects_every_structural_difference() {
        use serde_json::json;
        let ok = |a: serde_json::Value, b: serde_json::Value| compare_shape(&a, &b, "$").is_ok();
        let bad = |a: serde_json::Value, b: serde_json::Value| compare_shape(&a, &b, "$").is_err();

        assert!(
            ok(json!({"a": 1}), json!({"a": 1})),
            "identical shapes agree"
        );

        // Object: differing key COUNT, and same count with a differing NAME.
        // The second case is what `|| -> &&` would let through.
        assert!(bad(json!({"a": 1, "b": 2}), json!({"a": 1})), "extra key");
        assert!(bad(json!({"a": 1}), json!({"b": 1})), "renamed key");

        // Array length.
        assert!(bad(json!([1, 2]), json!([1])), "array length");

        // Type discriminant mismatch (kills the `false` guard replacement).
        assert!(bad(json!(1), json!("1")), "number vs string");
        assert!(bad(json!(null), json!(0)), "null vs number");
        assert!(bad(json!({"a": 1}), json!([1])), "object vs array");

        // Nested difference must still surface.
        assert!(
            bad(json!({"a": {"b": 1}}), json!({"a": {"b": "1"}})),
            "nested"
        );
    }

    /// Kills the seven `node_kind_name` / `form_name` mutants: constant
    /// returns (`""`, `"xyzzy"`) and the deleted match arms. Every spelling is
    /// pinned to a literal, so any substitution is visible.
    #[test]
    fn hir_kind_and_form_names_are_exact() {
        let literal = HirItemKind::Literal { value: "v".into() };
        let node = HirItemKind::NodeConstruction {
            name: "custom-name".into(),
        };
        let ordered = HirItemKind::OrderedBlock;
        let expression = HirItemKind::Expression {
            source: "1 + 1".into(),
        };
        let macro_call = HirItemKind::MacroInvocation {
            name: "m".into(),
            arguments: Vec::new(),
        };
        let reference = HirItemKind::Reference {
            target: HirReference::Unresolved("t".into()),
        };
        let relation = HirItemKind::RelationConstruction {
            source: HirReference::Unresolved("a".into()),
            kind: "links".into(),
            target: HirReference::Unresolved("b".into()),
        };
        let attribute = HirItemKind::AttributeAssignment {
            owner: None,
            name: "k".into(),
            value: HirValue::String("v".into()),
        };

        assert_eq!(node_kind_name(&literal), Some("literal"));
        assert_eq!(node_kind_name(&node), Some("custom-name"));
        assert_eq!(node_kind_name(&ordered), Some("ordered-block"));
        assert_eq!(node_kind_name(&expression), Some("expression"));
        assert_eq!(node_kind_name(&macro_call), Some("macro-invocation"));
        assert_eq!(node_kind_name(&reference), None);
        assert_eq!(node_kind_name(&relation), None);

        assert_eq!(form_name(&literal), "literal");
        assert_eq!(form_name(&node), "node-construction");
        assert_eq!(form_name(&relation), "relation-construction");
        assert_eq!(form_name(&attribute), "attribute-assignment");
        assert_eq!(form_name(&ordered), "ordered-block");
        assert_eq!(form_name(&reference), "reference");
        assert_eq!(form_name(&expression), "expression");
        assert_eq!(form_name(&macro_call), "macro-invocation");
    }

    /// Kills both `payload_for` constant-return mutants: the payload must
    /// carry the item's actual kind and attributes, not a fixed string.
    #[test]
    fn derived_payload_carries_kind_and_attributes() {
        let kind = HirItemKind::Literal {
            value: "carried-value".into(),
        };
        let attributes =
            BTreeMap::from([("colour".to_owned(), HirValue::String("blue".to_owned()))]);
        let payload = payload_for(&kind, &attributes).expect("payload encodes");
        assert!(
            payload.contains("carried-value"),
            "payload dropped the literal value: {payload}"
        );
        assert!(
            payload.contains("colour") && payload.contains("blue"),
            "payload dropped attributes: {payload}"
        );
        // Distinct inputs must not collapse to one payload.
        let other = payload_for(
            &HirItemKind::Literal {
                value: "different".into(),
            },
            &BTreeMap::new(),
        )
        .expect("payload encodes");
        assert_ne!(payload, other, "payload_for ignores its input");
    }

    /// Kills `replace | with ^` in `kind_id`: the high bit marks derived
    /// kinds and must be set for every name, which `^` cannot guarantee.
    #[test]
    fn derived_kind_ids_always_set_the_derived_high_bit() {
        for name in [
            "literal",
            "node-construction",
            "ordered-block",
            "expression",
            "macro-invocation",
            "",
        ] {
            let KindId(raw) = kind_id(name);
            assert_eq!(
                raw & 0x8000_0000,
                0x8000_0000,
                "kind_id({name:?}) lost the derived high bit: {raw:#x}"
            );
        }
        assert_ne!(
            kind_id("literal"),
            kind_id("expression"),
            "distinct kind names must not collide"
        );
    }

    /// Kills `replace nearest_owner -> Option<NodeId> with None`: an item
    /// nested below a node-bearing ancestor must resolve to that ancestor.
    #[test]
    fn nearest_owner_walks_up_to_the_owning_node() {
        let source = SourceId::from_name("owner-walk");
        let owner_node = derived_node_id(source, "node-construction", &[0]);
        let parents = HashMap::from([(HirId(2), HirId(1)), (HirId(1), HirId(0))]);
        let node_for = BTreeMap::from([(HirId(0), owner_node)]);
        assert_eq!(
            nearest_owner(HirId(2), &parents, &node_for),
            Some(owner_node),
            "nearest_owner must walk transitively to the owning node"
        );
        assert_eq!(
            nearest_owner(HirId(0), &parents, &node_for),
            None,
            "a root with no node-bearing ancestor has no owner"
        );
    }

    /// Kills `replace || with &&` in `deserialize_debug_v1`'s framing check:
    /// a missing trailing LF and a doubled trailing LF must EACH be rejected.
    #[test]
    fn debug_json_framing_rejects_both_violations_independently() {
        let graph = graph_with_relations();
        let bytes = serialize_debug_v1(&graph).expect("serializes");
        deserialize_debug_v1(&bytes).expect("canonical framing decodes");

        let mut missing = bytes.clone();
        assert_eq!(missing.pop(), Some(b'\n'), "canonical form ends with LF");
        assert!(
            deserialize_debug_v1(&missing).is_err(),
            "a document with no trailing LF must be rejected"
        );

        let mut doubled = bytes.clone();
        doubled.push(b'\n');
        assert!(
            deserialize_debug_v1(&doubled).is_err(),
            "a document with two trailing LFs must be rejected"
        );
    }

    /// Kills `replace compare_shape -> Result<(), DebugJsonError> with Ok(())`
    /// and its `delete match arm` / `|| -> &&` variants.
    #[test]
    fn noncanonical_debug_json_is_rejected() {
        let graph = graph_with_relations();
        let bytes = serialize_debug_v1(&graph).expect("serializes");
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("parses");

        value
            .as_object_mut()
            .expect("top level is an object")
            .insert("unexpected".into(), serde_json::Value::Bool(true));
        let altered = serde_json::to_vec(&value).expect("re-encodes");
        assert!(
            deserialize_debug_v1(&altered).is_err(),
            "a document with an extra key must not decode as canonical"
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("parses");
        if let Some(relations) = value.get_mut("relations").and_then(|v| v.as_array_mut()) {
            relations.pop();
            let altered = serde_json::to_vec(&value).expect("re-encodes");
            assert!(
                deserialize_debug_v1(&altered).is_err()
                    || deserialize_debug_v1(&altered)
                        .is_ok_and(|decoded| decoded.relations.len() == 1),
                "a truncated relation array must be detected, not silently accepted as full"
            );
        }
    }
}
