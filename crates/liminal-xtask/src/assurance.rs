//! Assurance maintenance, not qualification authority (ADR-0022).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    #[serde(deserialize_with = "unique_profiles")]
    profiles: BTreeMap<String, Vec<String>>,
    families: Vec<Family>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Family {
    id: String,
    owner: String,
    source_areas: Vec<String>,
    targets: Vec<String>,
    references: Vec<Reference>,
    review_triggers: Vec<String>,
    profiles: Vec<String>,
    platforms: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    path: String,
    anchor: String,
}

fn unique_profiles<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Vec<String>>, D::Error> {
    struct Profiles;
    impl<'de> serde::de::Visitor<'de> for Profiles {
        type Value = BTreeMap<String, Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("profiles with unique keys")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut profiles = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, Vec<String>>()? {
                if profiles.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate profile key"));
                }
            }
            Ok(profiles)
        }
    }
    deserializer.deserialize_map(Profiles)
}

// This baseline is independent of catalog/workflow generation. Removing a gate
// from both projections must still fail. Changes require ADR-0022 review.
const PROFILES: &[(&str, &[&str])] = &[
    ("development", &["style", "assurance"]),
    (
        "merge",
        &[
            "style",
            "lint",
            "bootstrap",
            "proof-support",
            "nextest",
            "threaded",
            "doctest",
            "docs",
            "deny",
            "inventory",
            "canaries",
            "resource-run",
            "resource-lint",
            "assurance",
        ],
    ),
    ("portability", &["nextest", "doctest"]),
    ("scheduled", &["advisories", "beta"]),
    ("qualification", &["haqp"]),
];

/// Check classification and command completeness without executing tests.
///
/// # Errors
/// Returns an error for malformed metadata, uncovered/stale targets or missing
/// authority references. A successful result never establishes qualification.
pub fn check(root: &Utf8Path) -> Result<()> {
    let root = root
        .canonicalize_utf8()
        .context("canonical repository root")?;
    let catalog = load_catalog(&root)?;
    check_catalog(&root, &catalog)?;
    println!("assurance catalog: pass; qualification: not-established");
    Ok(())
}

fn load_catalog(root: &Utf8Path) -> Result<Catalog> {
    let catalog: Catalog = serde_json::from_slice(&std::fs::read(inside(
        root,
        "verification/assurance/catalog.json",
    )?)?)?;
    ensure!(catalog.schema_version == 1, "unsupported assurance schema");
    ensure!(
        catalog.profiles.len() == PROFILES.len(),
        "profile set drift"
    );
    for (name, expected) in PROFILES {
        let actual = catalog
            .profiles
            .get(*name)
            .context("missing required profile")?;
        ensure!(
            actual
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied()),
            "mandatory command sequence drift: {name}"
        );
    }
    Ok(catalog)
}

fn check_catalog(root: &Utf8Path, catalog: &Catalog) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut targets = BTreeMap::new();
    for family in &catalog.families {
        ensure!(
            !family.id.trim().is_empty() && ids.insert(&family.id),
            "empty or duplicate family ID"
        );
        ensure!(
            !family.owner.trim().is_empty(),
            "missing owner: {}",
            family.id
        );
        ensure!(
            !family.source_areas.is_empty()
                && !family.targets.is_empty()
                && !family.references.is_empty()
                && !family.review_triggers.is_empty(),
            "incomplete family: {}",
            family.id
        );
        ensure!(
            !family.profiles.is_empty()
                && family
                    .profiles
                    .iter()
                    .all(|p| catalog.profiles.contains_key(p)),
            "unknown family profile: {}",
            family.id
        );
        ensure!(
            !family.platforms.is_empty()
                && family
                    .platforms
                    .iter()
                    .all(|p| ["linux", "macos", "windows"].contains(&p.as_str())),
            "unknown platform: {}",
            family.id
        );
        for area in &family.source_areas {
            inside(root, area)?;
        }
        for target in &family.targets {
            ensure!(
                targets.insert(target.clone(), family).is_none(),
                "duplicate target: {target}"
            );
        }
        for reference in &family.references {
            ensure!(!reference.anchor.is_empty(), "empty authority anchor");
            let text = std::fs::read_to_string(inside(root, &reference.path)?)?;
            ensure!(
                text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                    .any(|word| word == reference.anchor),
                "missing authority anchor: {}#{}",
                reference.path,
                reference.anchor
            );
        }
    }
    discover_rust(root, "Cargo.toml", false, &mut targets)?;
    for manifest in [
        "fuzz/Cargo.toml",
        "verification/resource-controls/Cargo.toml",
    ] {
        if root.join(manifest).exists() {
            discover_rust(root, manifest, true, &mut targets)?;
        }
    }
    for directory in ["verification/bootstrap", "verification/proof"] {
        if root.join(directory).exists() {
            discover_python(root, directory, &mut targets)?;
        }
    }
    ensure!(
        targets.is_empty(),
        "stale catalog targets: {:?}",
        targets.keys().collect::<Vec<_>>()
    );
    Ok(())
}

fn discover_python(
    root: &Utf8Path,
    directory: &str,
    targets: &mut BTreeMap<String, &Family>,
) -> Result<()> {
    for entry in std::fs::read_dir(inside(root, directory)?)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-UTF8 test path"))?;
        let relative = format!("{directory}/{name}");
        if entry.file_type()?.is_dir() && name != "__pycache__" {
            discover_python(root, &relative, targets)?;
        } else if name.starts_with("test_") && Utf8Path::new(&name).extension() == Some("py") {
            inside(root, &relative)?;
            consume_target(
                &format!("python::{relative}"),
                Utf8Path::new(&relative),
                targets,
            )?;
        }
    }
    Ok(())
}

fn consume_target(
    id: &str,
    source: &Utf8Path,
    targets: &mut BTreeMap<String, &Family>,
) -> Result<()> {
    let family = targets
        .remove(id)
        .with_context(|| format!("unclassified target: {id}"))?;
    ensure!(
        family
            .source_areas
            .iter()
            .any(|area| source.starts_with(area)),
        "target outside family source areas: {id}"
    );
    Ok(())
}

fn discover_rust(
    root: &Utf8Path,
    selected_manifest: &str,
    all: bool,
    targets: &mut BTreeMap<String, &Family>,
) -> Result<()> {
    inside(root, selected_manifest)?;
    let output = std::process::Command::new("cargo")
        .current_dir(root)
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
            selected_manifest,
        ])
        .output()
        .context("cargo target discovery unavailable")?;
    ensure!(
        output.status.success(),
        "cargo target discovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    for package in metadata.packages {
        ensure!(
            package.manifest_path.starts_with(root),
            "package outside repository"
        );
        let manifest = package.manifest_path.strip_prefix(root)?;
        for target in package
            .targets
            .into_iter()
            .filter(|target| target.test || all)
        {
            let id = format!("{manifest}::{}::{}", target.kind.join(","), target.name);
            let source = target
                .src_path
                .strip_prefix(root)
                .context("target outside repository")?;
            inside(root, source.as_str())?;
            consume_target(&id, source, targets)?;
        }
    }
    Ok(())
}

fn inside(root: &Utf8Path, relative: &str) -> Result<Utf8PathBuf> {
    let path = Utf8Path::new(relative);
    ensure!(
        !path.is_absolute()
            && path.components().all(
                |part| matches!(part, camino::Utf8Component::Normal(value) if value != "heldout")
            ),
        "unsafe catalog path: {relative}"
    );
    let resolved = root.join(path).canonicalize_utf8()?;
    let local = resolved
        .strip_prefix(root)
        .context("catalog path escapes repository")?;
    ensure!(
        !local.components().any(|part| part.as_str() == "heldout"),
        "locked corpus is not catalog input"
    );
    Ok(resolved)
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    manifest_path: Utf8PathBuf,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    src_path: Utf8PathBuf,
    test: bool,
}
