//! The four M08.4 pure queries (v4 §24, §7.5), each `impl liminal_query::Query`.
//!
//! A query is a PURE function of a [`WorkspaceBasis`] over a [`World`]: the
//! basis fixes which component (durable file vs a client's dirty buffer, and
//! which graph revision) each read resolves to, and the query records every
//! component key it touches into `deps` (D08.2) so the memo layer can
//! invalidate it precisely. The `World` seam is what lets the SAME query run
//! against the live `(store, root)` and, at M08.8, a `FrozenWorld` served purely
//! from a serialized Basis — byte-identically (v4 §42, §24 "replayable").
//!
//! Effects stay OUT (v4 §8.6): observations enter through the reactor as graph
//! transactions; queries only ever read.

use std::fmt::Write as _;

use liminal_graph::ns::SYS_BLOB;
use liminal_graph::{GraphStore, StateView, kind};
use liminal_id::{GraphRevisionId, JurisdictionKey, NodeId, PathId, RelationId};
use liminal_query::Query;
use liminal_revision::{BasisComponent, BasisPerspective, ComponentDeps, WorkspaceBasis, graph_key};

/// The read surface a query runs against. Both the live workspace and the
/// frozen-basis replay world (M08.8) implement it, so a query cannot tell which
/// it is reading from — that is the determinism guarantee (v4 §24).
pub trait World {
    /// Graph state materialized as of `rev` (an as-of read, M08.1).
    fn graph_at(&self, rev: GraphRevisionId) -> StateView;

    /// The perspective-resolved text of `path`: the buffer-generation blob when
    /// `component` is a [`BasisComponent::BufferGeneration`], else the durable
    /// file bytes. `component` is exactly `basis.components[Path(path)]`.
    fn resolved_text(&self, path: &PathId, component: Option<&BasisComponent>) -> String;

    /// Every durable file path in the workspace (`SYS_BLOB["file/<path>"]`).
    fn file_paths(&self) -> Vec<PathId>;
}

/// The live `(store, root)` world.
#[derive(Debug)]
pub struct LiveWorld<'w> {
    store: &'w GraphStore,
    root: &'w camino::Utf8Path,
}

impl<'w> LiveWorld<'w> {
    /// A live world over an open workspace's store and root.
    #[must_use]
    pub fn new(store: &'w GraphStore, root: &'w camino::Utf8Path) -> Self {
        Self { store, root }
    }
}

impl World for LiveWorld<'_> {
    fn graph_at(&self, rev: GraphRevisionId) -> StateView {
        self.store
            .state_at(rev)
            .expect("as-of read of a revision <= head cannot fail in the toy")
    }

    fn resolved_text(&self, path: &PathId, component: Option<&BasisComponent>) -> String {
        match component {
            Some(BasisComponent::BufferGeneration {
                client,
                buffer,
                generation,
                ..
            }) => {
                let blob_key = format!("buf/{client}/{buffer}/{generation}");
                self.store
                    .get_aux(SYS_BLOB, &blob_key)
                    .ok()
                    .flatten()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            }
            _ => std::fs::read_to_string(self.root.join(&path.0)).unwrap_or_default(),
        }
    }

    fn file_paths(&self) -> Vec<PathId> {
        self.store
            .scan_aux(SYS_BLOB)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, _)| k.strip_prefix("file/").map(|p| PathId(p.into())))
            .collect()
    }
}

/// The D08.3 provenance string embedded by every query — `client-scoped:<name>`
/// / `durable-only` / `published` / `federated`. The caller supplies the human
/// client name (the toy's "neovim"/"phone"); no `ClientId → name` map exists in
/// Phase -1, and the scenario asserts the exact string.
#[must_use]
pub fn perspective_label(perspective: &BasisPerspective, client_name: Option<&str>) -> String {
    match perspective {
        BasisPerspective::ClientScoped { .. } => {
            format!("client-scoped:{}", client_name.unwrap_or("unknown"))
        }
        BasisPerspective::DurableOnly => "durable-only".to_owned(),
        BasisPerspective::Published { .. } => "published".to_owned(),
        BasisPerspective::Federated { .. } => "federated".to_owned(),
    }
}

/// Record `key` as a dependency, debug-asserting it is present in the basis
/// (D08.2: every byte a query reads comes from a declared component).
fn require(basis: &WorkspaceBasis, deps: &mut ComponentDeps, key: JurisdictionKey) {
    debug_assert!(
        basis.components.contains_key(&key),
        "D08.2: query read component {key:?} absent from the resolved basis"
    );
    deps.record(key);
}

/// The graph revision the basis pins at [`graph_key`], recording it as a dep.
fn graph_revision(basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> Option<GraphRevisionId> {
    require(basis, deps, graph_key());
    match basis.components.get(&graph_key()) {
        Some(BasisComponent::GraphSnapshot { revision }) => Some(*revision),
        _ => None,
    }
}

/// The `{#id}` alias a graph records for `node` (scans `JUR_ALIAS`), if any.
fn alias_of(view: &StateView, node: NodeId) -> Option<String> {
    let want = node.to_string();
    view.scan_aux(liminal_graph::ns::JUR_ALIAS)
        .find(|(_, v)| v.get("node").and_then(|n| n.as_str()) == Some(want.as_str()))
        .map(|(alias, _)| alias.clone())
}

/// The durable payload text of a node (empty for non-text payloads).
fn node_text(view: &StateView, node: NodeId) -> String {
    match view.node(node).map(|n| &n.payload) {
        Some(liminal_graph::PayloadRef::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// The single FILE node that contains `node` as an ordered child (toy: exactly
/// one file, so any tracked paragraph resolves to it).
fn owning_file_path<W: World>(world: &W, view: &StateView, node: NodeId) -> Option<PathId> {
    let owns = view
        .nodes()
        .any(|n| n.kind == kind::FILE && view.children(n.id).contains(&node));
    if !owns {
        return None;
    }
    world.file_paths().into_iter().next()
}

/// The block text for `node` in `text`: the block whose `{#id}` marker equals
/// the node's alias, falling back to the node's durable payload.
fn block_text_for(view: &StateView, text: &str, node: NodeId, alias: Option<&str>) -> String {
    if let Some(a) = alias
        && let Some(block) = liminal_source::paragraph::parse(text)
            .into_iter()
            .find(|b| b.id.as_deref() == Some(a))
    {
        return block.text;
    }
    node_text(view, node)
}

/// `render_block(node)` (v4 §24): the perspective-selected text of the block,
/// as `"{alias}: {text}\n"` (alias omitted when none), behind a `# perspective:`
/// provenance line (D08.3).
pub struct RenderBlock<'w, W: World> {
    /// The read world.
    pub world: &'w W,
    /// The block node to render.
    pub node: NodeId,
    /// The D08.3 provenance label.
    pub perspective_label: String,
}

impl<W: World> std::fmt::Debug for RenderBlock<'_, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderBlock")
            .field("node", &self.node)
            .field("perspective_label", &self.perspective_label)
            .finish_non_exhaustive()
    }
}

impl<W: World> Query for RenderBlock<'_, W> {
    type Output = String;

    fn execute(&self, basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> String {
        let mut out = format!("# perspective: {}\n", self.perspective_label);
        let Some(rev) = graph_revision(basis, deps) else {
            return out;
        };
        let view = self.world.graph_at(rev);
        let alias = alias_of(&view, self.node);
        let Some(path) = owning_file_path(self.world, &view, self.node) else {
            return out;
        };
        let key = JurisdictionKey::Path(path.clone());
        require(basis, deps, key.clone());
        let text = self.world.resolved_text(&path, basis.components.get(&key));
        let block = block_text_for(&view, &text, self.node, alias.as_deref());
        match alias {
            Some(a) => {
                let _ = writeln!(out, "{a}: {block}");
            }
            None => {
                let _ = writeln!(out, "{block}");
            }
        }
        out
    }
}

/// `backlinks(node)` (v4 §24): the ids of every Relation targeting `node` at the
/// basis graph revision, sorted, one per line, behind a `# perspective:` line.
pub struct Backlinks<'w, W: World> {
    /// The read world.
    pub world: &'w W,
    /// The node whose inbound relations are listed.
    pub node: NodeId,
    /// The D08.3 provenance label.
    pub perspective_label: String,
}

impl<W: World> std::fmt::Debug for Backlinks<'_, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backlinks")
            .field("node", &self.node)
            .field("perspective_label", &self.perspective_label)
            .finish_non_exhaustive()
    }
}

impl<W: World> Query for Backlinks<'_, W> {
    type Output = String;

    fn execute(&self, basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> String {
        let mut out = format!("# perspective: {}\n", self.perspective_label);
        let Some(rev) = graph_revision(basis, deps) else {
            return out;
        };
        let view = self.world.graph_at(rev);
        let mut ids: Vec<RelationId> = view
            .relations()
            .filter(|r| r.target.node() == self.node)
            .map(|r| r.id)
            .collect();
        ids.sort();
        for id in ids {
            let _ = writeln!(out, "{id}");
        }
        out
    }
}

/// `workspace_export()` (v4 §24): every file's blocks re-emitted from durable
/// graph state in child order with `{#id}` markers, `\n\n`-separated, behind a
/// `# perspective:` line. Reads graph payloads (not buffers) — a DurableOnly
/// export contains no dirty text.
pub struct WorkspaceExport<'w, W: World> {
    /// The read world.
    pub world: &'w W,
    /// The D08.3 provenance label.
    pub perspective_label: String,
}

impl<W: World> std::fmt::Debug for WorkspaceExport<'_, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceExport")
            .field("perspective_label", &self.perspective_label)
            .finish_non_exhaustive()
    }
}

impl<W: World> Query for WorkspaceExport<'_, W> {
    type Output = String;

    fn execute(&self, basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> String {
        let mut out = format!("# perspective: {}\n", self.perspective_label);
        let Some(rev) = graph_revision(basis, deps) else {
            return out;
        };
        let view = self.world.graph_at(rev);
        let mut blocks: Vec<String> = Vec::new();
        for path in self.world.file_paths() {
            require(basis, deps, JurisdictionKey::Path(path.clone()));
            // Toy: one FILE node holds every paragraph.
            let Some(file) = view.nodes().find(|n| n.kind == kind::FILE) else {
                continue;
            };
            for &child in view.children(file.id) {
                let text = node_text(&view, child);
                match alias_of(&view, child) {
                    Some(a) => blocks.push(format!("{text} {{#{a}}}")),
                    None => blocks.push(text),
                }
            }
        }
        out.push_str(&blocks.join("\n\n"));
        out.push('\n');
        out
    }
}

/// `ai_context_stub(focus)` (v4 §24, §7.5): a canonical-JSON context bundle for
/// an AI consumer — the focus block's perspective-selected text, its sibling
/// blocks as neighbors, and the selected mode embedded in `basis.perspective`
/// (D08.3: the provenance rides INSIDE the JSON, not a comment line).
pub struct AiContextStub<'w, W: World> {
    /// The read world.
    pub world: &'w W,
    /// The focus block node.
    pub focus: NodeId,
    /// The D08.3 provenance label (embedded in the JSON).
    pub perspective_label: String,
}

impl<W: World> std::fmt::Debug for AiContextStub<'_, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiContextStub")
            .field("focus", &self.focus)
            .field("perspective_label", &self.perspective_label)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, serde::Serialize)]
struct AiContext {
    focus: String,
    text: String,
    neighbors: Vec<String>,
    basis: AiBasis,
}

#[derive(Debug, serde::Serialize)]
struct AiBasis {
    transaction: String,
    perspective: String,
}

impl<W: World> Query for AiContextStub<'_, W> {
    type Output = String;

    fn execute(&self, basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> String {
        let Some(rev) = graph_revision(basis, deps) else {
            return "{}".to_owned();
        };
        let view = self.world.graph_at(rev);
        let alias = alias_of(&view, self.focus);

        let (text, neighbors) = match owning_file_path(self.world, &view, self.focus) {
            Some(path) => {
                let key = JurisdictionKey::Path(path.clone());
                require(basis, deps, key.clone());
                let resolved = self.world.resolved_text(&path, basis.components.get(&key));
                let text = block_text_for(&view, &resolved, self.focus, alias.as_deref());
                // Neighbors: sibling blocks' durable text, focus excluded.
                let neighbors = view
                    .nodes()
                    .find(|n| n.kind == kind::FILE)
                    .map(|file| {
                        view.children(file.id)
                            .iter()
                            .filter(|&&c| c != self.focus)
                            .map(|&c| node_text(&view, c))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                (text, neighbors)
            }
            None => (node_text(&view, self.focus), Vec::new()),
        };

        let ctx = AiContext {
            focus: self.focus.to_string(),
            text,
            neighbors,
            basis: AiBasis {
                transaction: basis.transaction.to_string(),
                perspective: self.perspective_label.clone(),
            },
        };
        serde_json::to_string(&ctx).unwrap_or_else(|_| "{}".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use liminal_graph::ns::JUR_ALIAS;
    use liminal_id::ClientId;

    use super::*;
    use crate::scenario::{SetupFile, SetupGraph};
    use crate::workspace::ToyWorkspace;

    const NOTES: &str = "\
The Fourier transform decomposes a signal
into its constituent frequencies. {#p-fourier}

The Laplace transform generalizes it
to the complex plane. {#p-laplace}
";

    fn tmp_root(name: &str) -> camino::Utf8PathBuf {
        let dir = camino::Utf8PathBuf::from(std::env::temp_dir().to_str().unwrap()).join(format!(
            "liminal-queries-{name}-{}-{:x}",
            std::process::id(),
            liminal_id::Timestamp::now().0
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Build a workspace with NOTES ingested and a comment Relation on
    /// p-fourier. Ingest happens on a first handle, then we drop it and reopen —
    /// `seed_durable_inputs` runs at open, so the reopened workspace's input map
    /// (which every Basis resolves from) reflects the ingested file.
    fn workspace(name: &str) -> (camino::Utf8PathBuf, ToyWorkspace) {
        let root = tmp_root(name);
        std::fs::write(root.join("notes.md"), NOTES).unwrap();
        {
            let ws = ToyWorkspace::open(&root).unwrap();
            let files = vec![SetupFile {
                path: "notes.md".into(),
                text: NOTES.into(),
            }];
            crate::runner::ingest_files(ws.store(), &files).unwrap();
            let graph = vec![SetupGraph {
                kind: "comment-relation".into(),
                target: Some("p-fourier".into()),
                requires_grade: Some("explicit".into()),
                extra: toml::toml! { body = "see also" },
            }];
            crate::runner::ingest_graph(ws.store(), &graph).unwrap();
        }
        let ws = ToyWorkspace::open(&root).unwrap();
        (root, ws)
    }

    fn node_for(ws: &ToyWorkspace, alias: &str) -> NodeId {
        ws.store()
            .get_aux(JUR_ALIAS, alias)
            .unwrap()
            .unwrap()["node"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn render_block_durable_carries_perspective_and_text() {
        let (_root, ws) = workspace("render-durable");
        let node = node_for(&ws, "p-fourier");
        let basis = ws.basis(BasisPerspective::DurableOnly).unwrap();
        let world = LiveWorld::new(ws.store(), ws.root());
        let q = RenderBlock {
            world: &world,
            node,
            perspective_label: "durable-only".into(),
        };
        let mut deps = ComponentDeps::default();
        let out = q.execute(&basis, &mut deps);
        assert!(out.starts_with("# perspective: durable-only\n"), "{out}");
        assert!(out.contains("p-fourier: "), "{out}");
        assert!(out.contains("constituent frequencies."), "{out}");
        // D08.2: recorded both the file path and the graph revision.
        assert!(deps.read.contains(&graph_key()));
        assert!(
            deps.read
                .contains(&JurisdictionKey::Path(PathId("notes.md".into())))
        );
    }

    #[test]
    fn render_block_client_scoped_sees_own_buffer() {
        let (_root, mut ws) = workspace("render-client");
        let node = node_for(&ws, "p-fourier");
        let client = ClientId::new();
        let buffer = {
            let mut session = ws.client(client);
            let b = session.open_buffer(PathId("notes.md".into()));
            session.edit(
                b,
                "NEOVIM-EDIT decomposes a signal\ninto its constituent frequencies. {#p-fourier}\n\nThe Laplace transform generalizes it\nto the complex plane. {#p-laplace}\n",
            );
            b
        };
        let _ = buffer;
        let basis = ws
            .basis(BasisPerspective::ClientScoped { client })
            .unwrap();
        let world = LiveWorld::new(ws.store(), ws.root());
        let q = RenderBlock {
            world: &world,
            node,
            perspective_label: "client-scoped:neovim".into(),
        };
        let mut deps = ComponentDeps::default();
        let out = q.execute(&basis, &mut deps);
        assert!(out.contains("NEOVIM-EDIT"), "buffer edit not visible: {out}");
        assert!(out.contains("client-scoped:neovim"), "{out}");
    }

    #[test]
    fn backlinks_lists_the_comment_relation() {
        let (_root, ws) = workspace("backlinks");
        let node = node_for(&ws, "p-fourier");
        let basis = ws.basis(BasisPerspective::DurableOnly).unwrap();
        let world = LiveWorld::new(ws.store(), ws.root());
        let q = Backlinks {
            world: &world,
            node,
            perspective_label: "durable-only".into(),
        };
        let mut deps = ComponentDeps::default();
        let out = q.execute(&basis, &mut deps);
        // Exactly one comment Relation targets p-fourier.
        let rel = ws
            .store()
            .relations()
            .unwrap()
            .into_iter()
            .find(|r| r.target.node() == node)
            .unwrap();
        assert!(out.contains(&rel.id.to_string()), "{out}");
        assert!(out.starts_with("# perspective: durable-only\n"));
    }

    #[test]
    fn workspace_export_is_durable_and_dirty_free() {
        let (_root, mut ws) = workspace("export");
        // A dirty buffer must NOT leak into a durable export.
        let client = ClientId::new();
        {
            let mut session = ws.client(client);
            let b = session.open_buffer(PathId("notes.md".into()));
            session.edit(b, "DIRTY {#p-fourier}\n");
        }
        let basis = ws.basis(BasisPerspective::DurableOnly).unwrap();
        let world = LiveWorld::new(ws.store(), ws.root());
        let q = WorkspaceExport {
            world: &world,
            perspective_label: "durable-only".into(),
        };
        let mut deps = ComponentDeps::default();
        let out = q.execute(&basis, &mut deps);
        assert!(out.contains("{#p-fourier}"), "{out}");
        assert!(out.contains("{#p-laplace}"), "{out}");
        assert!(!out.contains("DIRTY"), "dirty buffer leaked into export: {out}");
    }

    #[test]
    fn ai_context_embeds_perspective_in_json() {
        let (_root, mut ws) = workspace("ai");
        let node = node_for(&ws, "p-fourier");
        let client = ClientId::new();
        {
            let mut session = ws.client(client);
            let b = session.open_buffer(PathId("notes.md".into()));
            session.edit(
                b,
                "PHONE-EDIT frequencies. {#p-fourier}\n\nThe Laplace transform generalizes it\nto the complex plane. {#p-laplace}\n",
            );
        }
        let basis = ws
            .basis(BasisPerspective::ClientScoped { client })
            .unwrap();
        let world = LiveWorld::new(ws.store(), ws.root());
        let q = AiContextStub {
            world: &world,
            focus: node,
            perspective_label: "client-scoped:phone".into(),
        };
        let mut deps = ComponentDeps::default();
        let out = q.execute(&basis, &mut deps);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["basis"]["perspective"], "client-scoped:phone");
        assert!(
            parsed["text"].as_str().unwrap().contains("PHONE-EDIT"),
            "{out}"
        );
        assert!(parsed["neighbors"].is_array());
    }
}
