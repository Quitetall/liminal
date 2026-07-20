//! Final Phase -1 gate (v4 Part XXII): one born-passing test per gate bullet.

use liminal_conformance::harness::{ToyRun, all_scenarios};
use liminal_conformance::identity::claims::{column_cap, external_file_floor, strategy_index};
use liminal_conformance::identity::matrix::{Matrix, redact_git_line};
use liminal_conformance::identity::{Config, Strategy};
use liminal_conformance::pandoc;
use liminal_conformance::replay::{FrozenWorld, freeze};
use liminal_conformance::scenario::{ScenarioHeader, ScenarioScript};
use liminal_daemon::ToyWorkspace;
use liminal_daemon::queries::{AiContextStub, Backlinks, RenderBlock, WorkspaceExport};
use liminal_daemon::scenario::Step;
use liminal_id::{ClientId, IdentityGrade, NodeId, PathId};
use liminal_query::Query;
use liminal_revision::{BasisComponent, BasisPerspective, ComponentDeps};
use spike_annotation::{
    AnnotatedDoc, canonical_eq, declared_level, emit, parse, run_foreign_edit_loop,
};
use spike_richedit::run_richedit_loop;
use std::fmt::Write as _;

fn one_file_workspace(label: &str) -> ToyRun {
    let scenario = ScenarioScript {
        scenario: ScenarioHeader {
            id: format!("m12-{label}"),
            title: None,
            spec: vec![],
            profiles: vec!["external-file".into()],
        },
        setup: liminal_daemon::scenario::Setup {
            files: vec![liminal_daemon::scenario::SetupFile {
                path: "notes.md".into(),
                text: "durable base {#p}\n".into(),
            }],
            graph: vec![],
            buffers: vec![],
        },
        steps: vec![],
        expect: liminal_daemon::scenario::Expectation::default(),
    };
    let run = ToyRun::new(label).expect("create one-file workspace");
    let output = run
        .exec_scenario(&scenario)
        .expect("ingest one-file workspace");
    assert!(
        output.status.success(),
        "one-file setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    run
}

fn alias_node(workspace: &ToyWorkspace, alias: &str) -> NodeId {
    workspace
        .store()
        .get_aux(liminal_graph::ns::JUR_ALIAS, alias)
        .expect("read alias namespace")
        .expect("alias exists")["node"]
        .as_str()
        .expect("alias node is a string")
        .parse()
        .expect("alias node parses")
}

fn run_frozen_consumers(world: &FrozenWorld, node: NodeId) -> [String; 4] {
    let basis = world.basis();
    let label = "durable-only".to_owned();
    let preview = RenderBlock {
        world,
        node,
        perspective_label: label.clone(),
    }
    .execute(basis, &mut ComponentDeps::default());
    let ai_context = AiContextStub {
        world,
        focus: node,
        perspective_label: label.clone(),
    }
    .execute(basis, &mut ComponentDeps::default());
    let backlinks = Backlinks {
        world,
        node,
        perspective_label: label.clone(),
    }
    .execute(basis, &mut ComponentDeps::default());
    let export = WorkspaceExport {
        world,
        perspective_label: label,
    }
    .execute(basis, &mut ComponentDeps::default());
    [preview, ai_context, backlinks, export]
}

fn scan_rust_sources(dir: &camino::Utf8Path, needles: &[String], hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(path) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path.is_dir() {
            scan_rust_sources(&path, needles, hits);
        } else if path.extension() == Some("rs") {
            let text =
                std::fs::read_to_string(&path).expect("read Rust source for primitive audit");
            for (line_index, line) in text.lines().enumerate() {
                if needles.iter().any(|needle| line.contains(needle)) {
                    hits.push(format!("{path}:{}", line_index + 1));
                }
            }
        }
    }
}

fn exact_source_coordinate(path: &camino::Utf8Path, signature: &str) -> String {
    let text = std::fs::read_to_string(path).expect("read exact allow-list source");
    let matches: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == signature)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "exact signature must occur once in {path}"
    );
    format!("{path}:{}", matches[0].0 + 1)
}

fn scan_token_hits(dir: &camino::Utf8Path, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(path) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path.is_dir() {
            if path.file_name() != Some("target") {
                scan_token_hits(&path, needle, hits);
            }
        } else if path.extension() == Some("rs") {
            let text = std::fs::read_to_string(&path).expect("read source for token sweep");
            for (line_index, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(needle) {
                    hits.push(format!("{path}:{}", line_index + 1));
                }
            }
        }
    }
}

/// Every subject created by every Phase -1 scenario resolves through checker
/// Q1 to a concrete Holder at the durable Basis. `Ungoverned` is never hidden
/// by sampling a hand-written subset of subjects.
#[test]
fn every_toy_subject_names_its_holder() {
    let scenarios = all_scenarios().expect("load every scenario");
    assert!(
        !scenarios.is_empty(),
        "the toy scenario set must be non-empty"
    );

    for scenario in &scenarios {
        let run =
            ToyRun::new(&format!("m12-holder-{}", scenario.scenario.id)).expect("create workspace");
        let exec = run.exec_scenario(scenario).expect("execute scenario");
        assert!(
            exec.status.success(),
            "scenario {} must execute before holder audit: {}",
            scenario.scenario.id,
            String::from_utf8_lossy(&exec.stderr)
        );

        let mut ws = ToyWorkspace::open(&run.root).expect("open audited workspace");
        let basis = ws
            .basis(BasisPerspective::DurableOnly)
            .expect("capture durable basis");
        let checker = ws.checker();
        let subjects = checker.subjects().expect("enumerate governed subjects");
        assert!(
            !subjects.is_empty(),
            "scenario {} must create at least one governed subject",
            scenario.scenario.id
        );
        let mut graph_native_subjects = Vec::new();
        for subject in subjects {
            if let liminal_id::JurisdictionSubject::Node(node_id) = subject
                && ws
                    .store()
                    .node_at(ws.store().head().expect("read graph head"), node_id)
                    .expect("read subject node")
                    .is_some_and(|node| node.kind == liminal_graph::kind::EXTERNAL_VALUE)
            {
                assert_eq!(
                    liminal_jurisdiction::profile::grade_of(subject, ws.store()),
                    IdentityGrade::Managed,
                    "EXTERNAL_VALUE must carry its approved Managed identity"
                );
            }
            let holder = checker
                .resolve_holder(subject, &basis)
                .unwrap_or_else(|error| {
                    panic!(
                        "scenario {} subject {subject} has no concrete Holder: {error}",
                        scenario.scenario.id
                    )
                });
            if holder == liminal_jurisdiction::Holder::Graph {
                graph_native_subjects.push(subject);
            }
        }

        if let Some(path) =
            basis
                .components
                .iter()
                .find_map(|(key, component)| match (key, component) {
                    (
                        liminal_id::JurisdictionKey::Path(path),
                        BasisComponent::FileContent { .. },
                    ) => Some(path.clone()),
                    _ => None,
                })
        {
            let client = ClientId::new();
            ws.client(client).open_buffer(path);
            let client_basis = ws
                .basis(BasisPerspective::ClientScoped { client })
                .expect("capture client-scoped audit basis");
            let checker = ws.checker();
            for subject in graph_native_subjects {
                assert_eq!(
                    checker
                        .resolve_holder(subject, &client_basis)
                        .expect("graph-native subject remains governed"),
                    liminal_jurisdiction::Holder::Graph,
                    "graph-native subject {subject} must not be captured by a client buffer"
                );
            }
        }
    }
}

/// Schema shorthands never became a third governed primitive: no dedicated
/// identifier symbol exists in production Rust, and every enumerated subject
/// has the canonical node or relation address form.
#[test]
fn no_facet_primitive_exists() {
    let needles = ["Facet".to_owned() + "Id", "facet".to_owned() + "_id"];
    let repo_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance has a repository parent")
        .to_owned();
    let mut hits = Vec::new();
    scan_rust_sources(&repo_root.join("crates"), &needles, &mut hits);
    assert!(
        hits.is_empty(),
        "a third primitive leaked into production: {hits:?}"
    );

    for scenario in all_scenarios().expect("load every scenario") {
        let run = ToyRun::new(&format!("m12-address-{}", scenario.scenario.id))
            .expect("create subject-address workspace");
        let exec = run.exec_scenario(&scenario).expect("execute scenario");
        assert!(
            exec.status.success(),
            "scenario {} must pass",
            scenario.scenario.id
        );
        let workspace = ToyWorkspace::open(&run.root).expect("open subject-address workspace");
        for subject in workspace.checker().subjects().expect("enumerate subjects") {
            let address = subject.to_string();
            assert!(
                address.starts_with("node:") || address.starts_with("relation:"),
                "subject address must be node/relation shaped, got {address}"
            );
        }
    }
}

/// The anonymous-text profile's identity claim is capped by the weakest
/// outcome demonstrated by the two heuristic M07 strategies. The matrix is
/// regenerated and compared with its frozen golden before its evidence is
/// used, so this gate cannot reason from a stale report.
#[test]
fn anonymous_text_identity_grade_is_honest() {
    let config = Config::load().expect("load identity corpus config");
    let matrix = Matrix::build(&config).expect("rebuild identity matrix");
    let golden_path =
        camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/identity_matrix.md");
    let golden = std::fs::read_to_string(&golden_path).expect("read identity matrix golden");
    assert_eq!(
        redact_git_line(&matrix.render()),
        redact_git_line(&golden),
        "M07 identity evidence drifted from its frozen golden"
    );

    let heuristic_cap = [Strategy::Structural, Strategy::RevisionAnchor]
        .into_iter()
        .map(|strategy| {
            let index = strategy_index(&matrix, strategy).expect("heuristic strategy is present");
            column_cap(&matrix, index)
        })
        .min_by_key(|grade| grade.strength())
        .expect("the heuristic strategy set is non-empty");

    let declared = external_file_floor();
    assert_eq!(
        declared, heuristic_cap,
        "the anonymous-text identity declaration must equal demonstrated evidence"
    );
    assert!(
        declared.strength() < IdentityGrade::ContentAddressed.strength(),
        "anonymous text must not claim the strongest identity grade"
    );
}

/// One concrete projection reaches capability Level 2, while both richer
/// adapter lenses keep their limitations measured and their declared levels
/// mechanically tied to those measurements.
#[test]
fn one_projection_reaches_canonical_roundtrip_and_lenses_are_measured() {
    let config = Config::load().expect("load identity corpus config");
    let graph = AnnotatedDoc::build(&config.template, 33).expect("build supported projection");
    let source = emit(&graph);
    let reparsed = parse(&source).expect("parse emitted projection");
    assert!(
        canonical_eq(&reparsed, &graph),
        "parse(emit(graph)) must preserve the supported subset"
    );
    let canonical_source = emit(&reparsed);
    let canonical_again = emit(&parse(&canonical_source).expect("parse canonical source"));
    assert_eq!(canonical_again.text, canonical_source.text);
    assert_eq!(
        serde_json::to_vec(&canonical_again.annotations).expect("serialize canonical annotations"),
        serde_json::to_vec(&canonical_source.annotations).expect("serialize source annotations"),
        "emit(parse(source)) must canonicalize"
    );

    let golden_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden");
    for report in ["anchor_recovery.md", "pandoc_loss.md"] {
        let bytes = std::fs::read(golden_root.join(report)).expect("read committed lens report");
        assert!(!bytes.is_empty(), "{report} must not be empty");
    }

    let foreign = run_foreign_edit_loop(&config).expect("measure annotation recovery");
    let rich_edit = run_richedit_loop(&config);
    assert_eq!(
        declared_level(&foreign, &rich_edit, true),
        spike_annotation::DECLARED_LEVEL,
        "M09 declared level must equal its live measurement"
    );

    pandoc::assert_pandoc_pinned();
    let fixture = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/conversion-loss/pandoc");
    let workdir = camino::Utf8PathBuf::from(
        std::env::temp_dir()
            .to_str()
            .expect("temporary directory is UTF-8"),
    )
    .join(format!("liminal-m12-pandoc-{}", std::process::id()));
    let measurement = pandoc::measure(&fixture, &workdir).expect("measure pandoc lens");
    assert_eq!(
        measurement.declared_level(),
        pandoc::DECLARED_LEVEL,
        "M10 declared level must equal its live measurement"
    );
}

/// The canonical cross-Holder DAG survives every derived crash point and an
/// accepted repair remains revertible through the public CLI.
#[test]
fn crash_gate_matrix_and_revert_hold() {
    let scenarios = all_scenarios().expect("load scenarios");
    let dag = scenarios
        .iter()
        .find(|scenario| scenario.scenario.id == "dag_accept")
        .expect("dag_accept scenario exists");
    ToyRun::crash_matrix(dag).expect("two-step repair DAG crash matrix passes");

    let repair_scenario = scenarios
        .iter()
        .find(|scenario| scenario.scenario.id == "disjoint_safe_repair")
        .expect("disjoint_safe_repair scenario exists");
    let run = ToyRun::new("m12-revert").expect("create repair workspace");
    let exec = run
        .exec_scenario(repair_scenario)
        .expect("execute repair scenario");
    assert!(exec.status.success(), "accepted repair scenario must pass");
    let (repair_id, repaired_bytes, expected_preimage) = {
        let workspace = ToyWorkspace::open(&run.root).expect("open repair workspace");
        let repairs = workspace.repairs().expect("read repair records");
        assert_eq!(repairs.len(), 1, "exactly one repair must be accepted");
        assert!(
            repairs[0].inverse.is_some(),
            "accepted repair records its inverse"
        );
        let expected_preimage = repairs[0]
            .inverse
            .as_ref()
            .expect("accepted repair records its inverse")
            .steps
            .values()
            .find_map(|step| match &step.operation {
                liminal_jurisdiction::RepairOperation::WriteFile { path, contents }
                    if path.0 == "notes.md" =>
                {
                    Some(contents.clone())
                }
                _ => None,
            })
            .expect("inverse records notes.md preimage bytes");
        (
            repairs[0].repair,
            std::fs::read(run.root.join("notes.md")).expect("read repaired file"),
            expected_preimage,
        )
    };
    let undo = run
        .lim(&["repair", "undo", &repair_id.to_string()])
        .expect("run repair undo");
    assert!(undo.status.success(), "repair undo must succeed");
    assert!(
        String::from_utf8_lossy(&undo.stdout).starts_with("undone: repair:"),
        "repair undo must report the reverted record"
    );
    let reverted_bytes = std::fs::read(run.root.join("notes.md")).expect("read reverted file");
    assert_ne!(
        reverted_bytes, repaired_bytes,
        "undo must change repaired bytes"
    );
    assert_eq!(
        reverted_bytes, expected_preimage,
        "undo must restore the inverse's exact recorded preimage"
    );
}

/// Divergent working claims never form a chimeric Basis. An ambiguous pair
/// falls back to durable truth and creates one visible root-cause item; a
/// bounded two-client interleaving rechecks the invariant after every edit.
#[test]
fn no_chimeric_basis_at_gate() {
    let run = one_file_workspace("m12-ambiguous");
    let mut workspace = ToyWorkspace::open(&run.root).expect("open ambiguous workspace");
    let path = PathId("notes.md".into());
    let client = ClientId::new();
    let first = workspace.client(client).open_buffer(path.clone());
    workspace
        .client(client)
        .edit(first, "first dirty branch {#p}\n");
    let second = workspace.client(client).open_buffer(path.clone());
    workspace
        .client(client)
        .edit(second, "second dirty branch {#p}\n");

    let requester_less = workspace
        .basis(BasisPerspective::DurableOnly)
        .expect("requester-less computation fails closed to durable truth");
    assert_eq!(requester_less.perspective, BasisPerspective::DurableOnly);
    assert!(
        requester_less
            .components
            .values()
            .all(|component| !matches!(component, BasisComponent::BufferGeneration { .. })),
        "DurableOnly must contain no dirty buffer component"
    );
    let pending: Vec<_> = workspace
        .reconciliation()
        .items()
        .expect("read reconciliation queue")
        .into_iter()
        .filter(|item| {
            item.status == liminal_jurisdiction::ReconciliationStatus::Pending
                && item.root_cause.starts_with("ambiguity:")
        })
        .collect();
    assert_eq!(
        pending.len(),
        1,
        "ambiguity must surface as one coalesced item"
    );

    let run = one_file_workspace("m12-interleaving");
    let mut workspace = ToyWorkspace::open(&run.root).expect("open interleaving workspace");
    let neovim = ClientId::new();
    let phone = ClientId::new();
    let neovim_buffer = workspace.client(neovim).open_buffer(path.clone());
    let phone_buffer = workspace.client(phone).open_buffer(path);
    for step in 0..16 {
        let (actor, buffer) = if step % 2 == 0 {
            (neovim, neovim_buffer)
        } else {
            (phone, phone_buffer)
        };
        workspace
            .client(actor)
            .edit(buffer, &format!("dirty-{step} {{#p}}\n"));
        for perspective in [
            BasisPerspective::ClientScoped { client: neovim },
            BasisPerspective::ClientScoped { client: phone },
            BasisPerspective::DurableOnly,
        ] {
            let basis = workspace
                .basis(perspective.clone())
                .expect("resolve bounded Basis");
            let buffer_clients: Vec<_> = basis
                .components
                .values()
                .filter_map(|component| match component {
                    BasisComponent::BufferGeneration { client, .. } => Some(*client),
                    _ => None,
                })
                .collect();
            assert!(buffer_clients.len() <= 1, "Basis combined dirty claims");
            match perspective {
                BasisPerspective::ClientScoped { client } => {
                    assert!(buffer_clients.iter().all(|actual| *actual == client));
                }
                BasisPerspective::DurableOnly => assert!(buffer_clients.is_empty()),
                BasisPerspective::Published { .. } | BasisPerspective::Federated { .. } => {
                    unreachable!("bounded gate only uses concrete Phase -1 perspectives")
                }
            }
        }
    }
}

#[test]
fn overlay_debt_visible_without_agenda() {
    let scenarios = all_scenarios().expect("load scenarios");
    let mut scenario = scenarios
        .into_iter()
        .find(|scenario| scenario.scenario.id == "offline_holder_overlay")
        .expect("offline_holder_overlay scenario exists");
    scenario.steps.truncate(3);
    let mut extra = toml::Table::new();
    extra.insert("by_secs".into(), toml::Value::Integer(90_000));
    scenario.steps.push(Step {
        kind: "advance_clock".into(),
        extra,
    });

    let run = ToyRun::new("m12-overlay-debt").expect("create offline workspace");
    let exec = run
        .exec_scenario(&scenario)
        .expect("execute aged offline trace");
    assert!(exec.status.success(), "aged offline trace must pass");
    let output = run.lim(&["overlays"]).expect("run overlay debt view");
    assert!(output.status.success(), "overlay debt view must succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("notes.md"),
        "aged debt must name its path: {stdout:?}"
    );
    assert!(
        stdout.contains("overdue"),
        "aged debt must be visibly overdue: {stdout:?}"
    );

    let needle = "ag".to_owned() + "enda";
    let repo_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance has repository parent")
        .to_owned();
    let mut hits = Vec::new();
    for dir in ["crates", "conformance"] {
        scan_token_hits(&repo_root.join(dir), &needle, &mut hits);
    }
    hits.sort();
    let m05 = repo_root.join("conformance/tests/milestones/m05.rs");
    let final_gate = repo_root.join("conformance/tests/gate_final.rs");
    let m05_signature = "fn no_".to_owned() + &needle + "_symbols() {";
    let final_signature = "fn overlay_debt_visible_without_".to_owned() + &needle + "() {";
    let mut allowed = vec![
        exact_source_coordinate(&m05, &m05_signature),
        exact_source_coordinate(&final_gate, &final_signature),
    ];
    allowed.sort();
    assert_eq!(
        hits, allowed,
        "token sweep permits only two exact frozen absence-assertion coordinates"
    );
}

/// Replaying the four pure consumers twice from the same serialized Basis,
/// after the live workspace is gone, produces byte-identical outputs.
#[test]
fn frozen_basis_replay_is_deterministic_at_gate() {
    let scenarios = all_scenarios().expect("load scenarios");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.scenario.id == "four_consumers")
        .expect("four_consumers scenario exists");
    let run = ToyRun::new("m12-frozen-replay").expect("create replay workspace");
    let exec = run
        .exec_scenario(scenario)
        .expect("execute four-consumer scenario");
    assert!(exec.status.success(), "four-consumer scenario must pass");

    let workspace = ToyWorkspace::open(&run.root).expect("open replay workspace");
    let node = alias_node(&workspace, "p-fourier");
    let basis = workspace
        .basis(BasisPerspective::DurableOnly)
        .expect("capture durable replay basis");
    let frozen = freeze(&basis, &workspace).expect("freeze exact Basis inputs");
    drop(workspace);
    std::fs::remove_dir_all(&run.root).expect("remove live replay workspace");

    let first_world = FrozenWorld::load(&frozen).expect("load first replay world");
    let second_world = FrozenWorld::load(&frozen).expect("load second replay world");
    let first = run_frozen_consumers(&first_world, node);
    let second = run_frozen_consumers(&second_world, node);
    assert_eq!(
        first, second,
        "frozen consumer replays must be byte-identical"
    );
}

/// Frozen denominator arithmetic, the immutable held-out/v1 split, and both
/// recorded per-profile scorecards all remain present and internally valid.
#[test]
fn denominators_frozen_and_split_locked() {
    let conformance_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut rendered = String::from(
        "# §7.4 denominator counts — hand-labeled traces (M11.3, FROZEN)\n\n\
         Regenerate deliberately with BLESS_DENOMINATOR_COUNTS=1; a change\n\
         here is a corpus-version bump, never an edit (v4 §7.4; POLICY.md).\n",
    );
    for name in [
        "coalesce-basic",
        "session-timeout",
        "git-rootcause",
        "offline-transient",
    ] {
        let fixture = conformance_root
            .join("fixtures/traces/labeled")
            .join(format!("{name}.trace.ndjson"));
        let trace = liminal_conformance::Trace::parse(
            &std::fs::read_to_string(&fixture).expect("read labeled denominator fixture"),
        )
        .expect("parse labeled denominator fixture");
        let counts = liminal_conformance::denominator::count(&trace.events);
        let _ = write!(
            rendered,
            "\n## {name}\n\nops: {}\ntxns: {}\nsessions: {}\n",
            counts.ops, counts.txns, counts.sessions
        );
        if counts.incidents_by_cause.is_empty() {
            rendered.push_str("incidents: none\n");
        } else {
            for (cause, count) in &counts.incidents_by_cause {
                let _ = writeln!(rendered, "incident: {cause} = {count}");
            }
        }
    }
    let denominator_golden =
        std::fs::read_to_string(conformance_root.join("golden/denominator_counts.md"))
            .expect("read frozen denominator golden");
    assert_eq!(
        rendered, denominator_golden,
        "denominator arithmetic drifted"
    );

    let empty_probe = camino::Utf8PathBuf::from(
        std::env::temp_dir()
            .to_str()
            .expect("temporary directory is UTF-8"),
    )
    .join(format!("liminal-m12-empty-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty_probe);
    std::fs::create_dir_all(&empty_probe).expect("create empty-corpus probe");
    std::fs::write(empty_probe.join("MANIFEST.b3"), b" \n")
        .expect("write whitespace manifest probe");
    let empty_error = liminal_conformance::harness::verify_heldout_manifest(&empty_probe)
        .expect_err("an empty locked corpus must fail closed");
    let _ = std::fs::remove_dir_all(&empty_probe);
    assert!(
        empty_error.to_string().contains("non-empty"),
        "empty-corpus error must name the invariant: {empty_error}"
    );

    let heldout_v1 = conformance_root.join("corpora/heldout/v1");
    liminal_conformance::harness::verify_heldout_manifest(&heldout_v1)
        .expect("heldout/v1 must match its immutable manifest");
    assert!(
        std::fs::metadata(heldout_v1.join("MANIFEST.b3"))
            .expect("heldout/v1 manifest exists")
            .len()
            > 0,
        "a valid non-empty manifest proves heldout/v1 contains corpus entries"
    );

    let scorecards: serde_json::Value = serde_json::from_slice(
        &std::fs::read(conformance_root.join("golden/scorecard-heldout-v1.json"))
            .expect("read recorded held-out scorecards"),
    )
    .expect("parse recorded held-out scorecards");
    for profile in ["external-file", "graph-native"] {
        assert_eq!(
            scorecards[profile]["scorecard"]["profile"], profile,
            "recorded scorecard must identify {profile}"
        );
        assert_eq!(
            scorecards[profile]["scorecard"]["corpus"], "heldout/v1",
            "recorded scorecard must remain tied to the locked split"
        );
    }
}

/// The accepted phase-gate ADR records one unambiguous GO/NO-GO decision and
/// carries a complete results table for the nine evidence-producing gates.
#[test]
fn go_no_go_adr_recorded() {
    let repo_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance has a repository parent")
        .to_owned();
    let adr_path =
        repo_root.join("docs/adr/0013-phase-0-go-no-go-after-falsification-laboratory.md");
    let adr = std::fs::read_to_string(&adr_path).expect("read Phase 0 go/no-go ADR");
    assert_eq!(
        adr.lines()
            .filter(|line| {
                *line == "# 0013. Phase 0 go/no-go after the falsification laboratory"
            })
            .count(),
        1,
        "go/no-go ADR must have the exact frozen title"
    );
    assert_eq!(
        adr.lines()
            .filter(|line| *line == "- **Status:** accepted")
            .count(),
        1,
        "go/no-go ADR must have exactly one accepted status"
    );

    let decision_markers = adr.matches("**GO**").count() + adr.matches("**NO-GO**").count();
    assert_eq!(
        decision_markers, 1,
        "go/no-go ADR must contain exactly one binding decision marker"
    );

    let results = adr
        .split_once("## Final gate results\n")
        .map(|(_, tail)| tail)
        .and_then(|tail| tail.split_once("\n## ").map(|(section, _)| section))
        .expect("ADR must contain a bounded final-gate results section");
    let result_rows: Vec<_> = results
        .lines()
        .filter(|line| line.starts_with("| `"))
        .collect();
    assert_eq!(
        result_rows.len(),
        10,
        "results table must have ten test rows"
    );

    let forbidden_token = "ag".to_owned() + "enda";
    let overlay_gate = format!("overlay_debt_visible_without_{forbidden_token}");
    for test_name in [
        "every_toy_subject_names_its_holder",
        "no_facet_primitive_exists",
        "anonymous_text_identity_grade_is_honest",
        "one_projection_reaches_canonical_roundtrip_and_lenses_are_measured",
        "crash_gate_matrix_and_revert_hold",
        "no_chimeric_basis_at_gate",
        &overlay_gate,
        "frozen_basis_replay_is_deterministic_at_gate",
        "denominators_frozen_and_split_locked",
        "go_no_go_adr_recorded",
    ] {
        let expected = format!("| `{test_name}` | PASS |");
        assert_eq!(
            result_rows.iter().filter(|row| **row == expected).count(),
            1,
            "results table must contain exactly one passing row for {test_name}"
        );
    }
}
