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

    let empty_nodes = r#"
version = 1

[[example]]
id = "empty"
domain = "prose"
node = []
relation = []
"#;
    let empty_nodes_error = parse_representation_fixture(empty_nodes)
        .expect_err("an example with no nodes must fail its explicit invariant");
    assert!(
        empty_nodes_error.contains("has no nodes"),
        "empty-node case hit wrong rejection path: {empty_nodes_error}"
    );

    let malformed = [
        valid.replacen("id = \"code\"", "id = \"prose\"", 1),
        valid.replacen("alias = \"paragraph\"", "alias = \"document\"", 1),
        valid.replacen("to = \"paragraph\"", "to = \"missing\"", 1),
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformExamples {
    version: u32,
    examples: Vec<TransformExample>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformExample {
    id: String,
    accepted: bool,
    contract: serde_json::Value,
}

const TRANSFORM_ARRAY_FIELDS: [&str; 5] = [
    "requires",
    "preserves",
    "introduces",
    "may_discard",
    "required_capabilities",
];
const TRANSFORM_BOOL_FIELDS: [&str; 3] = ["is_deterministic", "is_reversible", "has_effects"];

fn parse_transform_examples(
    input: &str,
) -> Result<Vec<liminal_transform::TransformContract>, String> {
    let examples: TransformExamples =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    if examples.version != 1 || examples.examples.len() < 2 {
        return Err("transform examples require version 1 and at least two cases".into());
    }
    let mut ids = BTreeSet::new();
    let mut contracts = Vec::new();
    for example in examples.examples {
        if example.id.is_empty() || !ids.insert(example.id.clone()) {
            return Err(format!(
                "empty or duplicate transform example id {:?}",
                example.id
            ));
        }
        validate_transform_contract(&example.contract)?;
        let contract: liminal_transform::TransformContract =
            serde_json::from_value(example.contract).map_err(|error| error.to_string())?;
        if !contract.may_discard.is_empty() && !example.accepted {
            return Err(format!(
                "destructive example {:?} lacks acceptance",
                example.id
            ));
        }
        contracts.push(contract);
    }
    Ok(contracts)
}

fn validate_transform_contract(value: &serde_json::Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("TransformContract must be an object")?;
    let expected = TRANSFORM_ARRAY_FIELDS
        .into_iter()
        .chain(TRANSFORM_BOOL_FIELDS)
        .collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "TransformContract fields {actual:?} differ from {expected:?}"
        ));
    }
    for field in TRANSFORM_ARRAY_FIELDS {
        let values = object[field]
            .as_array()
            .ok_or_else(|| format!("{field} must be an array"))?;
        let mut unique = BTreeSet::new();
        for value in values {
            let text = value
                .as_str()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| format!("{field} values must be non-empty strings"))?;
            if !unique.insert(text) {
                return Err(format!("{field} values must be unique"));
            }
        }
    }
    for field in TRANSFORM_BOOL_FIELDS {
        if !object[field].is_boolean() {
            return Err(format!("{field} must be boolean"));
        }
    }
    Ok(())
}

fn assert_transform_schema_shape(input: &str) -> Result<(), String> {
    let schema: serde_json::Value =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    if schema["$schema"] != "https://json-schema.org/draft/2020-12/schema"
        || schema["type"] != "object"
        || schema["additionalProperties"] != false
    {
        return Err("transform schema header is not strict draft 2020-12".into());
    }
    let required = schema["required"]
        .as_array()
        .ok_or("schema required must be an array")?
        .iter()
        .map(|value| value.as_str().ok_or("required value must be string"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = TRANSFORM_ARRAY_FIELDS
        .into_iter()
        .chain(TRANSFORM_BOOL_FIELDS)
        .collect::<BTreeSet<_>>();
    if required != expected {
        return Err("schema required set differs from eight live fields".into());
    }
    for field in TRANSFORM_ARRAY_FIELDS {
        let property = &schema["properties"][field];
        if property["type"] != "array"
            || property["uniqueItems"] != true
            || property["items"]["type"] != "string"
            || property["items"]["minLength"] != 1
        {
            return Err(format!("schema array property {field} is not strict"));
        }
    }
    for field in TRANSFORM_BOOL_FIELDS {
        if schema["properties"][field]["type"] != "boolean" {
            return Err(format!("schema boolean property {field} drifted"));
        }
    }
    Ok(())
}

fn malformed_transform_contracts() -> Vec<(&'static str, serde_json::Value)> {
    let base = serde_json::json!({
        "requires": ["resolved-prose"],
        "preserves": ["text"],
        "introduces": [],
        "may_discard": [],
        "is_deterministic": true,
        "is_reversible": true,
        "has_effects": false,
        "required_capabilities": []
    });
    let mut unknown = base.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("surprise".into(), serde_json::Value::Bool(true));
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("preserves");
    let mut duplicate = base.clone();
    duplicate["requires"] = serde_json::json!(["resolved-prose", "resolved-prose"]);
    let mut empty = base.clone();
    empty["preserves"] = serde_json::json!([""]);
    let mut wrong_array_type = base.clone();
    wrong_array_type["introduces"] = serde_json::json!("canonical-order");
    let mut wrong_item_type = base.clone();
    wrong_item_type["may_discard"] = serde_json::json!([7]);
    let mut wrong_boolean_type = base;
    wrong_boolean_type["has_effects"] = serde_json::json!("false");

    vec![
        ("unknown field", unknown),
        ("missing field", missing),
        ("duplicate array member", duplicate),
        ("empty array member", empty),
        ("wrong array type", wrong_array_type),
        ("wrong array item type", wrong_item_type),
        ("wrong boolean type", wrong_boolean_type),
        ("non-object", serde_json::json!([])),
    ]
}

fn assert_phase_minus_1_identity_and_projection_evidence() -> Result<(), String> {
    use liminal_conformance::identity::matrix::Matrix;
    use liminal_conformance::identity::{Config, Strategy, claims};
    use liminal_id::IdentityGrade;

    let config = Config::load().map_err(|error| error.to_string())?;
    let matrix = Matrix::build(&config).map_err(|error| error.to_string())?;
    let external_floor = claims::external_file_floor();
    let graph_floor = claims::graph_native_floor();
    if external_floor != IdentityGrade::Anchored || graph_floor != IdentityGrade::Managed {
        return Err("live profile identity floors differ from ADR-0009".into());
    }
    for strategy in [
        Strategy::InlineId,
        Strategy::Sidecar,
        Strategy::Structural,
        Strategy::RevisionAnchor,
    ] {
        let index = claims::strategy_index(&matrix, strategy)
            .ok_or_else(|| format!("missing identity strategy {}", strategy.kebab()))?;
        let cap = claims::column_cap(&matrix, index);
        if !cap.satisfies(external_floor) {
            return Err(format!(
                "strategy {} cap {cap:?} is below external-file floor {external_floor:?}",
                strategy.kebab()
            ));
        }
    }
    let managed_index = claims::strategy_index(&matrix, Strategy::ManagedGraph)
        .ok_or("missing managed-graph identity strategy")?;
    if !claims::column_cap(&matrix, managed_index).satisfies(graph_floor) {
        return Err("managed-graph evidence is below graph-native floor".into());
    }

    let foreign =
        spike_annotation::run_foreign_edit_loop(&config).map_err(|error| error.to_string())?;
    let rich = spike_richedit::run_richedit_loop(&config);
    let document = spike_annotation::AnnotatedDoc::build(&config.template, 33)
        .map_err(|error| error.to_string())?;
    let parsed = spike_annotation::parse(&spike_annotation::emit(&document))
        .map_err(|error| error.to_string())?;
    let measured_level =
        spike_annotation::declared_level(&foreign, &rich, parsed.text == document.text);
    if measured_level != 2 || spike_annotation::DECLARED_LEVEL != measured_level {
        return Err(format!(
            "annotated-source level drifted: measured={measured_level}, declared={}",
            spike_annotation::DECLARED_LEVEL
        ));
    }

    liminal_conformance::pandoc::assert_pandoc_pinned();
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixtures/conversion-loss/pandoc");
    let workdir = phase0_temp_dir(15_010)?;
    let _ = std::fs::remove_dir_all(&workdir);
    let measurement = liminal_conformance::pandoc::measure(&fixture, &workdir)
        .map_err(|error| error.to_string())?;
    let measured_level = measurement.declared_level();
    let _ = std::fs::remove_dir_all(&workdir);
    if measured_level != 1 || liminal_conformance::pandoc::DECLARED_LEVEL != measured_level {
        return Err(format!(
            "Pandoc level drifted: measured={measured_level}, declared={}",
            liminal_conformance::pandoc::DECLARED_LEVEL
        ));
    }
    Ok(())
}

fn assert_phase_minus_1_goldens_unchanged() -> Result<(), String> {
    use liminal_conformance::identity::Config;
    use liminal_conformance::identity::matrix::{Matrix, redact_git_line};

    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = Config::load().map_err(|error| error.to_string())?;

    let identity = Matrix::build(&config).map_err(|error| error.to_string())?;
    let identity_golden = std::fs::read_to_string(root.join("golden/identity_matrix.md"))
        .map_err(|error| error.to_string())?;
    if redact_git_line(&identity.render()) != redact_git_line(&identity_golden) {
        return Err("identity matrix differs from its committed golden".into());
    }

    let foreign =
        spike_annotation::run_foreign_edit_loop(&config).map_err(|error| error.to_string())?;
    let rich = spike_richedit::run_richedit_loop(&config);
    let anchor_report = spike_annotation::render_report(
        &config,
        &foreign,
        &rich,
        spike_annotation::BOX_START,
        "2026-07-18",
    );
    let anchor_golden = std::fs::read_to_string(root.join("golden/anchor_recovery.md"))
        .map_err(|error| error.to_string())?;
    if redact_anchor_report(&anchor_report) != redact_anchor_report(&anchor_golden) {
        return Err("anchor-recovery report differs from its committed golden".into());
    }

    liminal_conformance::pandoc::assert_pandoc_pinned();
    let workdir = phase0_temp_dir(15_011)?;
    let _ = std::fs::remove_dir_all(&workdir);
    let measurement = liminal_conformance::pandoc::measure(
        &root.join("fixtures/conversion-loss/pandoc"),
        &workdir,
    )
    .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&workdir);
    let pandoc_report = liminal_conformance::pandoc::loss_report(&measurement);
    let pandoc_golden = std::fs::read_to_string(root.join("golden/pandoc_loss.md"))
        .map_err(|error| error.to_string())?;
    if pandoc_report != pandoc_golden {
        return Err("Pandoc loss report differs from its committed golden".into());
    }

    Ok(())
}

fn redact_anchor_report(report: &str) -> String {
    let mut redacted = String::with_capacity(report.len());
    for line in report.lines() {
        if line.starts_with("box: ") {
            redacted.push_str("box: [boxed] (hard 2-week box)\n");
        } else if line.starts_with("config: ") {
            if let Some(git_pos) = line.find("git: ") {
                redacted.push_str(&line[..git_pos]);
                redacted.push_str("git: [redacted]\n");
            } else {
                redacted.push_str(line);
                redacted.push('\n');
            }
        } else {
            redacted.push_str(line);
            redacted.push('\n');
        }
    }
    redacted
}

fn assert_phase1_boundary_adrs() -> Result<(), String> {
    let adrs = [
        (
            "0015",
            include_str!("../../docs/adr/0015-select-phase-1-source-grammar.md"),
            "# 0015. Select Phase 1 source grammar",
            "**Decision:** option 3. Begin Phase 1 with the constrained Markdown-compatible frontend",
        ),
        (
            "0016",
            include_str!(
                "../../docs/adr/0016-use-stable-debug-json-as-interim-graph-interchange.md"
            ),
            "# 0016. Use stable debug JSON as interim graph interchange",
            "Use versioned stable debug JSON as the only Phase 1 graph interchange.",
        ),
        (
            "0017",
            include_str!("../../docs/adr/0017-select-phase-1-incremental-engine.md"),
            "# 0017. Select Phase 1 incremental engine",
            "**Decision:** option 1. Salsa retires less novel infrastructure risk",
        ),
        (
            "0018",
            include_str!("../../docs/adr/0018-preserve-interpretive-jurisdiction-oracles.md"),
            "# 0018. Preserve interpretive Jurisdiction oracles through Phase 1",
            "Keep interpretive implementations as conformance oracles throughout Phase 1.",
        ),
    ];
    let index = include_str!("../../docs/adr/README.md");
    for (number, adr, title, decision) in adrs {
        assert_accepted_adr(number, adr, title, decision, index)?;
    }
    Ok(())
}

fn assert_accepted_adr(
    number: &str,
    adr: &str,
    title: &str,
    decision: &str,
    index: &str,
) -> Result<(), String> {
    if adr.lines().filter(|line| *line == title).count() != 1 {
        return Err(format!("ADR-{number} lacks its unique frozen title"));
    }
    if adr
        .lines()
        .filter(|line| *line == "- **Status:** accepted")
        .count()
        != 1
    {
        return Err(format!("ADR-{number} is not uniquely accepted"));
    }
    if adr.contains("user decision required")
        || adr.contains("user acceptance required")
        || adr.contains("user review required")
        || adr.contains("**Status:** proposed")
    {
        return Err(format!(
            "ADR-{number} retains an unresolved decision marker"
        ));
    }
    let normalized = normalize_markdown_words(adr);
    if normalized.matches(decision).count() != 1 {
        return Err(format!(
            "ADR-{number} lacks exactly one approved decision {decision:?}"
        ));
    }
    let index_prefix = format!("| [{number}]");
    let index_rows = index
        .lines()
        .filter(|line| line.starts_with(&index_prefix) && line.ends_with("| accepted |"))
        .count();
    if index_rows != 1 {
        return Err(format!(
            "ADR-{number} index status is not uniquely accepted"
        ));
    }
    Ok(())
}

fn assert_prior_art_dispositions() -> Result<(), String> {
    let adr = include_str!("../../docs/adr/0019-dispose-phase-0-prior-art-commitments.md");
    let index = include_str!("../../docs/adr/README.md");
    assert_accepted_adr(
        "0019",
        adr,
        "# 0019. Dispose Phase 0 prior-art commitments",
        "Accept all six dispositions as architectural boundaries.",
        index,
    )?;

    let matrix = adr
        .split_once("## Disposition matrix\n")
        .map(|(_, tail)| tail)
        .and_then(|tail| tail.split_once("\n## Decision").map(|(table, _)| table))
        .ok_or("ADR-0019 lacks a bounded disposition matrix")?;
    let expected = [
        "Salsa",
        "Automerge and Peritext",
        "AtJSON",
        "Pandoc",
        "Unison",
        "Datomic",
    ];
    let mut found = Vec::new();
    for line in matrix.lines().filter(|line| line.starts_with("| ")) {
        if line.starts_with("| Reference ") || line.starts_with("|---") {
            continue;
        }
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.len() != 5 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!("invalid prior-art disposition row: {line}"));
        }
        if !matches!(fields[1], "Adopt" | "Adapt" | "Defer" | "Reject") {
            return Err(format!("invalid disposition {:?}", fields[1]));
        }
        found.push(fields[0]);
    }
    if found != expected {
        return Err(format!(
            "prior-art inventory differs: expected {expected:?}, got {found:?}"
        ));
    }
    Ok(())
}

fn normalize_markdown_words(markdown: &str) -> String {
    markdown.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn assert_fixture_inventory_governed() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("conformance manifest lacks workspace parent")?;
    let output = std::process::Command::new("git")
        .args(["ls-files", "conformance/fixtures"])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("git fixture inventory failed".into());
    }
    let tracked = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let families = tracked
        .lines()
        .filter_map(|path| {
            path.strip_prefix("conformance/fixtures/")?
                .split('/')
                .next()
        })
        .filter(|name| *name != "README.md")
        .collect::<BTreeSet<_>>();
    if families.is_empty() {
        return Err("no tracked fixture families".into());
    }

    let inventory = include_str!("../fixtures/README.md");
    let mut documented = BTreeSet::new();
    for line in inventory.lines().filter(|line| line.starts_with("| ")) {
        if line.starts_with("| Family ") || line.starts_with("|---") {
            continue;
        }
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.len() != 7 {
            return Err(format!("fixture inventory row has {} fields", fields.len()));
        }
        if fields[..6]
            .iter()
            .any(|field| field.is_empty() || *field == "—")
        {
            return Err(format!(
                "fixture family {:?} has an unowned field",
                fields[0]
            ));
        }
        if !matches!(fields[6], "public" | "sanitized") {
            return Err(format!(
                "fixture family {:?} has invalid confidentiality {:?}",
                fields[0], fields[6]
            ));
        }
        if !documented.insert(fields[0]) {
            return Err(format!("fixture family {:?} is duplicated", fields[0]));
        }
    }
    if documented != families {
        return Err(format!(
            "documented fixture families {documented:?} differ from tracked {families:?}"
        ));
    }
    Ok(())
}

fn assert_sound_session_conjunction() -> Result<(), String> {
    use liminal_conformance::harness::{ToyRun, all_scenarios, assert_silent};
    use liminal_revision::BasisPerspective;

    let scenarios = all_scenarios().map_err(|error| error.to_string())?;
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.scenario.id == "sound_session")
        .ok_or("sound_session fixture missing")?;
    let run = ToyRun::new("phase0-sound-conjunction").map_err(|error| error.to_string())?;
    let execution = run
        .exec_scenario(scenario)
        .map_err(|error| error.to_string())?;
    if !execution.status.success() {
        return Err(format!(
            "sound session execution failed: {}",
            String::from_utf8_lossy(&execution.stderr)
        ));
    }
    let output = run.check().map_err(|error| error.to_string())?;
    assert_silent(&output);

    let workspace =
        liminal_daemon::ToyWorkspace::open(&run.root).map_err(|error| error.to_string())?;
    let basis = workspace
        .basis(BasisPerspective::DurableOnly)
        .map_err(|error| error.to_string())?;
    let report = workspace
        .checker()
        .check_workspace(&basis)
        .map_err(|error| error.to_string())?;
    if !report.is_sound()
        || !workspace
            .reconciliation()
            .items()
            .map_err(|error| error.to_string())?
            .is_empty()
        || !workspace
            .store()
            .scan_aux(liminal_graph::ns::JUR_OVERLAY)
            .map_err(|error| error.to_string())?
            .is_empty()
    {
        return Err("sound session emitted findings, items, or overlays".into());
    }

    let corpus =
        camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/traces/labeled");
    let (scorecard, drafts) = liminal_conformance::pipeline::run(&corpus, "external-file")
        .map_err(|error| error.to_string())?;
    if scorecard.sound_session_diagnostics != 0
        || scorecard.sound_checker_output_bytes != 0
        || scorecard.unexpected_reconciliation_items != 0
        || scorecard.manual_contract_authoring != 0
    {
        return Err(format!(
            "development scorecard is not silent: {scorecard:?}"
        ));
    }
    if drafts.declared == 0 || drafts.reconciled_within_window == 0 {
        return Err(format!(
            "transient drafts were hidden instead of declared and reconciled: {drafts:?}"
        ));
    }
    assert_aged_overlay_remains_visible(&scenarios)?;

    let cli = include_str!("../../crates/liminal-cli/src/main.rs");
    for prohibited in [
        ["Set", "Contract"].concat(),
        ["Author", "Contract"].concat(),
        ["Edit", "Contract"].concat(),
    ] {
        if cli.contains(&prohibited) {
            return Err(format!(
                "standard CLI exposes manual Contract authoring surface {prohibited}"
            ));
        }
    }
    drop(workspace);
    std::fs::remove_dir_all(&run.root).map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_aged_overlay_remains_visible(
    scenarios: &[liminal_conformance::ScenarioScript],
) -> Result<(), String> {
    let mut scenario = scenarios
        .iter()
        .find(|scenario| scenario.scenario.id == "offline_holder_overlay")
        .cloned()
        .ok_or("offline_holder_overlay fixture missing")?;
    scenario.steps.truncate(3);
    let mut extra = toml::Table::new();
    extra.insert("by_secs".into(), toml::Value::Integer(90_000));
    scenario.steps.push(liminal_daemon::scenario::Step {
        kind: "advance_clock".into(),
        extra,
    });

    let run =
        liminal_conformance::ToyRun::new("phase0-aged-debt").map_err(|error| error.to_string())?;
    let execution = run
        .exec_scenario(&scenario)
        .map_err(|error| error.to_string())?;
    if !execution.status.success() {
        return Err("aged Overlay witness failed to execute".into());
    }
    let workspace =
        liminal_daemon::ToyWorkspace::open(&run.root).map_err(|error| error.to_string())?;
    let items = workspace
        .reconciliation()
        .items()
        .map_err(|error| error.to_string())?;
    let overlays = workspace
        .store()
        .scan_aux(liminal_graph::ns::JUR_OVERLAY)
        .map_err(|error| error.to_string())?;
    if items.len() != 1 || overlays.len() != 1 {
        return Err("aged transient debt was hidden or deleted".into());
    }
    drop(workspace);
    std::fs::remove_dir_all(&run.root).map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_phase0_threat_model_complete() -> Result<(), String> {
    let model = include_str!("../../docs/security/threat-model.md");
    let expected_boundaries = [
        "graph store",
        "filesystem Holders",
        "clients/buffers",
        "resolver observations",
        "repair execution",
        "plugins/processes",
        "corpus handling",
        "supply chain",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut boundary_counts = std::collections::BTreeMap::new();
    for line in model
        .lines()
        .filter(|line| line.starts_with("| TM-") && !line.starts_with("| TM-ID"))
    {
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.len() != 14 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "threat row must contain 14 non-empty fields: {line}"
            ));
        }
        let id = fields[0];
        if id.len() != 5
            || !id.starts_with("TM-")
            || !id[3..].bytes().all(|byte| byte.is_ascii_digit())
            || !ids.insert(id)
        {
            return Err(format!("invalid or duplicate threat id {id:?}"));
        }
        if !expected_boundaries.contains(fields[2]) {
            return Err(format!("unknown trust boundary {:?}", fields[2]));
        }
        *boundary_counts.entry(fields[2]).or_insert(0usize) += 1;
        if !matches!(fields[12], "low" | "medium" | "high" | "critical") {
            return Err(format!("invalid impact {:?}", fields[12]));
        }
        if !matches!(fields[13], "unlikely" | "possible" | "likely") {
            return Err(format!("invalid likelihood {:?}", fields[13]));
        }
    }
    if ids.len() < 16 {
        return Err(format!("threat inventory has only {} threats", ids.len()));
    }
    for boundary in expected_boundaries {
        if boundary_counts.get(boundary).copied().unwrap_or(0) < 2 {
            return Err(format!(
                "trust boundary {boundary:?} lacks two threat probes"
            ));
        }
    }
    if !include_str!("../../SECURITY.md").contains("docs/security/threat-model.md") {
        return Err("SECURITY.md does not link the Phase 0 threat model".into());
    }
    Ok(())
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
fn transform_contract_schema_roundtrips_all_fields() {
    let schema = include_str!("../../spec/fixtures/transform-contract.schema.json");
    let examples = include_str!("../../spec/fixtures/transform-contract.examples.json");
    assert_transform_schema_shape(schema).unwrap();
    let contracts = parse_transform_examples(examples).unwrap();
    assert!(
        contracts
            .iter()
            .any(|contract| contract.may_discard.is_empty())
    );
    assert!(
        contracts
            .iter()
            .any(|contract| !contract.may_discard.is_empty())
    );
    for contract in contracts {
        validate_transform_contract(&serde_json::to_value(&contract).unwrap()).unwrap();
        let bytes = serde_json::to_vec(&contract).unwrap();
        let decoded: liminal_transform::TransformContract = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, contract);
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    }
}

/// Exercises unknown, missing, duplicate, empty, and wrong-typed fields as
/// independent malformed schema cases under `additionalProperties: false`.
/// Fails closed if any malformed value deserializes or validates successfully.
#[test]
fn transform_contract_schema_rejects_malformed_cases() {
    for (case, value) in malformed_transform_contracts() {
        assert!(
            validate_transform_contract(&value).is_err(),
            "malformed case {case:?} was accepted"
        );
    }
}

/// Audits normative identity and projection claims against accepted ADR values
/// and independently regenerated Phase -1 measurements without blessing them.
/// Fails closed if any grade or level differs from, or exceeds, its evidence.
#[test]
fn projection_laws_and_identity_ceilings_are_frozen() {
    assert_phase_minus_1_identity_and_projection_evidence().unwrap();
    assert_phase_minus_1_goldens_unchanged().unwrap();
    let kernel = include_str!("../../spec/kernel.md");
    let ir = include_str!("../../spec/ir.md");
    let syntax = include_str!("../../spec/syntax.md");
    for anchor in [
        "K-ID-01",
        "K-ID-02",
        "IR-PROJ-01",
        "IR-PROJ-02",
        "IR-XFORM-01",
        "SYN-ID-01",
    ] {
        let count = kernel.match_indices(anchor).count()
            + ir.match_indices(anchor).count()
            + syntax.match_indices(anchor).count();
        assert_eq!(
            count, 1,
            "normative evidence anchor {anchor} must be unique"
        );
    }
}

/// Requires one accepted, explicit ADR decision for each §119, §121, §122, and
/// §125 boundary before any parser, engine, or compiled-plan implementation.
/// Fails closed on a missing, proposed, rejected, duplicated, or ambiguous choice.
#[test]
fn phase1_boundary_adrs_are_accepted_and_unambiguous() {
    assert_phase1_boundary_adrs().unwrap();
}

/// Conjoins byte-empty diagnostics, zero unexpected items, zero manual Contract
/// authoring, and no hidden transient debt beyond the policy window.
/// Fails closed when any conjunct emits, persists, or requires manual intervention.
#[test]
fn sound_session_zero_diagnostics_items_and_authoring() {
    assert_sound_session_conjunction().unwrap();
}

/// Compares documented public and sanitized fixture families with tracked paths
/// outside locked trees, requiring a version, parser, owner, and confidentiality.
/// Fails closed on undocumented, unowned, versionless, or locked-tree inventory.
#[test]
fn phase0_fixture_inventory_is_versioned_and_owned() {
    assert_fixture_inventory_governed().unwrap();
}

/// Checks every named trust boundary has assets, attacker, entry point, failure,
/// controls, detection, recovery, verification, residual risk, and owner.
/// Fails closed on any missing boundary or unsupported mitigation claim.
#[test]
fn phase0_threat_model_covers_every_trust_boundary() {
    assert_phase0_threat_model_complete().unwrap();
}

/// Compares the complete v4 §118A reference inventory with the accepted ADR,
/// requiring one Adopt, Adapt, Defer, or Reject disposition and falsifier each.
/// Fails closed on an omitted, duplicated, unaccepted, or unfalsifiable entry.
#[test]
fn prior_art_dispositions_are_accepted() {
    assert_prior_art_dispositions().unwrap();
}

/// Exercises frozen positive, negative, malformed, and panic-capture cases while
/// comparing incremental recovery with an independent full-reparse oracle.
/// Fails closed on panic, graph/loss divergence, boundary loss, or bad undo.
#[test]
fn real_cst_spike_oracle_handles_synthetic_cases() {
    let measurement = spike_real_cst::run_frozen_measurement()
        .expect("the frozen real-CST measurement must execute without panic or oracle drift");
    spike_real_cst::verify_synthetic_coverage(&measurement)
        .expect("positive, negative, malformed, and panic-capture coverage must be explicit");
}

/// Recomputes the sorted real-CST results, counts, Wilson interval, inventory hash,
/// and provenance against the reviewed golden with only elapsed time redacted.
/// Fails closed on missing cases, changed seeds, unsorted rows, or byte drift.
#[test]
fn real_cst_anchor_recovery_report_matches_golden() {
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_path = root.join("golden/anchor_recovery_real_cst.md");
    let golden = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|error| panic!("read reviewed real-CST golden {golden_path}: {error}"));
    let git_commit = real_cst_report_field(&golden, "git_commit").unwrap();
    let measurement_tree = real_cst_report_field(&golden, "measurement_tree").unwrap();
    assert_real_cst_provenance(git_commit, measurement_tree).unwrap();

    let measurement = spike_real_cst::run_frozen_measurement().unwrap();
    let report = spike_real_cst::render_report(&measurement, git_commit, measurement_tree);
    assert_eq!(
        spike_real_cst::redact_elapsed(&report),
        spike_real_cst::redact_elapsed(&golden),
        "real-CST measurement drifted from the reviewed golden"
    );
}

/// Independently derives the supported level from every D17.3 measurement
/// conjunct and compares it with the report's declared capability.
/// Fails closed to Level 2 unless the ≥95% bar and every other L3 condition pass.
#[test]
fn real_cst_declared_level_never_exceeds_measurement() {
    let measurement = spike_real_cst::run_frozen_measurement().unwrap();
    assert_eq!(measurement.merge_total, 8);
    assert!(measurement.merge_rate >= 0.95);
    assert!(measurement.malformed_nonpanic);
    assert!(measurement.selection_and_mark_boundaries_survive);
    assert!(measurement.undo_correct);
    assert!(measurement.no_undeclared_loss);
    assert!(
        !measurement.canonical_roundtrip_supported,
        "tree-sitter-md has no independently qualified lossless native serializer"
    );
    assert_eq!(spike_real_cst::derived_level(&measurement), 2);
    assert_eq!(
        spike_real_cst::DECLARED_LEVEL,
        spike_real_cst::derived_level(&measurement),
        "declared capability exceeds independently derived measurement"
    );
}

fn real_cst_report_field<'a>(report: &'a str, key: &str) -> Result<&'a str, String> {
    let prefix = format!("{key}: ");
    let mut matches = report.lines().filter_map(|line| line.strip_prefix(&prefix));
    let value = matches
        .next()
        .ok_or_else(|| format!("real-CST report is missing {key}"))?;
    if matches.next().is_some() {
        return Err(format!("real-CST report repeats {key}"));
    }
    Ok(value)
}

fn assert_real_cst_provenance(git_commit: &str, measurement_tree: &str) -> Result<(), String> {
    for (name, value) in [
        ("git_commit", git_commit),
        ("measurement_tree", measurement_tree),
    ] {
        if value.len() != 40
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!("{name} is not 40 lowercase hexadecimal characters"));
        }
    }

    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "conformance directory has no repository parent".to_owned())?
        .to_owned();
    let output = std::process::Command::new("git")
        .args([
            "-C",
            root.as_str(),
            "rev-parse",
            &format!("{git_commit}^{{tree}}"),
        ])
        .output()
        .map_err(|error| format!("run git rev-parse for real-CST provenance: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "real-CST measurement commit is unavailable: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let actual_tree = String::from_utf8(output.stdout)
        .map_err(|error| format!("git returned non-UTF-8 tree id: {error}"))?;
    if actual_tree.trim() != measurement_tree {
        return Err(format!(
            "real-CST measurement tree mismatch: report={measurement_tree}, commit={}",
            actual_tree.trim()
        ));
    }
    Ok(())
}

/// Checks the template-derived Phase 1 inventory and proposed-suite authority.
/// Fails closed unless the machine inventory is complete, bound into the review
/// packet, and still unratified/proposed.
#[test]
#[ignore = "Phase 0 M17: HAQP qualification evidence not yet complete"]
fn phase1_suite_packet_is_complete_and_unratified() {
    let root = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    liminal_xtask::haq::verify_qualified_repo(root)
        .expect("Phase 1 HAQP qualification evidence must verify");
}

/// Requires an accepted ADR with exactly one explicit user GO or NO-GO decision
/// and a complete Phase 0 results table; green CI is not an independent decision.
/// Fails closed on absent authority, ambiguous verdicts, or incomplete evidence.
#[test]
#[ignore = "Phase 0 M17: final Phase 0 decision"]
fn phase0_go_no_go_adr_recorded() {
    unimplemented!("require the Phase 0 go-no-go ADR");
}
