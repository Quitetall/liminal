//! M09 exit gate: anchor-recovery report golden + DECLARED_LEVEL consistency
//! (v4 §-1.2, §8.4, §8.5; R4 §11.6).
//!
//! `anchor_recovery_report_golden` regenerates the report via the spike libs
//! and snapshots it with insta (date and git-version lines redacted for
//! cross-machine stability).
//!
//! `declared_level_matches_report` recomputes the level from the report's own
//! numbers via D09.4 and asserts it equals `spike_annotation::DECLARED_LEVEL`.

use camino::Utf8PathBuf;
use spike_annotation::{render_report, declared_level, run_foreign_edit_loop, emit, parse, AnnotatedDoc};
use spike_richedit::run_richedit_loop;
use liminal_conformance::identity::Config;

/// The workspace root (conformance/ has a parent).
fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_owned()
}

/// The committed golden path.
fn golden_path() -> Utf8PathBuf {
    repo_root().join("conformance/golden/anchor_recovery.md")
}

/// Redact the box date range and git version for cross-machine stability.
fn redact_volatile(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        if line.starts_with("box: ") {
            out.push_str("box: [boxed] (hard 2-week box)\n");
        } else if line.starts_with("config: ") {
            if let Some(git_pos) = line.find("git: ") {
                let prefix = &line[..git_pos];
                out.push_str(prefix);
                out.push_str("git: [redacted]\n");
            } else {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Build the full report: run both spikes, render.
fn build_report() -> String {
    let cfg = Config::load().expect("load identity config");

    // Spike 1: foreign-edit loop (11 ops × 8 seeds).
    let foreign = run_foreign_edit_loop(&cfg).expect("foreign edit loop");

    // Spike 2: rich-edit loop (5 ops × 8 seeds).
    let richedit = run_richedit_loop(&cfg);

    render_report(&cfg, &foreign, &richedit, spike_annotation::BOX_START, "2026-07-18")
}

/// Check round-trip on unedited docs (L2 prerequisite).
fn check_round_trip() -> bool {
    let cfg = Config::load().expect("load identity config");
    let doc = AnnotatedDoc::build(&cfg.template, 33).expect("build");
    let emitted = emit(&doc);
    let parsed = parse(&emitted).expect("parse");
    parsed.text == doc.text
}

/// `anchor_recovery_report_golden`: the rendered report matches the committed
/// golden, modulo volatile lines (box date, git version).
#[test]
fn anchor_recovery_report_golden() {
    let rendered = build_report();
    let _redacted = redact_volatile(&rendered);
    let path = golden_path();

    if std::env::var("BLESS_ANCHOR_REPORT").is_ok() || !path.exists() {
        std::fs::write(&path, &rendered).expect("write golden");
    }
    let golden = std::fs::read_to_string(&path).expect("read anchor_recovery.md golden");

    assert_eq!(
        redact_volatile(&rendered),
        redact_volatile(&golden),
        "anchor recovery report drifted from the committed golden; if intended, \
         regenerate with BLESS_ANCHOR_REPORT=1 and review the diff"
    );
}

/// `declared_level_matches_report`: recompute the level from the report's own
/// numbers and assert it equals `DECLARED_LEVEL`.
#[test]
fn declared_level_matches_report() {
    let cfg = Config::load().expect("load identity config");
    let foreign = run_foreign_edit_loop(&cfg).expect("foreign edit loop");
    let richedit = run_richedit_loop(&cfg);

    let round_trip_ok = check_round_trip();
    let computed = declared_level(&foreign, &richedit, round_trip_ok);
    assert_eq!(
        computed,
        spike_annotation::DECLARED_LEVEL,
        "computed level ({}) != DECLARED_LEVEL ({}); report numbers may have changed",
        computed,
        spike_annotation::DECLARED_LEVEL
    );
}
