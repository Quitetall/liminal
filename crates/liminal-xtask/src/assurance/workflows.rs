//! Deterministic hosted command wiring; no workflow parser or scheduler.

use std::collections::BTreeMap;
use std::io::Write;

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;

const CI: &str = include_str!("../../../../verification/assurance/ci.template.yml");
const SCHEDULED: &str = include_str!("../../../../verification/assurance/scheduled.template.yml");

fn shell_command(id: &str) -> Result<String> {
    match id {
        "assurance" => return Ok("cargo run -p liminal-xtask -- assurance check".into()),
        "resource-run" | "resource-lint" => {
            return Ok(format!(
                "python3 -B verification/assurance/resource.py {id} --output \"$RUNNER_TEMP/liminal-{id}\""
            ));
        }
        _ => {}
    }
    let command = super::runner::registered(id, Utf8Path::new("unused"))?;
    Ok(std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| format!("'{}'", arg.to_string_lossy().replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" "))
}

fn render(catalog: &super::Catalog) -> Result<[(String, String); 2]> {
    let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for id in &catalog.profiles["merge"] {
        let group = match id.as_str() {
            "style" => "STYLE",
            "lint" => "CLIPPY",
            "docs" => "DOCS",
            "deny" => "DENY",
            "nextest" | "doctest" => "PORTABILITY",
            // `threaded` runs every test in one libtest process, host-capability
            // tests included, so it runs where those capabilities are
            // provisioned. On the plain runner it failed for want of pandoc.
            "host" | "threaded" => "HOST",
            "tracked-sizes" | "fuzz-lock" | "bootstrap" | "proof-support" | "partition"
            | "inventory" | "canaries" | "resource-run" | "resource-lint" | "assurance" => "LINUX",
            _ => bail!("unmapped mandatory CI command: {id}"),
        };
        groups.entry(group).or_default().push(shell_command(id)?);
    }
    for id in &catalog.profiles["scheduled"] {
        let group = match id.as_str() {
            "advisories" => "ADVISORIES",
            "beta" => "BETA",
            _ => bail!("unmapped scheduled command: {id}"),
        };
        groups.entry(group).or_default().push(shell_command(id)?);
    }
    let mut ci = CI.to_string();
    let mut scheduled = SCHEDULED.to_string();
    for (group, commands) in groups {
        let marker = format!("@@{group}@@");
        ensure!(
            ci.matches(&marker).count() + scheduled.matches(&marker).count() == 1,
            "workflow template missing/duplicated group {group}"
        );
        let lines = commands
            .iter()
            .map(|line| format!("          {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        ci = ci.replace(&marker, &lines);
        scheduled = scheduled.replace(&marker, &lines);
    }
    ensure!(
        !ci.contains("@@") && !scheduled.contains("@@"),
        "unknown workflow template marker"
    );
    Ok([("ci.yml".into(), ci), ("scheduled.yml".into(), scheduled)])
}

pub(super) fn check(root: &Utf8Path, catalog: &super::Catalog) -> Result<()> {
    // Also bind source templates to this executable: stale compiled tools must
    // not bless outputs rendered from different template bytes.
    for (path, expected) in [
        ("ci.template.yml", CI),
        ("scheduled.template.yml", SCHEDULED),
    ] {
        let source = root.join("verification/assurance").join(path);
        // Synthetic CLI fixtures have no source templates. Real repositories
        // with this xtask must carry the exact embedded templates.
        if root.join("crates/liminal-xtask/Cargo.toml").exists() {
            ensure!(
                std::fs::read_to_string(source)? == expected,
                "compiled workflow templates stale"
            );
        }
    }
    for (name, expected) in render(catalog)? {
        let path = format!(".github/workflows/{name}");
        let actual = std::fs::read_to_string(super::inside(root, &path)?)
            .context("workflow drift: missing workflow")?;
        ensure!(actual == expected, "workflow drift: {path}");
    }
    Ok(())
}

/// Explicitly regenerate hosted wiring from validated profile metadata.
///
/// # Errors
/// Refuses invalid catalog/command groups and escaping output paths. Does not
/// run CI, enroll trust or change qualification authority.
pub fn generate(root: &Utf8Path) -> Result<()> {
    let root = root.canonicalize_utf8()?;
    let catalog = super::load_catalog(&root)?;
    for (name, contents) in render(&catalog)? {
        if !root.join(".github").exists() {
            std::fs::create_dir(root.join(".github"))?;
        }
        let github = super::inside(&root, ".github")?;
        if !github.join("workflows").exists() {
            std::fs::create_dir(github.join("workflows"))?;
        }
        let parent = super::inside(&root, ".github/workflows")?;
        let temporary = parent.join(format!(".assurance-{}", uuid::Uuid::now_v7()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(temporary, parent.join(name))?;
    }
    Ok(())
}
