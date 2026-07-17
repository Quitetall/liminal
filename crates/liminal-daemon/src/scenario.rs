//! Scenario fixture model — the shared vocabulary between the harness and
//! `liminald exec` (fixtures live in `conformance/fixtures/scenarios/`).
//!
//! Deliberately lenient (`Option` + defaults + pass-through tables): fixtures
//! are data a second implementation could consume (v4 §114), and Phase -1
//! reshapes step vocabularies freely.

use std::fs;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

/// One `*.scenario.toml` fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioScript {
    /// The `[scenario]` header.
    pub scenario: ScenarioHeader,
    /// Workspace setup.
    #[serde(default)]
    pub setup: Setup,
    /// Ordered steps.
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
    /// Expected outcome.
    pub expect: Expectation,
}

/// The `[scenario]` header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioHeader {
    /// Stable id (matches the file stem).
    pub id: String,
    /// Human title.
    #[serde(default)]
    pub title: Option<String>,
    /// Spec citations (e.g. `"v4 §7.7"`).
    #[serde(default)]
    pub spec: Vec<String>,
    /// Profiles the scenario exercises.
    #[serde(default)]
    pub profiles: Vec<String>,
}

/// Workspace setup: files and graph subjects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Setup {
    /// Files to create.
    #[serde(default, rename = "file")]
    pub files: Vec<SetupFile>,
    /// Graph-native subjects to create.
    #[serde(default, rename = "graph")]
    pub graph: Vec<SetupGraph>,
    /// Buffer setup (AM-2.2: additive).
    #[serde(default, rename = "buffer")]
    pub buffers: Vec<SetupBuffer>,
}

/// One file in the toy workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupFile {
    /// Workspace-relative path.
    pub path: String,
    /// Full contents (toy paragraph format: blank-line blocks, optional
    /// `{#id}` suffixes).
    pub text: String,
}

/// One graph-native subject (e.g. the comment Relation of the R4 §10 toy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupGraph {
    /// Subject kind (provisional vocabulary, e.g. `comment-relation`).
    pub kind: String,
    /// Target anchor/id in the file world, when applicable.
    #[serde(default)]
    pub target: Option<String>,
    /// Minimum identity grade the Relation requires (v4 §19.1).
    #[serde(default)]
    pub requires_grade: Option<String>,
    /// Step-specific extra fields, passed through untyped.
    #[serde(flatten)]
    pub extra: toml::Table,
}

/// One buffer in the workspace (AM-2.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupBuffer {
    /// Client name (e.g. "neovim").
    pub client: String,
    /// File path this buffer covers.
    pub path: String,
}

/// One scripted step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step kind (`buffer_edit`, `save`, `foreign_edit`, `holder_unavailable`,
    /// `propose_repair`, … — provisional vocabulary, reshaped freely in
    /// Phase -1).
    pub kind: String,
    /// Step-specific fields, passed through untyped.
    #[serde(flatten)]
    pub extra: toml::Table,
}

/// The `[expect]` block. All fields optional: a scenario asserts only what it
/// is about.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Expectation {
    /// Expected repair plan steps, in dependency order.
    #[serde(default)]
    pub plan_steps: Option<Vec<String>>,
    /// Whether the repair must auto-apply (R4 §6: determinism is not safety).
    #[serde(default)]
    pub auto_apply: Option<bool>,
    /// Expected terminal ILRP state (`Committed` / `NeedsReview` / `Aborted`).
    #[serde(default)]
    pub terminal: Option<String>,
    /// Expected checker stdout — `""` asserts BYTE-EMPTY silence (Law 3E).
    #[serde(default)]
    pub checker_stdout: Option<String>,
    /// Expected number of reconciliation items.
    #[serde(default)]
    pub reconciliation_items: Option<u64>,
    /// Expected number of active overlays.
    #[serde(default)]
    pub overlays: Option<u64>,
    /// Scenario-specific extra expectations, passed through untyped.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl ScenarioScript {
    /// Load a scenario fixture.
    pub fn load(path: &Utf8Path) -> anyhow::Result<Self> {
        let text = fs::read_to_string(path)?;
        let script: Self =
            toml::from_str(&text).map_err(|e| anyhow::anyhow!("{path}: invalid scenario: {e}"))?;
        Ok(script)
    }

    /// Load every `*.scenario.toml` under a directory, sorted by id.
    pub fn load_dir(dir: &Utf8Path) -> anyhow::Result<Vec<Self>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(path) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.as_str().ends_with(".scenario.toml") {
                out.push(Self::load(&path)?);
            }
        }
        out.sort_by(|a, b| a.scenario.id.cmp(&b.scenario.id));
        Ok(out)
    }
}
