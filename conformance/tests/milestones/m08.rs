//! M08.7/M08.8 exit-gate born-passing tests.
//!
//! `four_consumers_two_dirty_buffers` (M08.7): four consumers, two dirty
//! buffers, one materialized observation — each consumer selects its OWN
//! perspective explicitly (v4 §7.5's provenance rule) and sees exactly what
//! that perspective promises, never another client's intent (v4 §24).
//!
//! `replay_from_frozen_basis_is_deterministic` (M08.8, Algorithm C): the same
//! four queries, re-executed against a [`FrozenWorld`] loaded from frozen
//! JSON after the workspace directory is deleted, reproduce byte-identical
//! output (v4 §42, §112).

use liminal_conformance::harness::{ToyRun, all_scenarios};
use liminal_conformance::replay::{FrozenWorld, freeze};
use liminal_daemon::ToyWorkspace;
use liminal_daemon::queries::{
    AiContextStub, Backlinks, LiveWorld, RenderBlock, WorkspaceExport, World, perspective_label,
};
use liminal_id::{
    BufferId, ClientId, ContentHash, JurisdictionKey, NodeId, PathId, SessionEpoch, SourceId,
    Timestamp,
};
use liminal_query::Query;
use liminal_query::memo::MemoTable;
use liminal_revision::{BasisPerspective, ComponentDeps, WorkspaceBasis};

const NEOVIM_TEXT: &str = "NEOVIM-EDIT decomposes a signal\ninto its constituent frequencies. {#p-fourier}\n\nThe Laplace transform generalizes it\nto the complex plane. {#p-laplace}\n\nAn anonymous paragraph with no durable id.\n";
const PHONE_TEXT: &str = "The Fourier transform decomposes a signal\nPHONE-EDIT constituent frequencies. {#p-fourier}\n\nThe Laplace transform generalizes it\nto the complex plane. {#p-laplace}\n\nAn anonymous paragraph with no durable id.\n";
const PHONE_TEXT_2: &str = "The Fourier transform decomposes a signal\nPHONE-EDIT-2 constituent frequencies, again. {#p-fourier}\n\nThe Laplace transform generalizes it\nto the complex plane. {#p-laplace}\n\nAn anonymous paragraph with no durable id.\n";

/// The node a `{#alias}` durable id names (`JUR_ALIAS[alias]["node"]`) —
/// mirrors `runner::node_for_alias`, which is private to its crate.
fn node_for(ws: &ToyWorkspace, alias: &str) -> NodeId {
    ws.store()
        .get_aux(liminal_graph::ns::JUR_ALIAS, alias)
        .expect("scan JUR_ALIAS")
        .expect("alias must exist")["node"]
        .as_str()
        .expect("node field must be a string")
        .parse()
        .expect("node field must parse as a NodeId")
}

fn runner_query_output(ws: &ToyWorkspace, query: &str, perspective: &str) -> String {
    ws.store()
        .get_aux(
            liminal_graph::ns::SYS_BLOB,
            &format!("query/{query}:{perspective}"),
        )
        .expect("read runner query output")
        .unwrap_or_else(|| panic!("runner persisted {query}:{perspective}"))
        .as_str()
        .expect("runner query output is text")
        .to_owned()
}

/// Open + edit both dirty clients directly (exactly as `queries.rs`'s own
/// unit tests do — the scenario's own `buffer_edit`/`query` steps only
/// mutate runner scratch bytes and step-local sessions. Their canonical query
/// outputs persist in `SYS_BLOB`; their working buffer claims do not survive
/// the post-scenario workspace reopen used by this test).
fn open_dirty_clients(ws: &mut ToyWorkspace) -> (ClientId, ClientId, BufferId) {
    let neovim = ClientId::new();
    {
        let mut session = ws.client(neovim);
        let b = session.open_buffer(PathId("notes.md".into()));
        session.edit(b, NEOVIM_TEXT);
    }
    let phone = ClientId::new();
    let phone_buffer = {
        let mut session = ws.client(phone);
        let b = session.open_buffer(PathId("notes.md".into()));
        session.edit(b, PHONE_TEXT);
        b
    };
    (neovim, phone, phone_buffer)
}

/// Consumer 1 (interactive preview) + consumer 2 (AI context build): each
/// dirty client sees its OWN text and never the other's.
fn assert_dirty_consumers(
    world: &LiveWorld<'_>,
    node: NodeId,
    neovim_basis: &WorkspaceBasis,
    phone_basis: &WorkspaceBasis,
    neovim_label: &str,
    phone_label: &str,
) -> (String, String) {
    let path_key = JurisdictionKey::Path(PathId("notes.md".into()));
    let BasisPerspective::ClientScoped {
        client: neovim_client,
    } = neovim_basis.perspective
    else {
        panic!("preview basis must be client-scoped");
    };
    let liminal_revision::BasisComponent::BufferGeneration {
        client: captured_client,
        ..
    } = neovim_basis
        .components
        .get(&path_key)
        .expect("preview basis captures notes.md")
    else {
        panic!("preview basis must capture a buffer generation");
    };
    assert_eq!(
        *captured_client, neovim_client,
        "shared Path dependency must capture neovim's component, never phone's"
    );

    let mut deps = ComponentDeps::default();
    let preview = RenderBlock {
        world,
        node,
        perspective_label: neovim_label.to_owned(),
    }
    .execute(neovim_basis, &mut deps);
    assert!(
        preview.starts_with("# perspective: client-scoped:neovim\n"),
        "{preview}"
    );
    assert!(preview.contains("NEOVIM-EDIT"), "{preview}");
    assert!(!preview.contains("PHONE-EDIT"), "{preview}");
    assert!(
        deps.read.contains(&path_key),
        "render_block must record its path dependency: {:?}",
        deps.read
    );

    let mut deps = ComponentDeps::default();
    let ai_context = AiContextStub {
        world,
        focus: node,
        perspective_label: phone_label.to_owned(),
    }
    .execute(phone_basis, &mut deps);
    let ai_json: serde_json::Value = serde_json::from_str(&ai_context).expect("ai context is JSON");
    assert_eq!(ai_json["basis"]["perspective"], "client-scoped:phone");
    let ai_text = ai_json["text"].as_str().expect("text field");
    assert!(ai_text.contains("PHONE-EDIT"), "{ai_text}");
    assert!(!ai_text.contains("NEOVIM-EDIT"), "{ai_text}");

    (preview, ai_context)
}

/// Consumer 3 (cross-document query, explicit `DurableOnly`): the comment
/// Relation on p-fourier is visible, behind the `durable-only` provenance line.
fn assert_backlinks_consumer(
    ws: &ToyWorkspace,
    world: &LiveWorld<'_>,
    node: NodeId,
    durable_basis: &WorkspaceBasis,
    durable_label: &str,
) -> String {
    let mut deps = ComponentDeps::default();
    let backlinks = Backlinks {
        world,
        node,
        perspective_label: durable_label.to_owned(),
    }
    .execute(durable_basis, &mut deps);
    assert!(
        backlinks.starts_with("# perspective: durable-only\n"),
        "{backlinks}"
    );
    let comment_relation = ws
        .store()
        .relations()
        .expect("relations")
        .into_iter()
        .find(|r| r.target.node() == node)
        .expect("comment relation on p-fourier must exist");
    assert!(
        backlinks.contains(&comment_relation.id.to_string()),
        "backlinks must list the comment relation: {backlinks}"
    );
    backlinks
}

/// Consumer 4 (durable export): byte-equal to an independent re-emission
/// (the query is a pure function of its Basis, v4 §24); no dirty edit leaks
/// in.
fn assert_export_consumer(ws: &ToyWorkspace, world: &LiveWorld<'_>, durable_label: &str) -> String {
    let durable_basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");
    let mut deps = ComponentDeps::default();
    let export = WorkspaceExport {
        world,
        perspective_label: durable_label.to_owned(),
    }
    .execute(&durable_basis, &mut deps);
    assert!(
        export.starts_with("# perspective: durable-only\n"),
        "{export}"
    );
    assert!(!export.contains("NEOVIM-EDIT"), "{export}");
    assert!(!export.contains("PHONE-EDIT"), "{export}");

    let re_emission_basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");
    let mut re_emission_deps = ComponentDeps::default();
    let re_emission = WorkspaceExport {
        world,
        perspective_label: durable_label.to_owned(),
    }
    .execute(&re_emission_basis, &mut re_emission_deps);
    assert_eq!(
        export, re_emission,
        "durable export must be byte-equal to a re-emission"
    );
    export
}

/// One pass over all four consumers through `table`.
#[allow(
    clippy::too_many_arguments,
    reason = "test glue over the four consumer queries"
)]
fn run_all_four(
    table: &mut MemoTable,
    world: &LiveWorld<'_>,
    node: NodeId,
    neovim_basis: &WorkspaceBasis,
    phone_basis: &WorkspaceBasis,
    durable_basis: &WorkspaceBasis,
    neovim_label: &str,
    phone_label: &str,
    durable_label: &str,
) {
    table.execute(
        "render_block",
        neovim_label,
        &RenderBlock {
            world,
            node,
            perspective_label: neovim_label.to_owned(),
        },
        neovim_basis,
    );
    table.execute(
        "ai_context_stub",
        phone_label,
        &AiContextStub {
            world,
            focus: node,
            perspective_label: phone_label.to_owned(),
        },
        phone_basis,
    );
    table.execute(
        "backlinks",
        durable_label,
        &Backlinks {
            world,
            node,
            perspective_label: durable_label.to_owned(),
        },
        durable_basis,
    );
    table.execute(
        "workspace_export",
        durable_label,
        &WorkspaceExport {
            world,
            perspective_label: durable_label.to_owned(),
        },
        durable_basis,
    );
}

/// Memoization: after a further phone edit, only the phone-scoped entry
/// invalidates (component-granular invalidation, D06.6).
#[allow(
    clippy::too_many_arguments,
    reason = "test glue over the four consumer queries"
)]
fn assert_only_phone_entry_invalidates(
    ws: &mut ToyWorkspace,
    node: NodeId,
    phone_buffer: BufferId,
    phone: ClientId,
    neovim_basis: &WorkspaceBasis,
    phone_basis: &WorkspaceBasis,
    durable_basis: &WorkspaceBasis,
    neovim_label: &str,
    phone_label: &str,
    durable_label: &str,
) {
    let mut table = MemoTable::new();
    {
        let world = LiveWorld::new(ws.store(), ws.root());
        run_all_four(
            &mut table,
            &world,
            node,
            neovim_basis,
            phone_basis,
            durable_basis,
            neovim_label,
            phone_label,
            durable_label,
        );
    }
    assert_eq!(table.misses(), 4, "first pass populates all four entries");
    assert_eq!(table.hits(), 0);

    // A further phone edit bumps phone's BufferGeneration content hash; every
    // OTHER perspective's resolved components are untouched (D06.2:
    // ClientScoped never sees another client's buffer; DurableOnly ignores
    // all working claims).
    {
        let mut session = ws.client(phone);
        session.edit(phone_buffer, PHONE_TEXT_2);
    }
    let phone_basis_2 = ws
        .basis(BasisPerspective::ClientScoped { client: phone })
        .expect("phone basis after further edit");

    let world = LiveWorld::new(ws.store(), ws.root());
    run_all_four(
        &mut table,
        &world,
        node,
        neovim_basis,
        &phone_basis_2,
        durable_basis,
        neovim_label,
        phone_label,
        durable_label,
    );
    assert_eq!(
        table.misses(),
        5,
        "only the phone-scoped entry recomputes after the further phone edit"
    );
    assert_eq!(
        table.hits(),
        3,
        "neovim, backlinks, and workspace_export all stay fresh"
    );
}

/// The four consumers, two dirty clients, one observation gate (M08.7): each
/// consumer picks its perspective explicitly and sees exactly what that
/// perspective promises.
///
/// "deps contain neovim's key, NOT phone's" (the exit-gate table's phrasing)
/// is read as: `JurisdictionKey` has no per-client variant (a buffer over a
/// path is keyed by `Path(path)` regardless of which client holds it — the
/// client lives inside the resolved `BasisComponent`, not the key) — so the
/// observable contract is that the query records a dependency on the shared
/// path key at all (`deps.read` is how memoization invalidates precisely),
/// while the RESOLVED COMPONENT — and hence the rendered text — is
/// perspective-specific. That is what is asserted here, plus the direct
/// non-leak checks (contains A not B, contains B not A).
#[test]
fn four_consumers_two_dirty_buffers() {
    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "four_consumers")
        .expect("four_consumers fixture must exist");

    let run = ToyRun::new("m08-four-consumers").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "capture never rejected (Law 3B); stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    let mut ws = ToyWorkspace::open(&run.root).expect("open");
    let node = node_for(&ws, "p-fourier");
    let runner_preview = runner_query_output(&ws, "render_block", "client-scoped:neovim");
    assert!(runner_preview.contains("NEOVIM-EDIT"), "{runner_preview}");
    assert!(!runner_preview.contains("PHONE-EDIT"), "{runner_preview}");
    let runner_ai = runner_query_output(&ws, "ai_context_stub", "client-scoped:phone");
    let runner_ai_json: serde_json::Value =
        serde_json::from_str(&runner_ai).expect("runner AI context is JSON");
    assert_eq!(
        runner_ai_json["basis"]["perspective"],
        "client-scoped:phone"
    );
    let runner_ai_text = runner_ai_json["text"]
        .as_str()
        .expect("runner AI context text");
    assert!(runner_ai_text.contains("PHONE-EDIT"), "{runner_ai_text}");
    assert!(!runner_ai_text.contains("NEOVIM-EDIT"), "{runner_ai_text}");
    let runner_backlinks = runner_query_output(&ws, "backlinks", "durable-only");
    let runner_export = runner_query_output(&ws, "workspace_export", "durable-only");
    assert!(!runner_export.contains("NEOVIM-EDIT"), "{runner_export}");
    assert!(!runner_export.contains("PHONE-EDIT"), "{runner_export}");

    let (neovim, phone, phone_buffer) = open_dirty_clients(&mut ws);

    let neovim_label = perspective_label(
        &BasisPerspective::ClientScoped { client: neovim },
        Some("neovim"),
    );
    let phone_label = perspective_label(
        &BasisPerspective::ClientScoped { client: phone },
        Some("phone"),
    );
    let durable_label = perspective_label(&BasisPerspective::DurableOnly, None);
    assert_eq!(neovim_label, "client-scoped:neovim");
    assert_eq!(phone_label, "client-scoped:phone");
    assert_eq!(durable_label, "durable-only");

    let neovim_basis = ws
        .basis(BasisPerspective::ClientScoped { client: neovim })
        .expect("neovim basis");
    let phone_basis = ws
        .basis(BasisPerspective::ClientScoped { client: phone })
        .expect("phone basis");
    let durable_basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");

    let world = LiveWorld::new(ws.store(), ws.root());
    let (preview, ai_context) = assert_dirty_consumers(
        &world,
        node,
        &neovim_basis,
        &phone_basis,
        &neovim_label,
        &phone_label,
    );
    let backlinks = assert_backlinks_consumer(&ws, &world, node, &durable_basis, &durable_label);
    let export = assert_export_consumer(&ws, &world, &durable_label);

    assert_eq!(
        runner_preview, preview,
        "runner preview must use query seam"
    );
    assert_eq!(
        runner_backlinks, backlinks,
        "runner backlinks must use query seam"
    );
    assert_eq!(runner_export, export, "runner export must use query seam");

    // The three text-bearing outputs are pairwise distinct.
    assert_ne!(preview, ai_context);
    assert_ne!(preview, export);
    assert_ne!(ai_context, export);

    assert_only_phone_entry_invalidates(
        &mut ws,
        node,
        phone_buffer,
        phone,
        &neovim_basis,
        &phone_basis,
        &durable_basis,
        &neovim_label,
        &phone_label,
        &durable_label,
    );
}

/// The four consumers' outputs over one `World` (live or frozen).
struct FourOutputs {
    preview: String,
    ai_context: String,
    backlinks: String,
    export: String,
}

#[allow(
    clippy::too_many_arguments,
    reason = "test glue over the four consumer queries"
)]
fn run_four<W: World>(
    world: &W,
    node: NodeId,
    neovim_basis: &WorkspaceBasis,
    phone_basis: &WorkspaceBasis,
    durable_basis: &WorkspaceBasis,
    neovim_label: &str,
    phone_label: &str,
    durable_label: &str,
) -> FourOutputs {
    let mut deps = ComponentDeps::default();
    let preview = RenderBlock {
        world,
        node,
        perspective_label: neovim_label.to_owned(),
    }
    .execute(neovim_basis, &mut deps);

    let mut deps = ComponentDeps::default();
    let ai_context = AiContextStub {
        world,
        focus: node,
        perspective_label: phone_label.to_owned(),
    }
    .execute(phone_basis, &mut deps);

    let mut deps = ComponentDeps::default();
    let backlinks = Backlinks {
        world,
        node,
        perspective_label: durable_label.to_owned(),
    }
    .execute(durable_basis, &mut deps);

    let mut deps = ComponentDeps::default();
    let export = WorkspaceExport {
        world,
        perspective_label: durable_label.to_owned(),
    }
    .execute(durable_basis, &mut deps);

    FourOutputs {
        preview,
        ai_context,
        backlinks,
        export,
    }
}

/// M08.8 Algorithm C, exit-gate: run the scenario, capture the four outputs
/// live, freeze each consulted Basis, DELETE the workspace directory, then
/// re-execute the same four queries against `FrozenWorld` — byte-identical
/// output is the final-gate requirement (v4 §42, §112: "resolver/query
/// replay with frozen inputs is deterministic").
#[test]
fn replay_from_frozen_basis_is_deterministic() {
    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "four_consumers")
        .expect("four_consumers fixture must exist");

    let run = ToyRun::new("m08-replay").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "capture never rejected (Law 3B); stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    let mut ws = ToyWorkspace::open(&run.root).expect("open");
    let node = node_for(&ws, "p-fourier");
    let (neovim, phone, _phone_buffer) = open_dirty_clients(&mut ws);

    let neovim_label = perspective_label(
        &BasisPerspective::ClientScoped { client: neovim },
        Some("neovim"),
    );
    let phone_label = perspective_label(
        &BasisPerspective::ClientScoped { client: phone },
        Some("phone"),
    );
    let durable_label = perspective_label(&BasisPerspective::DurableOnly, None);

    let neovim_basis = ws
        .basis(BasisPerspective::ClientScoped { client: neovim })
        .expect("neovim basis");
    let phone_basis = ws
        .basis(BasisPerspective::ClientScoped { client: phone })
        .expect("phone basis");
    let durable_basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");

    let live = {
        let world = LiveWorld::new(ws.store(), ws.root());
        run_four(
            &world,
            node,
            &neovim_basis,
            &phone_basis,
            &durable_basis,
            &neovim_label,
            &phone_label,
            &durable_label,
        )
    };

    // Freeze each consulted Basis (durable serves both backlinks and export).
    let frozen_neovim = freeze(&neovim_basis, &ws).expect("freeze neovim basis");
    let frozen_phone = freeze(&phone_basis, &ws).expect("freeze phone basis");
    let frozen_durable = freeze(&durable_basis, &ws).expect("freeze durable basis");

    // Release the store's advisory lock (M05 AM-5.1's own drop pattern),
    // then delete the workspace directory entirely.
    drop(ws);
    std::fs::remove_dir_all(&run.root).expect("delete the workspace directory");
    assert!(!run.root.exists(), "workspace directory must be gone");

    let neovim_world = FrozenWorld::load(&frozen_neovim).expect("load frozen neovim world");
    let phone_world = FrozenWorld::load(&frozen_phone).expect("load frozen phone world");
    let durable_world = FrozenWorld::load(&frozen_durable).expect("load frozen durable world");

    // Each frozen basis was captured for exactly one consumer's query — run
    // only that one against its own frozen world (a `FrozenWorld` frozen for
    // one perspective carries no OTHER perspective's blobs to answer with).
    let replayed_preview = RenderBlock {
        world: &neovim_world,
        node,
        perspective_label: neovim_label.clone(),
    }
    .execute(neovim_world.basis(), &mut ComponentDeps::default());
    let replayed_ai_context = AiContextStub {
        world: &phone_world,
        focus: node,
        perspective_label: phone_label.clone(),
    }
    .execute(phone_world.basis(), &mut ComponentDeps::default());
    let replayed_backlinks = Backlinks {
        world: &durable_world,
        node,
        perspective_label: durable_label.clone(),
    }
    .execute(durable_world.basis(), &mut ComponentDeps::default());
    let replayed_export = WorkspaceExport {
        world: &durable_world,
        perspective_label: durable_label.clone(),
    }
    .execute(durable_world.basis(), &mut ComponentDeps::default());

    assert_eq!(
        live.preview, replayed_preview,
        "render_block must replay byte-identically from the frozen neovim basis"
    );
    assert_eq!(
        live.ai_context, replayed_ai_context,
        "ai_context_stub must replay byte-identically from the frozen phone basis"
    );
    assert_eq!(
        live.backlinks, replayed_backlinks,
        "backlinks must replay byte-identically from the frozen durable basis"
    );
    assert_eq!(
        live.export, replayed_export,
        "workspace_export must replay byte-identically from the frozen durable basis"
    );
}

/// DG-8.3 (T1 review): after `accept_repair` lands an `InsertSourceId` plan,
/// the `SYS_BLOB["file/<path>"]` mirror must hold the LANDED bytes — the
/// on-disk file with the inserted id — not a stale echo. The fixture's one
/// foreign edit strips the id AND appends a paragraph: candidate matching is
/// exact-text, so the target paragraph must stay byte-identical, while the
/// appended text makes the landed file differ from the ingest bytes (the
/// stock `dag_accept` pure-strip fixture restores the ORIGINAL bytes
/// verbatim, which cannot distinguish stale from fresh).
#[test]
fn file_blob_fresh_after_accept_repair() {
    use liminal_daemon::scenario::{
        Expectation, ScenarioHeader, ScenarioScript, Setup, SetupFile, SetupGraph, Step,
    };
    let mut foreign = toml::Table::new();
    foreign.insert("path".into(), toml::Value::String("notes.md".into()));
    foreign.insert("find".into(), toml::Value::String(" {#p-fourier}\n".into()));
    foreign.insert(
        "replace".into(),
        toml::Value::String("\n\nA foreign paragraph appended offline.\n".into()),
    );
    let script = ScenarioScript {
        scenario: ScenarioHeader {
            id: "m08_blob_fresh_accept".to_owned(),
            title: Some("reworded foreign edit + accepted DAG leaves fresh file blob".to_owned()),
            spec: vec!["M08.3".to_owned(), "DG-8.3".to_owned()],
            profiles: vec!["external-file".to_owned(), "graph-native".to_owned()],
        },
        setup: Setup {
            files: vec![SetupFile {
                path: "notes.md".to_owned(),
                text: "The Fourier transform decomposes a signal. {#p-fourier}\n".to_owned(),
            }],
            graph: vec![SetupGraph {
                kind: "comment-relation".to_owned(),
                target: Some("p-fourier".to_owned()),
                requires_grade: Some("explicit".to_owned()),
                extra: toml::Table::new(),
            }],
            buffers: Vec::new(),
        },
        steps: vec![
            Step {
                kind: "foreign_edit".to_owned(),
                extra: foreign,
            },
            Step {
                kind: "accept_repair".to_owned(),
                extra: toml::Table::new(),
            },
        ],
        expect: Expectation::default(),
    };

    let run = ToyRun::new("m08-blob-fresh").expect("workspace");
    let exec = run.exec_scenario(&script).expect("exec must run");
    assert!(
        exec.status.success(),
        "accepted DAG must run Committed; stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    let ws = ToyWorkspace::open(&run.root).expect("open");
    let disk = std::fs::read_to_string(run.root.join("notes.md")).expect("landed file");
    assert!(
        disk.contains("A foreign paragraph appended offline") && disk.contains("{#p-fourier}"),
        "acceptance must land the foreign-appended text WITH the re-inserted id: {disk:?}"
    );
    let blob = ws
        .store()
        .get_aux(liminal_graph::ns::SYS_BLOB, "file/notes.md")
        .expect("scan")
        .expect("file blob must exist");
    assert_eq!(
        blob.as_str().expect("file blob must be a string"),
        disk,
        "file blob must mirror the landed on-disk bytes, not a stale echo"
    );
}

/// T1 review of M08.8: `freeze` must refuse absent or hash-wrong bytes for
/// every Phase -1 byte-bearing component — a frozen payload must never
/// silently contradict its own Basis.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one frozen-byte invariant exercised across all Phase -1 component kinds"
)]
fn freeze_rejects_stale_file_bytes() {
    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "four_consumers")
        .expect("four_consumers fixture must exist");
    let run = ToyRun::new("m08-freeze-stale").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(exec.status.success());

    let mut ws = ToyWorkspace::open(&run.root).expect("open");
    let basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("durable basis");
    assert!(
        freeze(&basis, &ws).is_ok(),
        "untampered freeze must succeed"
    );

    let mut missing_buffer_basis = basis.clone();
    missing_buffer_basis.components.insert(
        JurisdictionKey::Path(PathId("notes.md".into())),
        liminal_revision::BasisComponent::BufferGeneration {
            client: ClientId::new(),
            buffer: BufferId::new(),
            epoch: SessionEpoch(1),
            generation: 99,
            content_hash: Some(ContentHash::of(b"missing")),
            base_file_hash: None,
        },
    );
    assert!(
        freeze(&missing_buffer_basis, &ws).is_err(),
        "freeze must reject a missing pinned buffer blob"
    );

    let missing_source = SourceId::from_name("missing:observation");
    let mut missing_observation_basis = basis.clone();
    missing_observation_basis.components.insert(
        JurisdictionKey::Source(missing_source),
        liminal_revision::BasisComponent::Observation {
            source: missing_source,
            observed_at: Timestamp(1),
            hash: ContentHash::of(b"missing"),
        },
    );
    assert!(
        freeze(&missing_observation_basis, &ws).is_err(),
        "freeze must reject a missing pinned observation blob"
    );

    let client = ClientId::new();
    let buffer = ws.client(client).open_buffer(PathId("notes.md".into()));
    let buffer_basis = ws
        .basis(BasisPerspective::ClientScoped { client })
        .expect("client basis");
    let buffer_component = buffer_basis
        .components
        .get(&JurisdictionKey::Path(PathId("notes.md".into())))
        .expect("buffer component");
    let liminal_revision::BasisComponent::BufferGeneration { generation, .. } = buffer_component
    else {
        panic!("client basis must select a buffer generation");
    };
    ws.store()
        .put_working_aux(
            liminal_graph::ns::SYS_BLOB,
            &format!("buf/{client}/{buffer}/{generation}"),
            serde_json::Value::String("tampered buffer".into()),
        )
        .expect("tamper buffer blob");
    assert!(
        freeze(&buffer_basis, &ws).is_err(),
        "freeze must reject buffer bytes that do not match content_hash"
    );

    let observation = basis
        .components
        .values()
        .find(|component| {
            matches!(
                component,
                liminal_revision::BasisComponent::Observation { .. }
            )
        })
        .expect("scenario basis contains observation");
    let liminal_revision::BasisComponent::Observation { source, hash, .. } = observation else {
        unreachable!();
    };
    ws.store()
        .put_working_aux(
            liminal_graph::ns::SYS_BLOB,
            &format!("obs/{source}/{hash}"),
            serde_json::json!({"price": "tampered"}),
        )
        .expect("tamper observation blob");
    assert!(
        freeze(&basis, &ws).is_err(),
        "freeze must reject observation bytes that do not match the pinned hash"
    );

    drop(ws);
    let ws = ToyWorkspace::open(&run.root).expect("reopen after working tamper");
    let basis = ws
        .basis(BasisPerspective::DurableOnly)
        .expect("fresh durable basis");

    // Tamper with the durable file AFTER the basis pinned its hash.
    let path = run.root.join("notes.md");
    let mut bytes = std::fs::read(&path).expect("notes.md exists");
    bytes.extend_from_slice(b"\ntampered after basis capture\n");
    std::fs::write(&path, bytes).expect("tamper write");

    let err = freeze(&basis, &ws).expect_err("freeze must refuse stale bytes");
    assert!(
        err.to_string().contains("basis-pinned hash"),
        "error must name the hash pin: {err}"
    );
}
