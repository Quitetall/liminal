//! M03 exit-gate tests: the eight-questions golden and paragraph-parser
//! properties.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use liminal_conformance::harness::{ToyRun, all_scenarios};
use liminal_daemon::ToyWorkspace;
use liminal_graph::kind;
use liminal_id::{JurisdictionSubject, TransactionId};
use liminal_jurisdiction::{Checker, render_holder};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

/// Build a sound_session workspace on disk and return its root.
fn build_sound_workspace(label: &str) -> ToyRun {
    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "sound_session")
        .expect("sound_session must exist");
    let run = ToyRun::new(label).expect("must create workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "sound_session must exec cleanly: {}",
        String::from_utf8_lossy(&exec.stderr)
    );
    run
}

/// Render the eight questions for one subject under one perspective.
fn eight_questions(
    checker: &Checker<'_>,
    subject: JurisdictionSubject,
    basis: &WorkspaceBasis,
) -> String {
    let q1 = checker
        .resolve_holder(subject, basis)
        .map_or_else(|e| format!("error: {e}"), |h| render_holder(&h));
    let q2 = checker
        .write_route(subject, basis)
        .map_or_else(|e| format!("error: {e}"), |h| render_holder(&h));
    let q3 = checker.merge_runtime_required(subject).map_or_else(
        |e| format!("error: {e}"),
        |m| m.map_or_else(|| "none".to_owned(), |r| r.0),
    );
    let q4 = match subject {
        JurisdictionSubject::Node(n) => checker.identity_satisfies_relations(n, basis).map_or_else(
            |e| format!("error: {e}"),
            |f| format!("{} findings", f.len()),
        ),
        JurisdictionSubject::Relation(_) => "n/a (relation)".to_owned(),
    };
    let q5 = checker.unresolved_overlays(Some(subject)).map_or_else(
        |e| format!("error: {e}"),
        |o| format!("{} overlays", o.len()),
    );
    // Render the perspective by variant name only — the client UUID is not
    // deterministic and irrelevant to the golden.
    let q6 = match checker.governing_perspective(basis) {
        BasisPerspective::DurableOnly => "durable-only".to_owned(),
        BasisPerspective::ClientScoped { .. } => "client-scoped".to_owned(),
        BasisPerspective::Published { .. } => "published".to_owned(),
        BasisPerspective::Federated { .. } => "federated".to_owned(),
    };

    format!(
        "  Q1 holder: {q1}\n  Q2 write_route: {q2}\n  Q3 merge_runtime: {q3}\n  \
         Q4 identity: {q4}\n  Q5 overlays: {q5}\n  Q6 perspective: {q6}\n"
    )
}

#[test]
fn eight_questions_golden() {
    let run = build_sound_workspace("eight-q");
    let ws = ToyWorkspace::open(&run.root).expect("must open workspace");
    let checker = ws.checker();
    let store = ws.store();

    // Find the three subjects: file node, p-fourier paragraph, comment relation.
    let file_node = store
        .nodes()
        .unwrap()
        .into_iter()
        .find(|n| n.kind == kind::FILE)
        .expect("file node must exist");

    // p-fourier via alias.
    let alias_value = store
        .get_aux(liminal_graph::ns::JUR_ALIAS, "p-fourier")
        .unwrap()
        .expect("p-fourier alias must exist");
    let p_fourier: liminal_id::NodeId = alias_value["node"].as_str().unwrap().parse().unwrap();

    let comment_rel = store
        .relations()
        .unwrap()
        .into_iter()
        .find(|r| r.kind == kind::COMMENT)
        .expect("comment relation must exist");

    let subjects = [
        ("file node", JurisdictionSubject::Node(file_node.id)),
        ("p-fourier", JurisdictionSubject::Node(p_fourier)),
        (
            "comment relation",
            JurisdictionSubject::Relation(comment_rel.id),
        ),
    ];

    let durable = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    // ClientScoped without captured buffer components (real capture is M6):
    // resolution falls back to durable read precedence.
    let client_scoped = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::ClientScoped {
            client: liminal_id::ClientId::new(),
        },
        components: BTreeMap::new(),
    };

    let mut report = String::new();
    for (name, subject) in &subjects {
        let _ = writeln!(report, "== {name} — DurableOnly ==");
        report.push_str(&eight_questions(&checker, *subject, &durable));
        let _ = writeln!(report, "== {name} — ClientScoped(neovim) ==");
        report.push_str(&eight_questions(&checker, *subject, &client_scoped));
    }

    insta::assert_snapshot!(report);
}

#[test]
fn paragraph_parser_props() {
    use liminal_source::paragraph::parse;
    use proptest::prelude::*;

    proptest!(|(input in ".*")| {
        // Parse is total: never panics.
        let blocks = parse(&input);

        // Every block's range slices correctly into the input.
        for block in &blocks {
            let start = usize::try_from(block.range.start).unwrap();
            let end = usize::try_from(block.range.end).unwrap();
            prop_assert!(start <= end, "range must be well-ordered");
            prop_assert!(end <= input.len(), "range must be within input");
            // The slice must land on a char boundary and be extractable.
            prop_assert!(input.is_char_boundary(start));
            prop_assert!(input.is_char_boundary(end));
        }

        // No block text contains the raw marker of its own well-formed id:
        // a well-formed id is stripped from the text.
        for block in &blocks {
            if let Some(id) = &block.id {
                let marker = format!("{{#{id}}}");
                prop_assert!(
                    !block.text.ends_with(&marker),
                    "well-formed id marker must be stripped from text"
                );
            }
        }
    });
}
