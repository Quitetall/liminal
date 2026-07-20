//! M05 exit-gate born-passing tests: Overlay lifecycle, the Reconciliation
//! Queue, aging, holder-return auto-repair, `lim overlays`, and the
//! mechanical enforcement of prohibition #4 (Algorithm D).

use camino::Utf8PathBuf;
use liminal_conformance::harness::{ToyRun, all_scenarios};
use liminal_daemon::ToyWorkspace;
use liminal_daemon::scenario::{ScenarioScript, Step};

/// Build a scripted step from a kind + pass-through fields.
fn step(kind: &str, fields: &[(&str, toml::Value)]) -> Step {
    let mut extra = toml::Table::new();
    for (k, v) in fields {
        extra.insert((*k).to_owned(), v.clone());
    }
    Step {
        kind: kind.to_owned(),
        extra,
    }
}

/// The `offline_holder_overlay` fixture, truncated to its first three steps
/// (`holder_unavailable`, `buffer_edit`, `save`) — i.e. up to but not
/// including the fixture's own `daemon_restart`, so a test can append its own
/// tail steps (`holder_available`, `advance_clock`, …).
fn offline_draft_scenario() -> ScenarioScript {
    let scenarios = all_scenarios().expect("must load scenarios");
    let mut scenario = scenarios
        .into_iter()
        .find(|s| s.scenario.id == "offline_holder_overlay")
        .expect("offline_holder_overlay must exist");
    scenario.steps.truncate(3);
    scenario
}

/// R4 §2.3 SLO shape, asserted exactly: the sound scenario creates zero
/// reconciliation items — no incident, visible or otherwise.
#[test]
fn sound_session_zero_unexpected_items() {
    let scenarios = all_scenarios().expect("must load scenarios");
    let scenario = scenarios
        .iter()
        .find(|s| s.scenario.id == "sound_session")
        .expect("sound_session must exist");

    let run = ToyRun::new("m05-sound-zero-items").expect("workspace");
    let exec = run.exec_scenario(scenario).expect("exec must run");
    assert!(exec.status.success(), "sound scenario must exec cleanly");

    let ws = ToyWorkspace::open(&run.root).expect("open");
    let items = ws.reconciliation().items().expect("items");
    assert_eq!(
        items.len(),
        0,
        "a sound session creates zero reconciliation items"
    );
}

/// Algorithm B holder-return: once the write route returns, the offline draft
/// auto-repairs — overlay gone, file bytes equal the draft, a `JUR_REPAIR`
/// record exists, and zero reconciliation items remain.
#[test]
fn auto_repair_on_holder_return() {
    let mut scenario = offline_draft_scenario();
    scenario.steps.push(step(
        "holder_available",
        &[("path", toml::Value::String("notes.md".into()))],
    ));

    let run = ToyRun::new("m05-holder-return").expect("workspace");
    let exec = run.exec_scenario(&scenario).expect("exec must run");
    assert!(
        exec.status.success(),
        "capture never rejected; stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );

    let ws = ToyWorkspace::open(&run.root).expect("open");
    let store = ws.store();

    let overlays = store
        .scan_aux(liminal_graph::ns::JUR_OVERLAY)
        .expect("scan overlays");
    assert_eq!(
        overlays.len(),
        0,
        "the overlay is retired once the repair auto-applies"
    );

    let records = ws.repairs().expect("repairs");
    assert_eq!(
        records.len(),
        1,
        "exactly one accepted repair (holder-return)"
    );
    assert_eq!(records[0].selected_rule, "holder-return");

    let on_disk = std::fs::read_to_string(run.root.join("notes.md")).unwrap();
    assert!(
        on_disk.contains("decomposes a time-domain signal"),
        "the durable file now carries the draft: {on_disk:?}"
    );

    let items = ws.reconciliation().items().expect("items");
    assert_eq!(items.len(), 0, "no debt remains once the draft lands");
}

/// Algorithm B aging: a large `advance_clock` past `escalate_after` surfaces
/// an `offline:path:notes.md` item, and `lim overlays` marks it overdue.
#[test]
fn overlay_aging_escalates() {
    let mut scenario = offline_draft_scenario();
    scenario.steps.push(step(
        "advance_clock",
        &[("by_secs", toml::Value::Integer(90_000))],
    ));

    let run = ToyRun::new("m05-aging-escalates").expect("workspace");
    let exec = run.exec_scenario(&scenario).expect("exec must run");
    assert!(exec.status.success());

    {
        let ws = ToyWorkspace::open(&run.root).expect("open");
        let items = ws.reconciliation().items().expect("items");
        assert_eq!(items.len(), 1, "the aged overlay surfaces exactly one item");
        assert_eq!(items[0].root_cause, "offline:path:notes.md");
        assert_eq!(
            items[0].status,
            liminal_jurisdiction::ReconciliationStatus::Pending
        );
    }

    let overlays_out = run.lim(&["overlays"]).expect("lim overlays runs");
    assert!(overlays_out.status.success());
    let stdout = String::from_utf8_lossy(&overlays_out.stdout);
    assert!(
        stdout.contains("(overdue)"),
        "an overlay aged past escalate_after must render overdue: {stdout:?}"
    );
}

/// `lim overlays` golden (Algorithm C): a single offline draft, aged by a
/// day-scale `advance_clock` so the rendered age bucket is stable regardless
/// of test runtime jitter (D05.4: offsets in days dwarf test-run milliseconds).
#[test]
fn overlays_output_golden() {
    let mut scenario = offline_draft_scenario();
    scenario.steps.push(step(
        "advance_clock",
        &[("by_secs", toml::Value::Integer(200_000))],
    ));

    let run = ToyRun::new("m05-overlays-golden").expect("workspace");
    let exec = run.exec_scenario(&scenario).expect("exec must run");
    assert!(exec.status.success());

    let overlays_out = run.lim(&["overlays"]).expect("lim overlays runs");
    assert!(overlays_out.status.success(), "lim overlays exit 0");
    let stdout = String::from_utf8_lossy(&overlays_out.stdout);
    insta::assert_snapshot!(stdout);
}

/// Mechanical enforcement of prohibition #4 (Algorithm D): the forbidden
/// subsystem token, case-insensitive, appears nowhere under `crates/` or
/// `conformance/` `*.rs` (skipping `target/`; `docs/`/`spec/` legitimately
/// name the future domain and are not scanned). The needle is built at runtime
/// by concatenation so the needle-building line cannot match itself. Every file
/// is scanned — including this one — and the single unavoidable residue (this
/// frozen gate function's own name) is allow-listed by exact `file:line`; the
/// test asserts that residue still exists AND is the only hit anywhere, so a
/// rename that hides the name fails loudly rather than silently.
#[test]
fn no_agenda_symbols() {
    let needle = "ag".to_string() + "enda";
    let repo_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_owned();

    // Scan EVERY `.rs` under crates/ and conformance/ (skip target/); do NOT
    // blanket-exclude this file — that would open a blind spot. Instead the
    // single unavoidable occurrence (this frozen exit-gate function's own
    // name) is allow-listed by exact `file:line` coordinate, and the test
    // asserts that occurrence still exists (so a rename that hides the name
    // fails loudly) AND that it is the ONLY hit anywhere.
    let mut hits = Vec::new();
    for dir in ["crates", "conformance"] {
        scan_for_needle(&repo_root.join(dir), &needle, &mut hits);
    }
    hits.sort();

    // Two exact coordinate residues: this frozen gate plus M12's frozen final
    // gate asserting absence. AM-12.4 forbids prefix, wildcard, substring, and
    // whole-file allowances.
    let this_file = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/milestones/m05.rs");
    let final_file = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/gate_final.rs");
    let self_signature = "fn no_".to_owned() + &needle + "_symbols() {";
    let final_signature = "fn overlay_debt_visible_without_".to_owned() + &needle + "() {";
    let mut allowed = vec![
        exact_coordinate(&this_file, &self_signature),
        exact_coordinate(&final_file, &final_signature),
    ];
    allowed.sort();
    assert_eq!(
        hits, allowed,
        "forbidden subsystem token found outside the two exact frozen signatures"
    );
}

fn exact_coordinate(path: &Utf8PathBuf, signature: &str) -> String {
    let text = std::fs::read_to_string(path).expect("read exact allow-list source");
    let matches: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == signature)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "exact allow-list signature must occur once in {path}: {signature:?}"
    );
    format!("{path}:{}", matches[0].0 + 1)
}

fn scan_for_needle(dir: &Utf8PathBuf, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path.is_dir() {
            if path.file_name() == Some("target") {
                continue;
            }
            scan_for_needle(&path, needle, hits);
        } else if path.extension() == Some("rs") {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(needle) {
                    hits.push(format!("{path}:{}", i + 1));
                }
            }
        }
    }
}
