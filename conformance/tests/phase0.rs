//! Phase 0 constitutional, conformance, and gate test skeletons.

use std::collections::BTreeSet;

use liminal_id::IdentityGrade;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepresentationFixture {
    version: u32,
    example: Vec<RepresentationExample>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepresentationExample {
    id: String,
    domain: ExampleDomain,
    node: Vec<FixtureNode>,
    relation: Vec<FixtureRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ExampleDomain {
    Prose,
    Code,
    Table,
    Image,
    AudioInterval,
    ExternalValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureNode {
    alias: String,
    role: String,
    kind: FixtureNodeKind,
    payload: String,
    expected_profile: ExpectedProfile,
    expected_holder: String,
    minimum_identity: IdentityGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum FixtureNodeKind {
    #[serde(rename = "FILE")]
    File,
    #[serde(rename = "PARAGRAPH")]
    Paragraph,
    #[serde(rename = "EXTERNAL_VALUE")]
    ExternalValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRelation {
    role: String,
    kind: FixtureRelationKind,
    from: String,
    to: String,
    payload: String,
    expected_profile: ExpectedProfile,
    expected_holder: String,
    minimum_identity: IdentityGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum FixtureRelationKind {
    #[serde(rename = "COMMENT")]
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ExpectedProfile {
    ExternalFile,
    GraphNative,
}

fn parse_representation_fixture(input: &str) -> Result<RepresentationFixture, String> {
    let fixture: RepresentationFixture =
        toml::from_str(input).map_err(|error| error.to_string())?;
    if fixture.version != 1 {
        return Err(format!(
            "unsupported representation fixture version {}",
            fixture.version
        ));
    }
    if fixture.example.is_empty() {
        return Err("fixture has no examples".into());
    }

    let mut example_ids = BTreeSet::new();
    for example in &fixture.example {
        if example.id.is_empty() || !example_ids.insert(example.id.as_str()) {
            return Err(format!("empty or duplicate example id {:?}", example.id));
        }
        if example.node.is_empty() {
            return Err(format!("example {:?} has no nodes", example.id));
        }

        let mut aliases = BTreeSet::new();
        for node in &example.node {
            if node.alias.is_empty() || !aliases.insert(node.alias.as_str()) {
                return Err(format!("empty or duplicate node alias {:?}", node.alias));
            }
            validate_holder(node.expected_profile, &node.expected_holder)?;
        }
        for relation in &example.relation {
            if !aliases.contains(relation.from.as_str()) || !aliases.contains(relation.to.as_str())
            {
                return Err(format!(
                    "relation {:?} has dangling endpoints {:?} -> {:?}",
                    relation.role, relation.from, relation.to
                ));
            }
            validate_holder(relation.expected_profile, &relation.expected_holder)?;
        }
    }

    Ok(fixture)
}

fn validate_holder(profile: ExpectedProfile, holder: &str) -> Result<(), String> {
    let valid = match profile {
        ExpectedProfile::ExternalFile => holder
            .strip_prefix("file:")
            .is_some_and(|path| !path.is_empty()),
        ExpectedProfile::GraphNative => holder == "graph",
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "non-concrete or profile-inconsistent Holder {holder:?}"
        ))
    }
}

fn validate_example_topologies(fixture: &RepresentationFixture) -> Result<(), String> {
    let domains = [
        ExampleDomain::Prose,
        ExampleDomain::Code,
        ExampleDomain::Table,
        ExampleDomain::Image,
        ExampleDomain::AudioInterval,
        ExampleDomain::ExternalValue,
    ];
    if fixture.example.len() != domains.len() {
        return Err(format!(
            "expected six examples, got {}",
            fixture.example.len()
        ));
    }

    for (example, expected_domain) in fixture.example.iter().zip(domains) {
        if example.domain != expected_domain {
            return Err(format!(
                "example {:?} is out of frozen domain order",
                example.id
            ));
        }
        match example.domain {
            ExampleDomain::Prose => validate_prose_topology(example)?,
            ExampleDomain::Code => validate_code_topology(example)?,
            ExampleDomain::Table => validate_table_topology(example)?,
            ExampleDomain::Image => validate_image_topology(example)?,
            ExampleDomain::AudioInterval => validate_audio_topology(example)?,
            ExampleDomain::ExternalValue => validate_external_value_topology(example)?,
        }
    }
    Ok(())
}

fn validate_prose_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            ("document", "document", FixtureNodeKind::File),
            ("paragraph", "paragraph", FixtureNodeKind::Paragraph),
        ],
    )?;
    exact_relations(example, &[("contains", "document", "paragraph")])?;
    nonempty_payload(example, "paragraph")
}

fn validate_code_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            ("document", "document", FixtureNodeKind::File),
            ("code-block", "code-block", FixtureNodeKind::Paragraph),
        ],
    )?;
    exact_relations(example, &[("contains", "document", "code-block")])?;
    let value = canonical_payload(example, "code-block")?;
    require_nonempty_string(&value, "text")?;
    if value.get("language").and_then(serde_json::Value::as_str) == Some("rust") {
        Ok(())
    } else {
        Err("code language must be rust".into())
    }
}

fn validate_table_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            ("document", "document", FixtureNodeKind::File),
            ("table", "table", FixtureNodeKind::Paragraph),
            ("row-1", "row", FixtureNodeKind::Paragraph),
            ("row-2", "row", FixtureNodeKind::Paragraph),
            ("cell-1-1", "cell", FixtureNodeKind::Paragraph),
            ("cell-1-2", "cell", FixtureNodeKind::Paragraph),
            ("cell-2-1", "cell", FixtureNodeKind::Paragraph),
            ("cell-2-2", "cell", FixtureNodeKind::Paragraph),
        ],
    )?;
    exact_relations(
        example,
        &[
            ("contains", "document", "table"),
            ("contains", "table", "row-1"),
            ("contains", "table", "row-2"),
            ("contains", "row-1", "cell-1-1"),
            ("contains", "row-1", "cell-1-2"),
            ("contains", "row-2", "cell-2-1"),
            ("contains", "row-2", "cell-2-2"),
            ("next", "row-1", "row-2"),
        ],
    )?;
    for alias in ["cell-1-1", "cell-1-2", "cell-2-1", "cell-2-2"] {
        nonempty_payload(example, alias)?;
    }
    Ok(())
}

fn validate_image_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            ("document", "document", FixtureNodeKind::File),
            (
                "image-reference",
                "image-reference",
                FixtureNodeKind::Paragraph,
            ),
        ],
    )?;
    exact_relations(example, &[("embeds", "document", "image-reference")])?;
    let value = canonical_payload(example, "image-reference")?;
    require_sha256(&value, "sha256")?;
    require_nonempty_string(&value, "mime")?;
    require_nonempty_string(&value, "alt")
}

fn validate_audio_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            ("document", "document", FixtureNodeKind::File),
            (
                "audio-interval",
                "audio-interval",
                FixtureNodeKind::Paragraph,
            ),
        ],
    )?;
    exact_relations(example, &[("contains", "document", "audio-interval")])?;
    let value = canonical_payload(example, "audio-interval")?;
    require_sha256(&value, "sha256")?;
    let start = value
        .get("start_ms")
        .and_then(serde_json::Value::as_u64)
        .ok_or("audio start_ms must be unsigned")?;
    let end = value
        .get("end_ms")
        .and_then(serde_json::Value::as_u64)
        .ok_or("audio end_ms must be unsigned")?;
    if start < end {
        Ok(())
    } else {
        Err("audio interval must have start_ms < end_ms".into())
    }
}

fn validate_external_value_topology(example: &RepresentationExample) -> Result<(), String> {
    exact_nodes(
        example,
        &[
            (
                "external-source",
                "external-source",
                FixtureNodeKind::ExternalValue,
            ),
            ("value", "value", FixtureNodeKind::ExternalValue),
        ],
    )?;
    exact_relations(example, &[("materializes", "external-source", "value")])?;
    for alias in ["external-source", "value"] {
        let value = canonical_payload(example, alias)?;
        for field in ["source_id", "revision", "observed_at", "value"] {
            require_nonempty_string(&value, field)?;
        }
    }
    Ok(())
}

fn exact_nodes(
    example: &RepresentationExample,
    expected: &[(&str, &str, FixtureNodeKind)],
) -> Result<(), String> {
    let mut actual = example
        .node
        .iter()
        .map(|node| (node.alias.as_str(), node.role.as_str(), node.kind))
        .collect::<Vec<_>>();
    let mut expected = expected.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("node topology mismatch for {:?}", example.id))
    }
}

fn exact_relations(
    example: &RepresentationExample,
    expected: &[(&str, &str, &str)],
) -> Result<(), String> {
    let mut actual = Vec::new();
    for relation in &example.relation {
        if relation.kind != FixtureRelationKind::Comment || relation.payload != relation.role {
            return Err(format!("relation lowering mismatch for {:?}", example.id));
        }
        actual.push((
            relation.role.as_str(),
            relation.from.as_str(),
            relation.to.as_str(),
        ));
    }
    let mut expected = expected.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("relation topology mismatch for {:?}", example.id))
    }
}

fn node<'a>(example: &'a RepresentationExample, alias: &str) -> Result<&'a FixtureNode, String> {
    example
        .node
        .iter()
        .find(|node| node.alias == alias)
        .ok_or_else(|| format!("missing node {alias:?}"))
}

fn nonempty_payload(example: &RepresentationExample, alias: &str) -> Result<(), String> {
    if node(example, alias)?.payload.is_empty() {
        Err(format!("node {alias:?} has empty payload"))
    } else {
        Ok(())
    }
}

fn canonical_payload(
    example: &RepresentationExample,
    alias: &str,
) -> Result<serde_json::Value, String> {
    let payload = &node(example, alias)?.payload;
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    if serde_json::to_string(&value).map_err(|error| error.to_string())? != *payload {
        return Err(format!("node {alias:?} payload is not canonical JSON"));
    }
    if !value.is_object() {
        return Err(format!("node {alias:?} payload must be a JSON object"));
    }
    Ok(value)
}

fn require_nonempty_string(value: &serde_json::Value, field: &str) -> Result<(), String> {
    if value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty())
    {
        Ok(())
    } else {
        Err(format!("{field} must be a non-empty string"))
    }
}

fn require_sha256(value: &serde_json::Value, field: &str) -> Result<(), String> {
    let hash = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))?;
    if hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!(
            "{field} must be 64 lowercase hexadecimal characters"
        ))
    }
}

fn production_rust_sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let mut pending = vec![root.join("crates")];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).expect("read production source tree") {
            let entry = entry.expect("read production source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
}

fn materialize_and_assert_governance(example: &RepresentationExample) -> Result<(), String> {
    use liminal_graph::{
        GraphStore, Node, NodeFlags, Operation, Origin, PayloadRef, Relation, RelationFlags,
        Target, TxnMeta, kind,
    };
    use liminal_id::{JurisdictionSubject, NodeId, RelationId, RevisionId, Timestamp};

    static NEXT_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = phase0_temp_dir(sequence)?;
    let _ = std::fs::remove_dir_all(&dir);
    let store = GraphStore::open(&dir).map_err(|error| error.to_string())?;
    let mut txn = store.begin().map_err(|error| error.to_string())?;
    let mut aliases = std::collections::BTreeMap::new();
    let mut expected = std::collections::BTreeMap::new();

    if let Some(holder) = example
        .node
        .iter()
        .find(|node| node.expected_profile == ExpectedProfile::ExternalFile)
        .map(|node| node.expected_holder.as_str())
    {
        let path = holder
            .strip_prefix("file:")
            .ok_or("external-file node lacks file Holder")?;
        txn.put_aux(
            liminal_graph::ns::SYS_BLOB,
            &format!("file/{path}"),
            serde_json::json!("fixture"),
        )
        .map_err(|error| error.to_string())?;
    }

    for fixture_node in &example.node {
        let id = NodeId::new();
        aliases.insert(fixture_node.alias.as_str(), id);
        expected.insert(
            JurisdictionSubject::Node(id),
            (
                fixture_node.expected_profile,
                fixture_node.expected_holder.as_str(),
                fixture_node.minimum_identity,
            ),
        );
        let node_kind = match fixture_node.kind {
            FixtureNodeKind::File => kind::FILE,
            FixtureNodeKind::Paragraph => kind::PARAGRAPH,
            FixtureNodeKind::ExternalValue => kind::EXTERNAL_VALUE,
        };
        txn.apply(Operation::CreateNode {
            node: Node {
                id,
                kind: node_kind,
                payload: PayloadRef::Text(fixture_node.payload.clone()),
                revision: RevisionId(0),
                flags: NodeFlags::default(),
            },
        })
        .map_err(|error| error.to_string())?;
    }

    for fixture_relation in &example.relation {
        let id = RelationId::new();
        expected.insert(
            JurisdictionSubject::Relation(id),
            (
                fixture_relation.expected_profile,
                fixture_relation.expected_holder.as_str(),
                fixture_relation.minimum_identity,
            ),
        );
        let source = *aliases
            .get(fixture_relation.from.as_str())
            .ok_or("validated relation source disappeared")?;
        let target = *aliases
            .get(fixture_relation.to.as_str())
            .ok_or("validated relation target disappeared")?;
        txn.apply(Operation::AddRelation {
            relation: Relation {
                id,
                source,
                target: Target::Node(target),
                kind: kind::COMMENT,
                payload: PayloadRef::Text(fixture_relation.payload.clone()),
                revision: RevisionId(0),
                flags: RelationFlags::default(),
                requires: None,
            },
        })
        .map_err(|error| error.to_string())?;
    }

    let (_, transaction) = txn
        .commit(TxnMeta {
            actor: None,
            origin: Origin::Human,
            at: Timestamp::now(),
            provenance: Some(format!("Phase 0 M13 fixture {}", example.id)),
            inverse: None,
        })
        .map_err(|error| error.to_string())?;

    assert_governed_subjects(example, &store, transaction, &expected)?;
    drop(store);
    std::fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(())
}

fn phase0_temp_dir(sequence: u64) -> Result<camino::Utf8PathBuf, String> {
    Ok(
        camino::Utf8PathBuf::from(std::env::temp_dir().to_str().ok_or("non-UTF-8 temp dir")?).join(
            format!("liminal-phase0-m13-{}-{sequence}", std::process::id()),
        ),
    )
}

fn assert_governed_subjects(
    example: &RepresentationExample,
    store: &liminal_graph::GraphStore,
    transaction: liminal_id::TransactionId,
    expected: &std::collections::BTreeMap<
        liminal_id::JurisdictionSubject,
        (ExpectedProfile, &str, IdentityGrade),
    >,
) -> Result<(), String> {
    use liminal_jurisdiction::{Checker, Holder, ProfileSet};
    use liminal_revision::{BasisPerspective, WorkspaceBasis};

    let profiles = ProfileSet::phase_minus_1();
    let checker = Checker {
        store,
        profiles: &profiles,
    };
    let basis = WorkspaceBasis {
        transaction,
        perspective: BasisPerspective::DurableOnly,
        components: std::collections::BTreeMap::new(),
    };
    let subjects = checker.subjects().map_err(|error| error.to_string())?;
    if subjects.len() != expected.len() {
        return Err(format!(
            "materialized subject count {} differs from expected {}",
            subjects.len(),
            expected.len()
        ));
    }
    for subject in subjects {
        let (expected_profile, expected_holder, minimum_identity) = expected
            .get(&subject)
            .copied()
            .ok_or_else(|| format!("unexpected subject {subject}"))?;
        let profile = profiles
            .for_subject(subject, store)
            .ok_or_else(|| format!("subject {subject} is Ungoverned"))?;
        let profile_name = match expected_profile {
            ExpectedProfile::ExternalFile => "external-file",
            ExpectedProfile::GraphNative => "graph-native",
        };
        if profile.id().0 != profile_name {
            return Err(format!("subject {subject} resolved to wrong profile"));
        }
        let holder = checker
            .resolve_holder(subject, &basis)
            .map_err(|error| error.to_string())?;
        let declared_holder = if expected_holder == "graph" {
            Holder::Graph
        } else {
            Holder::File {
                path: liminal_id::PathId(
                    expected_holder
                        .strip_prefix("file:")
                        .ok_or("invalid expected file Holder")?
                        .into(),
                ),
            }
        };
        if holder != declared_holder {
            return Err(format!("subject {subject} resolved to wrong Holder"));
        }
        let actual_grade = liminal_jurisdiction::profile::grade_of(subject, store);
        if !actual_grade.satisfies(minimum_identity) {
            return Err(format!(
                "subject {subject} grade {actual_grade:?} is below {minimum_identity:?}"
            ));
        }
    }
    if store.nodes().map_err(|error| error.to_string())?.len() != example.node.len()
        || store.relations().map_err(|error| error.to_string())?.len() != example.relation.len()
    {
        return Err("materialized graph contains a non-fixture subject".into());
    }

    Ok(())
}

/// Rejects duplicate IDs or aliases, dangling endpoints, empty node sets, unknown
/// enum values, and unknown fields as independent malformed-fixture cases.
/// Fails closed unless every example has one concrete Holder and known grade.
#[test]
fn representation_fixture_rejects_malformed_cases() {
    let valid = include_str!("../fixtures/phase0/representation-examples.toml");
    assert!(parse_representation_fixture(valid).is_ok());

    let malformed = [
        valid.replacen("id = \"code\"", "id = \"prose\"", 1),
        valid.replacen("alias = \"paragraph\"", "alias = \"document\"", 1),
        valid.replacen("to = \"paragraph\"", "to = \"missing\"", 1),
        valid.replacen("[[example.node]]", "[[example.absent_node]]", 2),
        valid.replacen("domain = \"prose\"", "domain = \"unknown\"", 1),
        valid.replacen(
            "minimum_identity = \"anchored\"",
            "minimum_identity = \"mythic\"",
            1,
        ),
        valid.replacen(
            "expected_profile = \"external-file\"",
            "expected_profile = \"mystery\"",
            1,
        ),
        valid.replacen("kind = \"FILE\"", "kind = \"FACET\"", 1),
        valid.replacen(
            "role = \"document\"",
            "role = \"document\"\nunknown = true",
            1,
        ),
    ];

    for input in malformed {
        assert!(parse_representation_fixture(&input).is_err());
    }
}

/// Materializes the six exact M13 topology/multiplicity templates and independently
/// scans production Rust for split prohibited facet spellings; only Nodes and
/// Relations may remain. Domain-label-only or topology-substituted fixtures fail.
#[test]
fn kernel_represents_six_examples_without_new_primitive() {
    let fixture = parse_representation_fixture(include_str!(
        "../fixtures/phase0/representation-examples.toml"
    ))
    .unwrap();
    validate_example_topologies(&fixture).unwrap();
    for example in &fixture.example {
        materialize_and_assert_governance(example).unwrap();
    }

    let mut domain_label_substitution = fixture.clone();
    domain_label_substitution.example[1].node = fixture.example[0].node.clone();
    domain_label_substitution.example[1].relation = fixture.example[0].relation.clone();
    assert!(validate_example_topologies(&domain_label_substitution).is_err());

    let prohibited = ["Facet", "Id"].concat();
    let prohibited_snake = ["facet", "_id"].concat();
    for source in production_rust_sources() {
        let text = std::fs::read_to_string(&source).unwrap();
        assert!(
            !text.contains(&prohibited) && !text.contains(&prohibited_snake),
            "third semantic primitive spelling found in {}",
            source.display()
        );
    }
}

/// Enumerates materialized subjects through `Checker::subjects` and compares
/// profile dispatch against each fixture's declared Holder and minimum grade.
/// Fails closed on Ungoverned, implicit identity, or a grade below declaration.
#[test]
fn every_phase0_example_resolves_with_declared_identity() {
    let fixture = parse_representation_fixture(include_str!(
        "../fixtures/phase0/representation-examples.toml"
    ))
    .unwrap();
    for example in &fixture.example {
        materialize_and_assert_governance(example).unwrap();
    }
}

fn published_shape_anchors() -> Vec<(&'static str, String)> {
    vec![
        ("K-RPR-01", repair_decision_shape()),
        ("K-RPR-02", repair_record_fields()),
        ("K-RPR-03", safety_requirement_shape()),
        ("K-ILRP-01", intent_state_shape()),
        ("K-ILRP-02", prestate_match_shape()),
        ("K-ILRP-03", crash_boundary_shape()),
        ("K-BAS-01", basis_perspective_shape()),
        ("K-BAS-02", basis_component_shape()),
        ("K-BAS-03", json_fields(&sample_basis())),
        ("K-OVL-01", overlay_state_shape()),
        ("K-OVL-02", reconciliation_item_fields()),
    ]
}

fn variant_name(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
        .split([' ', '{', '('])
        .next()
        .expect("Debug variant name")
        .to_owned()
}

fn variant_shape(values: &[impl std::fmt::Debug]) -> String {
    values
        .iter()
        .map(variant_name)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn repair_decision_shape() -> String {
    use liminal_jurisdiction::{RepairDecision, ReviewReason, SafetyEvidence};
    variant_shape(&[
        RepairDecision::AutoApply {
            evidence: SafetyEvidence::StructurallyDisjoint {
                description: String::new(),
            },
        },
        RepairDecision::NeedsReview {
            reasons: vec![ReviewReason(String::new())],
        },
    ])
}

fn safety_requirement_shape() -> String {
    use liminal_jurisdiction::SafetyRequirement;
    variant_shape(&[
        SafetyRequirement::StructuralDisjointness,
        SafetyRequirement::DomainValidator(String::new()),
        SafetyRequirement::HumanApproval,
    ])
}

fn intent_state_shape() -> String {
    use liminal_jurisdiction::IntentState::*;
    variant_shape(&[
        Prepared,
        Applying,
        ExternalApplied,
        Finalizing,
        Committed,
        NeedsReview,
        Aborted,
    ])
}

fn prestate_match_shape() -> String {
    use liminal_jurisdiction::PrestateMatch::*;
    variant_shape(&[Prestate, Poststate, Neither])
}

fn crash_boundary_shape() -> String {
    liminal_jurisdiction::CrashPoint::all()
        .iter()
        .map(|point| point.name())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn basis_perspective_shape() -> String {
    use liminal_revision::{BasisPerspective, CausalFrontier};
    variant_shape(&[
        BasisPerspective::ClientScoped {
            client: liminal_id::ClientId::new(),
        },
        BasisPerspective::DurableOnly,
        BasisPerspective::Published {
            revision: liminal_id::PublicationId::new(),
        },
        BasisPerspective::Federated {
            domain: liminal_id::FederationId::new(),
            frontier: CausalFrontier(Vec::new()),
        },
    ])
}

fn basis_component_shape() -> String {
    use liminal_id::{
        BufferId, ClientId, GraphRevisionId, ObjectId, PathId, RevisionToken, SessionEpoch,
        SourceId, Timestamp,
    };
    use liminal_revision::BasisComponent;
    let hash = liminal_id::ContentHash::of(b"shape");
    variant_shape(&[
        BasisComponent::BufferGeneration {
            client: ClientId::new(),
            buffer: BufferId::new(),
            epoch: SessionEpoch(0),
            generation: 0,
            content_hash: None,
            base_file_hash: None,
        },
        BasisComponent::FileContent {
            path: PathId("shape".into()),
            hash,
        },
        BasisComponent::GitCommit {
            oid: ObjectId(String::new()),
        },
        BasisComponent::GraphSnapshot {
            revision: GraphRevisionId(0),
        },
        BasisComponent::ObjectContent { hash },
        BasisComponent::ExternalRevision {
            source: SourceId::new(),
            token: RevisionToken(String::new()),
        },
        BasisComponent::Observation {
            source: SourceId::new(),
            observed_at: Timestamp(0),
            hash,
        },
    ])
}

fn overlay_state_shape() -> String {
    use liminal_jurisdiction::OverlayState;
    variant_shape(&[
        OverlayState::Active,
        OverlayState::RepairProposed(liminal_id::RepairId::new()),
        OverlayState::Contested,
        OverlayState::Archived,
        OverlayState::Discarded,
    ])
}

fn sample_basis() -> liminal_revision::WorkspaceBasis {
    liminal_revision::WorkspaceBasis {
        transaction: liminal_id::TransactionId::new(),
        perspective: liminal_revision::BasisPerspective::DurableOnly,
        components: std::collections::BTreeMap::new(),
    }
}

fn repair_record_fields() -> String {
    let basis = sample_basis();
    let record = liminal_jurisdiction::RepairRecord {
        repair: liminal_id::RepairId::new(),
        input_basis: basis.clone(),
        selected_rule: String::new(),
        evidence: liminal_jurisdiction::SafetyEvidence::StructurallyDisjoint {
            description: String::new(),
        },
        applied_steps: Vec::new(),
        resulting_basis: basis,
        inverse: None,
    };
    json_fields(&record)
}

fn reconciliation_item_fields() -> String {
    let item = liminal_jurisdiction::ReconciliationItem {
        id: liminal_id::ReconciliationItemId::new(),
        root_cause: String::new(),
        subjects: Vec::new(),
        overlays: Vec::new(),
        repairs: Vec::new(),
        created_at: liminal_id::Timestamp(0),
        status: liminal_jurisdiction::ReconciliationStatus::Pending,
    };
    json_fields(&item)
}

fn json_fields(value: &impl serde::Serialize) -> String {
    let value = serde_json::to_value(value).expect("serialize live shape");
    let mut fields = value
        .as_object()
        .expect("live shape serializes as object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    fields.sort();
    fields.join(" | ")
}

fn assert_derived_crash_inventory() -> Result<(), String> {
    use liminal_conformance::harness::{ToyRun, runnable_crash_scenarios};

    let scenarios = runnable_crash_scenarios().map_err(|error| error.to_string())?;
    if scenarios.is_empty() {
        return Err("no runnable crash scenarios".into());
    }
    let mut observed_boundaries = BTreeSet::new();
    for scenario in &scenarios {
        let run = ToyRun::new(&format!("phase0-inventory-{}", scenario.scenario.id))
            .map_err(|error| error.to_string())?;
        let trace = run.baseline(scenario).map_err(|error| error.to_string())?;
        let derived = trace.enumerate_faults();
        if derived.is_empty() || derived != trace.hits {
            return Err(format!(
                "scenario {} did not derive every observed crash case",
                scenario.scenario.id
            ));
        }
        let unique = derived.iter().cloned().collect::<BTreeSet<_>>();
        if unique.len() != derived.len() {
            return Err(format!(
                "scenario {} repeated a boundary occurrence",
                scenario.scenario.id
            ));
        }
        observed_boundaries.extend(derived.into_iter().map(|(name, _)| name));
        std::fs::remove_dir_all(&run.root).map_err(|error| error.to_string())?;

        ToyRun::crash_matrix(scenario).map_err(|error| error.to_string())?;
    }

    let registered = liminal_jurisdiction::CrashPoint::all()
        .iter()
        .map(|point| point.name().to_owned())
        .collect::<BTreeSet<_>>();
    if observed_boundaries != registered {
        return Err(format!(
            "derived boundaries {observed_boundaries:?} differ from registered {registered:?}"
        ));
    }
    Ok(())
}

/// Compares every normative repair, ILRP, Overlay, and Basis table row with live
/// serde examples and enum debug names as an independent shape oracle.
/// Fails closed on a missing, duplicated, or live-shape-divergent table anchor.
#[test]
fn published_contract_matches_live_repair_and_basis_shapes() {
    let kernel = include_str!("../../spec/kernel.md");
    let normalized_kernel = kernel.split_whitespace().collect::<Vec<_>>().join(" ");
    let anchors = published_shape_anchors();
    for (anchor, live_shape) in anchors {
        assert_eq!(
            kernel.match_indices(anchor).count(),
            1,
            "normative anchor {anchor:?} must occur exactly once"
        );
        assert!(
            normalized_kernel.contains(&live_shape),
            "normative anchor {anchor:?} lacks live shape {live_shape:?}"
        );
    }
    assert_intent_transition_table();
}

fn assert_intent_transition_table() {
    use liminal_jurisdiction::IntentState::{
        Aborted, Applying, Committed, ExternalApplied, Finalizing, NeedsReview, Prepared,
    };
    let states = [
        Prepared,
        Applying,
        ExternalApplied,
        Finalizing,
        Committed,
        NeedsReview,
        Aborted,
    ];
    let permitted = [
        (Prepared, Applying),
        (Prepared, NeedsReview),
        (Prepared, Aborted),
        (Applying, ExternalApplied),
        (Applying, NeedsReview),
        (Applying, Aborted),
        (ExternalApplied, Finalizing),
        (ExternalApplied, NeedsReview),
        (Finalizing, Committed),
        (Finalizing, NeedsReview),
    ];

    for current in states {
        for next in states {
            assert_eq!(
                current.may_transition_to(next),
                permitted.contains(&(current, next)),
                "published ILRP transition table drifted at {current:?} -> {next:?}"
            );
        }
        assert_eq!(
            current.is_terminal(),
            matches!(current, Committed | NeedsReview | Aborted),
            "published ILRP terminal set drifted at {current:?}"
        );
    }
}

/// Derives crash cases from non-empty `ToyRun::baseline` hits and checks the
/// independent `crash_matrix` covers every `(point, occurrence)` exactly.
/// Fails closed on empty terminals, unknown boundaries, or non-idempotent recovery.
#[test]
fn ilrp_fixture_inventory_covers_every_durable_boundary() {
    assert_derived_crash_inventory().unwrap();
}

/// Serializes live `TransformContract` values and checks all eight schema fields
/// against an independent byte-canonical JSON round trip.
/// Fails closed on a missing field, renamed field, type drift, or unstable bytes.
#[test]
#[ignore = "Phase 0 M15: transform contract schema"]
fn transform_contract_schema_roundtrips_all_fields() {
    unimplemented!("round-trip every transform contract field");
}

/// Exercises unknown, missing, duplicate, empty, and wrong-typed fields as
/// independent malformed schema cases under `additionalProperties: false`.
/// Fails closed if any malformed value deserializes or validates successfully.
#[test]
#[ignore = "Phase 0 M15: malformed transform fixtures"]
fn transform_contract_schema_rejects_malformed_cases() {
    unimplemented!("reject malformed transform contract cases");
}

/// Audits normative identity and projection claims against accepted ADR values
/// and independently regenerated Phase -1 measurements without blessing them.
/// Fails closed if any grade or level differs from, or exceeds, its evidence.
#[test]
#[ignore = "Phase 0 M15: identity and projection publication"]
fn projection_laws_and_identity_ceilings_are_frozen() {
    unimplemented!("freeze projection laws and identity ceilings");
}

/// Requires one accepted, explicit ADR decision for each §119, §121, §122, and
/// §125 boundary before any parser, engine, or compiled-plan implementation.
/// Fails closed on a missing, proposed, rejected, duplicated, or ambiguous choice.
#[test]
#[ignore = "Phase 0 M15: accepted boundary ADRs"]
fn phase1_boundary_adrs_are_accepted_and_unambiguous() {
    unimplemented!("require accepted unambiguous Phase 1 boundary ADRs");
}

/// Conjoins byte-empty diagnostics, zero unexpected items, zero manual Contract
/// authoring, and no hidden transient debt beyond the policy window.
/// Fails closed when any conjunct emits, persists, or requires manual intervention.
#[test]
#[ignore = "Phase 0 M16: conjunctive sound-session gate"]
fn sound_session_zero_diagnostics_items_and_authoring() {
    unimplemented!("prove sound sessions emit no diagnostics items or authoring");
}

/// Compares documented public and sanitized fixture families with tracked paths
/// outside locked trees, requiring a version, parser, owner, and confidentiality.
/// Fails closed on undocumented, unowned, versionless, or locked-tree inventory.
#[test]
#[ignore = "Phase 0 M16: governed fixture inventory"]
fn phase0_fixture_inventory_is_versioned_and_owned() {
    unimplemented!("require versioned owned Phase 0 fixtures");
}

/// Checks every named trust boundary has assets, attacker, entry point, failure,
/// controls, detection, recovery, verification, residual risk, and owner.
/// Fails closed on any missing boundary or unsupported mitigation claim.
#[test]
#[ignore = "Phase 0 M16: published threat model"]
fn phase0_threat_model_covers_every_trust_boundary() {
    unimplemented!("cover every Phase 0 trust boundary");
}

/// Compares the complete v4 §118A reference inventory with the accepted ADR,
/// requiring one Adopt, Adapt, Defer, or Reject disposition and falsifier each.
/// Fails closed on an omitted, duplicated, unaccepted, or unfalsifiable entry.
#[test]
#[ignore = "Phase 0 M16: accepted prior-art ADR"]
fn prior_art_dispositions_are_accepted() {
    unimplemented!("require accepted prior-art dispositions");
}

/// Exercises frozen positive, negative, malformed, and panic-capture cases while
/// comparing incremental recovery with an independent full-reparse oracle.
/// Fails closed on panic, graph/loss divergence, boundary loss, or bad undo.
#[test]
#[ignore = "Phase 0 M17: real-CST spike oracle"]
fn real_cst_spike_oracle_handles_synthetic_cases() {
    unimplemented!("exercise the real-CST oracle with synthetic cases");
}

/// Recomputes the sorted real-CST results, counts, Wilson interval, inventory hash,
/// and provenance against the reviewed golden with only elapsed time redacted.
/// Fails closed on missing cases, changed seeds, unsorted rows, or byte drift.
#[test]
#[ignore = "Phase 0 M17: reviewed real-CST golden"]
fn real_cst_anchor_recovery_report_matches_golden() {
    unimplemented!("match real-CST anchor recovery to its golden");
}

/// Independently derives the supported level from every D17.3 measurement
/// conjunct and compares it with the report's declared capability.
/// Fails closed to Level 2 unless the ≥95% bar and every other L3 condition pass.
#[test]
#[ignore = "Phase 0 M17: measured capability report"]
fn real_cst_declared_level_never_exceeds_measurement() {
    unimplemented!("bound the declared real-CST level by measurement");
}

/// Checks the template-derived Phase 1 inventory and coverage matrices against
/// two independent adversarial-pass records and their verification dispositions.
/// Fails closed unless findings are resolved and suite status remains `proposed`.
#[test]
#[ignore = "Phase 0 M17: proposed Phase 1 suite packet"]
fn phase1_suite_packet_is_complete_and_unratified() {
    unimplemented!("require a complete unratified Phase 1 suite packet");
}

/// Requires an accepted ADR with exactly one explicit user GO or NO-GO decision
/// and a complete Phase 0 results table; green CI is not an independent decision.
/// Fails closed on absent authority, ambiguous verdicts, or incomplete evidence.
#[test]
#[ignore = "Phase 0 M17: final Phase 0 decision"]
fn phase0_go_no_go_adr_recorded() {
    unimplemented!("require the Phase 0 go-no-go ADR");
}
