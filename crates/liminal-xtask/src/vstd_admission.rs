//! Fail-closed admission for the frozen HAQP `vstd` dependency (DG17.5).
//!
//! This inspects Cargo's resolved metadata. Structural success is dependency
//! admission only; it does not establish a proof or qualify the candidate.

use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;
use serde::Deserialize;

const VSTD_NAME: &str = "vstd";
const VSTD_VERSION: &str = "0.0.0-2026-08-30-0159";
const VSTD_REQUIREMENT: &str = "=0.0.0-2026-08-30-0159";
const VSTD_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Option<Resolve>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    id: String,
    source: Option<String>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    source: Option<String>,
    req: String,
    features: Vec<String>,
    uses_default_features: bool,
    rename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    id: String,
    features: Vec<String>,
}

/// Verify that Cargo resolves only the frozen, featureless `vstd` package.
#[cfg(test)]
pub(crate) fn verify(root: &Utf8Path) -> Result<()> {
    let metadata = load_metadata(root)?;
    validate_metadata_value(&metadata)
}

/// Return whether the workspace has a strictly admitted `vstd` dependency.
pub(crate) fn admitted(root: &Utf8Path) -> Result<bool> {
    let metadata = load_metadata(root)?;
    admit_metadata_value(&metadata)
}

fn admit_metadata_value(metadata: &Metadata) -> Result<bool> {
    ensure!(
        metadata.resolve.is_some(),
        "Cargo metadata contains no resolve graph"
    );
    if !has_vstd_signal(metadata) {
        return Ok(false);
    }
    validate_metadata_value(metadata)?;
    Ok(true)
}

fn load_metadata(root: &Utf8Path) -> Result<Metadata> {
    let manifest = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .current_dir(root)
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--all-features",
            "--manifest-path",
            manifest.as_str(),
        ])
        .output()
        .with_context(|| format!("run cargo metadata for {manifest}"))?;

    if !output.status.success() {
        bail!(
            "cargo metadata refused vstd admission (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    parse_metadata(&output.stdout).context("validate Cargo metadata structure for vstd admission")
}

#[cfg(test)]
fn validate_metadata(bytes: &[u8]) -> Result<()> {
    let metadata = parse_metadata(bytes)?;
    validate_metadata_value(&metadata)
}

#[cfg(test)]
fn admit_metadata(bytes: &[u8]) -> Result<bool> {
    let metadata = parse_metadata(bytes)?;
    admit_metadata_value(&metadata)
}

fn has_vstd_signal(metadata: &Metadata) -> bool {
    metadata.packages.iter().any(|package| {
        package.name == VSTD_NAME
            || package.dependencies.iter().any(|dependency| {
                dependency.name == VSTD_NAME || dependency.rename.as_deref() == Some(VSTD_NAME)
            })
    })
}

fn parse_metadata(bytes: &[u8]) -> Result<Metadata> {
    serde_json::from_slice(bytes).context("Cargo metadata is not valid required JSON")
}

fn validate_metadata_value(metadata: &Metadata) -> Result<()> {
    let mut vstd_packages = metadata
        .packages
        .iter()
        .filter(|package| package.name == VSTD_NAME);
    let package = vstd_packages
        .next()
        .context("Cargo metadata contains no vstd package")?;
    ensure!(
        vstd_packages.next().is_none(),
        "Cargo metadata contains more than one vstd package"
    );
    ensure!(
        package.version == VSTD_VERSION,
        "resolved vstd version drift: expected {VSTD_VERSION}, got {}",
        package.version
    );
    ensure!(
        package.source.as_deref() == Some(VSTD_SOURCE),
        "resolved vstd source drift: expected {VSTD_SOURCE}, got {:?}",
        package.source
    );

    let resolve = metadata
        .resolve
        .as_ref()
        .context("Cargo metadata contains no resolve graph")?;
    let mut nodes = resolve.nodes.iter().filter(|node| node.id == package.id);
    let node = nodes
        .next()
        .context("Cargo resolve graph contains no node for the vstd package")?;
    ensure!(
        nodes.next().is_none(),
        "Cargo resolve graph contains duplicate nodes for the vstd package"
    );
    ensure!(
        node.features.is_empty(),
        "resolved vstd features must be empty, got {:?}",
        node.features
    );

    let mut declaration_count = 0_usize;
    for dependency in metadata
        .packages
        .iter()
        .flat_map(|package| &package.dependencies)
        .filter(|dependency| {
            dependency.name == VSTD_NAME || dependency.rename.as_deref() == Some(VSTD_NAME)
        })
    {
        declaration_count += 1;
        ensure!(
            dependency.name == VSTD_NAME,
            "dependency alias vstd must resolve to the vstd package, not {}",
            dependency.name
        );
        ensure!(
            dependency.req == VSTD_REQUIREMENT,
            "vstd dependency requirement drift: expected {VSTD_REQUIREMENT}, got {}",
            dependency.req
        );
        ensure!(
            dependency.source.as_deref() == Some(VSTD_SOURCE),
            "vstd dependency source drift: expected {VSTD_SOURCE}, got {:?}",
            dependency.source
        );
        ensure!(
            !dependency.uses_default_features,
            "vstd dependency must disable default features"
        );
        ensure!(
            dependency.features.is_empty(),
            "vstd dependency features must be empty, got {:?}",
            dependency.features
        );
    }
    ensure!(
        declaration_count > 0,
        "Cargo metadata contains no vstd dependency declaration"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use camino::Utf8Path;
    use liminal_scratch::ScratchDir;
    use serde_json::{Value, json};

    use super::{
        VSTD_REQUIREMENT, VSTD_SOURCE, VSTD_VERSION, admit_metadata, admitted, validate_metadata,
        verify,
    };

    const VSTD_ID: &str =
        "registry+https://github.com/rust-lang/crates.io-index#vstd@0.0.0-2026-08-30-0159";

    fn valid_metadata() -> Value {
        json!({
            "packages": [
                {
                    "name": "candidate",
                    "version": "0.1.0",
                    "id": "path+file:///candidate#0.1.0",
                    "source": null,
                    "dependencies": [{
                        "name": "vstd",
                        "source": VSTD_SOURCE,
                        "req": VSTD_REQUIREMENT,
                        "features": [],
                        "uses_default_features": false,
                        "rename": null,
                        "kind": null,
                        "target": null
                    }]
                },
                {
                    "name": "vstd",
                    "version": VSTD_VERSION,
                    "id": VSTD_ID,
                    "source": VSTD_SOURCE,
                    "dependencies": []
                }
            ],
            "resolve": {
                "nodes": [{"id": VSTD_ID, "features": []}]
            }
        })
    }

    fn check(value: &Value) -> anyhow::Result<()> {
        validate_metadata(&serde_json::to_vec(value).expect("serialize fixture"))
    }

    fn write_fixture_manifest(root: &Utf8Path, dependency: &str, features: Option<&str>) {
        let features = features.unwrap_or("");
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"vstd-admission-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency}\n\n[workspace]\n{features}"
            ),
        )
        .expect("write fixture manifest");
    }

    fn assert_cargo_metadata_succeeds(root: &Utf8Path) {
        let output = Command::new("cargo")
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--all-features",
                "--manifest-path",
                root.join("Cargo.toml").as_str(),
            ])
            .output()
            .expect("run independent cargo metadata control");
        assert!(
            output.status.success(),
            "inverse control broke Cargo metadata instead of reaching the admission guard: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout)
            .expect("independent cargo metadata control must emit JSON");
    }

    fn assert_live_refusal(root: &Utf8Path, expected: &str) {
        assert_cargo_metadata_succeeds(root);
        let error = admitted(root).expect_err("drifted live Cargo metadata must be refused");
        assert!(
            format!("{error:#}").contains(expected),
            "unexpected refusal for live metadata: {error:#}"
        );
    }

    #[test]
    fn minimal_exact_metadata_is_admitted() {
        check(&valid_metadata()).expect("exact frozen metadata must pass");
        assert!(
            admit_metadata(&serde_json::to_vec(&valid_metadata()).expect("serialize fixture"))
                .expect("admit exact fixture")
        );
    }

    #[test]
    fn valid_metadata_without_vstd_is_not_admitted() {
        let mut metadata = valid_metadata();
        metadata["packages"]
            .as_array_mut()
            .expect("packages array")
            .remove(1);
        metadata["packages"][0]["dependencies"] = json!([]);
        metadata["resolve"]["nodes"] = json!([]);
        assert!(
            !admit_metadata(&serde_json::to_vec(&metadata).expect("serialize fixture"))
                .expect("valid metadata without vstd must be a non-admission")
        );

        metadata["resolve"] = Value::Null;
        assert!(
            admit_metadata(&serde_json::to_vec(&metadata).expect("serialize fixture")).is_err(),
            "missing required resolve structure must refuse even without vstd"
        );
    }

    #[test]
    fn declaration_without_resolved_vstd_package_is_refused() {
        let mut metadata = valid_metadata();
        metadata["packages"]
            .as_array_mut()
            .expect("packages array")
            .remove(1);
        metadata["resolve"]["nodes"] = json!([]);
        assert!(
            admit_metadata(&serde_json::to_vec(&metadata).expect("serialize fixture")).is_err()
        );
    }

    #[test]
    fn wrong_package_version_and_source_are_rejected() {
        let mut wrong_version = valid_metadata();
        wrong_version["packages"][1]["version"] = json!("0.0.0");
        assert!(check(&wrong_version).is_err());

        let mut wrong_source = valid_metadata();
        wrong_source["packages"][1]["source"] = json!("registry+https://example.invalid");
        assert!(check(&wrong_source).is_err());
    }

    #[test]
    fn declaration_pin_source_defaults_and_features_are_rejected_on_drift() {
        for (field, value) in [
            ("req", json!(VSTD_VERSION)),
            ("source", json!("registry+https://example.invalid")),
            ("uses_default_features", json!(true)),
            ("features", json!(["proof_feature"])),
        ] {
            let mut metadata = valid_metadata();
            metadata["packages"][0]["dependencies"][0][field] = value;
            assert!(check(&metadata).is_err(), "accepted drift in {field}");
        }
    }

    #[test]
    fn every_resolved_feature_is_rejected() {
        for feature in ["default", "std", "alloc", "unknown"] {
            let mut metadata = valid_metadata();
            metadata["resolve"]["nodes"][0]["features"] = json!([feature]);
            assert!(check(&metadata).is_err(), "accepted {feature} feature");
        }
    }

    #[test]
    fn duplicate_vstd_packages_and_nodes_are_rejected() {
        let mut packages = valid_metadata();
        let duplicate = packages["packages"][1].clone();
        packages["packages"]
            .as_array_mut()
            .expect("packages array")
            .push(duplicate);
        assert!(check(&packages).is_err());

        let mut nodes = valid_metadata();
        let duplicate = nodes["resolve"]["nodes"][0].clone();
        nodes["resolve"]["nodes"]
            .as_array_mut()
            .expect("nodes array")
            .push(duplicate);
        assert!(check(&nodes).is_err());
    }

    #[test]
    fn missing_node_resolve_and_declaration_are_rejected() {
        let mut missing_node = valid_metadata();
        missing_node["resolve"]["nodes"] = json!([]);
        assert!(check(&missing_node).is_err());

        let mut missing_resolve = valid_metadata();
        missing_resolve["resolve"] = Value::Null;
        assert!(check(&missing_resolve).is_err());

        let mut missing_declaration = valid_metadata();
        missing_declaration["packages"][0]["dependencies"] = json!([]);
        assert!(check(&missing_declaration).is_err());
    }

    #[test]
    fn missing_or_malformed_feature_arrays_are_rejected() {
        let mut missing_declaration_features = valid_metadata();
        missing_declaration_features["packages"][0]["dependencies"][0]
            .as_object_mut()
            .expect("dependency object")
            .remove("features");
        assert!(check(&missing_declaration_features).is_err());

        let mut malformed_resolved_features = valid_metadata();
        malformed_resolved_features["resolve"]["nodes"][0]["features"] = json!(null);
        assert!(check(&malformed_resolved_features).is_err());
    }

    #[test]
    fn renamed_dev_declaration_drift_and_alias_squatting_are_rejected() {
        let mut renamed_dev = valid_metadata();
        renamed_dev["packages"][0]["dependencies"][0]["rename"] = json!("proof_vstd");
        renamed_dev["packages"][0]["dependencies"][0]["kind"] = json!("dev");
        renamed_dev["packages"][0]["dependencies"][0]["req"] = json!("*");
        assert!(check(&renamed_dev).is_err());

        let mut alias_squatting = valid_metadata();
        alias_squatting["packages"][0]["dependencies"][0]["name"] = json!("not-vstd");
        alias_squatting["packages"][0]["dependencies"][0]["rename"] = json!("vstd");
        assert!(check(&alias_squatting).is_err());
    }

    #[test]
    fn actual_cargo_metadata_accepts_only_the_frozen_featureless_dependency() {
        const EXACT: &str =
            "vstd = { version = \"=0.0.0-2026-08-30-0159\", default-features = false }";

        let root = ScratchDir::new("vstd-admission-live").expect("create scratch directory");
        std::fs::create_dir(root.join("src")).expect("create fixture source directory");
        std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n")
            .expect("write fixture source");
        write_fixture_manifest(&root, EXACT, None);

        let lock = Command::new("cargo")
            .args([
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                root.join("Cargo.toml").as_str(),
            ])
            .output()
            .expect("generate fixture lockfile");
        assert!(
            lock.status.success(),
            "generate fixture lockfile: {}",
            String::from_utf8_lossy(&lock.stderr)
        );
        assert_cargo_metadata_succeeds(&root);
        verify(&root).expect("exact live Cargo metadata must pass");
        assert!(admitted(&root).expect("admit exact live metadata"));

        let inverse_controls = [
            (
                "vstd = { version = \"0.0.0-2026-08-30-0159\", default-features = false }",
                None,
                "vstd dependency requirement drift",
            ),
            (
                "vstd = { version = \"=0.0.0-2026-08-30-0159\", default-features = false, features = [\"std\"] }",
                None,
                "resolved vstd features must be empty",
            ),
            (
                "vstd = { version = \"=0.0.0-2026-08-30-0159\", default-features = true }",
                None,
                "resolved vstd features must be empty",
            ),
            (
                "vstd = { version = \"=0.0.0-2026-08-30-0159\", default-features = false, optional = true }",
                Some("[features]\nenable-vstd-std = [\"dep:vstd\", \"vstd/std\"]\n"),
                "resolved vstd features must be empty",
            ),
            (
                "vstd = { version = \"=0.0.0-2026-08-30-0159\", default-features = false, features = [\"alloc\"] }",
                None,
                "resolved vstd features must be empty",
            ),
        ];

        for (dependency, features, expected) in inverse_controls {
            write_fixture_manifest(&root, EXACT, None);
            verify(&root).expect("restored positive control must pass before each inverse");
            write_fixture_manifest(&root, dependency, features);
            assert_live_refusal(&root, expected);
        }

        write_fixture_manifest(
            &root,
            "serde = { package = \"vstd\", version = \"=0.0.0-2026-08-30-0159\", default-features = false }",
            None,
        );
        assert_cargo_metadata_succeeds(&root);
        assert!(admitted(&root).expect("renamed exact vstd dependency must be admitted"));

        write_fixture_manifest(
            &root,
            "serde = { package = \"vstd\", version = \"=0.0.0-2026-08-30-0159\", default-features = true }",
            None,
        );
        assert_live_refusal(&root, "resolved vstd features must be empty");
    }

    #[test]
    fn current_workspace_metadata_is_admitted() {
        let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .parent()
            .and_then(Utf8Path::parent)
            .expect("liminal-xtask must be nested under the workspace root");
        verify(root).expect("current workspace vstd metadata must remain admitted");
    }
}
