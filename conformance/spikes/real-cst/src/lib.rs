//! Phase 0 M17 real-CST measurement spike.
//!
//! This crate is deliberately outside the production `liminal-cst` API. It
//! measures incremental anchor recovery over `tree-sitter-md` 0.5.3, compares
//! every incremental result with a separately constructed full reparse, and
//! preserves negative and malformed cases in a frozen inventory.
//!
//! The selected parser is useful evidence, not production authority: its own
//! upstream documentation warns that Markdown output still has inaccuracies.
//! The spike therefore keeps the capability declaration at Level 2 and records
//! the absence of an independently qualified lossless serializer as a blocker.

#![allow(
    clippy::all,
    clippy::pedantic,
    reason = "D17.1: isolated measurement spike, not production API"
)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;
use tree_sitter::{InputEdit, Node, Point};
use tree_sitter_md::{MarkdownParser, MarkdownTree};

/// Exact selected substrate version for the frozen M17 measurement.
pub const SUBSTRATE: &str = "tree-sitter-md 0.5.3 + tree-sitter 0.26.3";
/// Accepted grammar decision governing the experiment.
pub const GRAMMAR_ADR: &str = "docs/adr/0015-select-phase-1-source-grammar.md";
/// Evidence-bounded declaration. Level 3 remains blocked by serializer and
/// upstream-correctness qualification, even when recovery rows pass.
pub const DECLARED_LEVEL: u8 = 2;

const INVENTORY_BYTES: &[u8] = include_bytes!("../../../fixtures/phase0/real-cst-cases.json");

#[derive(Debug, Deserialize)]
struct Inventory {
    schema_version: u32,
    cases: Vec<FrozenCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct FrozenCase {
    id: String,
    edit_class: String,
    seed: u64,
    base: String,
    edited: String,
    anchor: String,
    expected: ExpectedOutcome,
    declared_loss: Option<String>,
    malformed: bool,
}

/// Frozen expected anchor-recovery classification.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExpectedOutcome {
    /// One exact candidate remains.
    Recovered,
    /// More than one exact candidate remains.
    Ambiguous,
    /// No exact candidate remains.
    Lost,
}

impl ExpectedOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::Recovered => "recovered",
            Self::Ambiguous => "ambiguous",
            Self::Lost => "lost",
        }
    }
}

/// One deterministic row in the M17 measurement report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementRow {
    pub case_id: String,
    pub edit_class: String,
    pub seed: u64,
    pub expected_anchor: String,
    pub observed_anchor: String,
    pub expected_outcome: ExpectedOutcome,
    pub observed_outcome: ExpectedOutcome,
    pub declared_loss: Option<String>,
    pub malformed: bool,
    pub incremental_matches_full: bool,
    pub selection_boundary_survived: bool,
    pub mark_boundary_survived: bool,
    pub undo_correct: bool,
}

/// Complete result of the frozen real-CST measurement.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub rows: Vec<MeasurementRow>,
    pub merge_recovered: u64,
    pub merge_total: u64,
    pub merge_rate: f64,
    pub wilson95_low: f64,
    pub wilson95_high: f64,
    pub seed_set_sha256: String,
    pub elapsed_ms: u128,
    pub panic_capture_passed: bool,
    pub canonical_roundtrip_supported: bool,
    pub malformed_nonpanic: bool,
    pub selection_and_mark_boundaries_survive: bool,
    pub undo_correct: bool,
    pub no_undeclared_loss: bool,
}

/// Parse the frozen case inventory and execute every row under panic capture.
///
/// # Errors
/// Returns a diagnostic for inventory drift, parse failure, incremental/full
/// divergence, wrong expected outcome, or undo failure.
pub fn run_frozen_measurement() -> Result<Measurement, String> {
    let started = Instant::now();
    let inventory: Inventory = serde_json::from_slice(INVENTORY_BYTES)
        .map_err(|error| format!("invalid frozen real-CST inventory: {error}"))?;
    if inventory.schema_version != 1 {
        return Err(format!(
            "unsupported real-CST inventory schema {}",
            inventory.schema_version
        ));
    }
    if inventory.cases.is_empty() {
        return Err("real-CST inventory is empty".into());
    }

    let mut rows = Vec::with_capacity(inventory.cases.len());
    for case in &inventory.cases {
        let result = catch_unwind(AssertUnwindSafe(|| measure_case(case)))
            .map_err(|_| format!("real-CST case {:?} panicked", case.id))??;
        rows.push(result);
    }
    rows.sort_by(|left, right| {
        (&left.edit_class, &left.case_id, left.seed).cmp(&(
            &right.edit_class,
            &right.case_id,
            right.seed,
        ))
    });

    let merge_rows: Vec<_> = rows
        .iter()
        .filter(|row| row.edit_class == "merge")
        .collect();
    let merge_total = merge_rows.len() as u64;
    let merge_recovered = merge_rows
        .iter()
        .filter(|row| row.observed_outcome == ExpectedOutcome::Recovered)
        .count() as u64;
    let merge_rate = if merge_total == 0 {
        0.0
    } else {
        merge_recovered as f64 / merge_total as f64
    };
    let (wilson95_low, wilson95_high) = wilson_interval(merge_recovered, merge_total);

    let malformed_nonpanic = rows
        .iter()
        .filter(|row| row.malformed)
        .all(|row| row.incremental_matches_full);
    let selection_and_mark_boundaries_survive = rows.iter().all(|row| {
        row.observed_outcome != ExpectedOutcome::Recovered
            || (row.selection_boundary_survived && row.mark_boundary_survived)
    });
    let undo_correct = rows.iter().all(|row| row.undo_correct);
    let no_undeclared_loss = rows.iter().all(|row| {
        row.observed_outcome == ExpectedOutcome::Recovered || row.declared_loss.is_some()
    });

    Ok(Measurement {
        rows,
        merge_recovered,
        merge_total,
        merge_rate,
        wilson95_low,
        wilson95_high,
        seed_set_sha256: seed_set_hash(&inventory.cases),
        elapsed_ms: started.elapsed().as_millis(),
        panic_capture_passed: panic_capture_canary(),
        // tree-sitter retains byte ranges but does not supply a lossless native
        // serializer. Copying the input buffer would be a vacuous round-trip.
        canonical_roundtrip_supported: false,
        malformed_nonpanic,
        selection_and_mark_boundaries_survive,
        undo_correct,
        no_undeclared_loss,
    })
}

fn measure_case(case: &FrozenCase) -> Result<MeasurementRow, String> {
    if case.id.is_empty() || case.edit_class.is_empty() || case.anchor.is_empty() {
        return Err("real-CST inventory contains an empty identity field".into());
    }
    if find_occurrences(case.base.as_bytes(), case.anchor.as_bytes()).len() != 1 {
        return Err(format!(
            "case {:?} base must contain its anchor exactly once",
            case.id
        ));
    }

    let mut parser = MarkdownParser::default();
    let mut incremental = parser
        .parse(case.base.as_bytes(), None)
        .ok_or_else(|| format!("case {:?} base parse returned no tree", case.id))?;
    let base_fingerprint = tree_fingerprint(&incremental);
    let edit = derive_single_edit(&case.base, &case.edited)?;
    incremental.edit(&edit);
    incremental = parser
        .parse(case.edited.as_bytes(), Some(&incremental))
        .ok_or_else(|| format!("case {:?} incremental parse returned no tree", case.id))?;

    // Independent full-reparse construction: new parser, no edited old tree.
    let mut oracle_parser = MarkdownParser::default();
    let full = oracle_parser
        .parse(case.edited.as_bytes(), None)
        .ok_or_else(|| format!("case {:?} full reparse returned no tree", case.id))?;
    let incremental_matches_full = tree_fingerprint(&incremental) == tree_fingerprint(&full);
    if !incremental_matches_full {
        return Err(format!(
            "case {:?} incremental CST diverges from full reparse",
            case.id
        ));
    }

    let incremental_candidates = cst_candidates(&incremental, &case.edited, &case.anchor);
    let observed_outcome = classify_candidates(&incremental_candidates);
    let oracle_candidates = full_reparse_oracle_candidates(&full, &case.edited, &case.anchor);
    let oracle_outcome = classify_candidates(&oracle_candidates);
    if observed_outcome != oracle_outcome {
        return Err(format!(
            "case {:?} recovery and full-reparse oracle disagree: {} vs {}",
            case.id,
            observed_outcome.label(),
            oracle_outcome.label()
        ));
    }
    if observed_outcome != case.expected {
        return Err(format!(
            "case {:?} expected {}, observed {}",
            case.id,
            case.expected.label(),
            observed_outcome.label()
        ));
    }

    let expected_anchor = render_expected_anchor(&oracle_candidates, case.expected);
    let observed_anchor = render_observed_anchor(&incremental_candidates, observed_outcome);
    let boundary_survived = case.expected != ExpectedOutcome::Recovered
        || (incremental_candidates.len() == 1
            && oracle_candidates.len() == 1
            && incremental_candidates[0] == oracle_candidates[0]);

    // Undo is a real incremental reparse in the reverse direction. The final
    // CST must equal a fresh parse of the frozen base, not merely restore bytes.
    let reverse = derive_single_edit(&case.edited, &case.base)?;
    incremental.edit(&reverse);
    let undo_tree = parser
        .parse(case.base.as_bytes(), Some(&incremental))
        .ok_or_else(|| format!("case {:?} undo parse returned no tree", case.id))?;
    let undo_correct = tree_fingerprint(&undo_tree) == base_fingerprint;
    if !undo_correct {
        return Err(format!("case {:?} undo CST differs from base", case.id));
    }

    Ok(MeasurementRow {
        case_id: case.id.clone(),
        edit_class: case.edit_class.clone(),
        seed: case.seed,
        expected_anchor,
        observed_anchor,
        expected_outcome: case.expected,
        observed_outcome,
        declared_loss: case.declared_loss.clone(),
        malformed: case.malformed,
        incremental_matches_full,
        selection_boundary_survived: boundary_survived,
        mark_boundary_survived: boundary_survived,
        undo_correct,
    })
}

fn derive_single_edit(old: &str, new: &str) -> Result<InputEdit, String> {
    let old_bytes = old.as_bytes();
    let new_bytes = new.as_bytes();
    let mut start = old_bytes
        .iter()
        .zip(new_bytes)
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start = start.saturating_sub(1);
    }

    let max_suffix = old_bytes.len().min(new_bytes.len()).saturating_sub(start);
    let mut suffix = old_bytes
        .iter()
        .rev()
        .zip(new_bytes.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(old_bytes.len() - suffix)
        || !new.is_char_boundary(new_bytes.len() - suffix)
    {
        suffix = suffix.saturating_sub(1);
    }

    let old_end_byte = old_bytes.len() - suffix;
    let new_end_byte = new_bytes.len() - suffix;
    Ok(InputEdit {
        start_byte: start,
        old_end_byte,
        new_end_byte,
        start_position: point_at(old, start)?,
        old_end_position: point_at(old, old_end_byte)?,
        new_end_position: point_at(new, new_end_byte)?,
    })
}

fn point_at(text: &str, byte: usize) -> Result<Point, String> {
    if byte > text.len() || !text.is_char_boundary(byte) {
        return Err(format!("invalid UTF-8 edit boundary {byte}"));
    }
    let prefix = &text[..byte];
    let row = prefix
        .as_bytes()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    let column = prefix
        .rfind('\n')
        .map_or(prefix.len(), |newline| prefix.len() - newline - 1);
    Ok(Point { row, column })
}

fn cst_candidates(tree: &MarkdownTree, text: &str, anchor: &str) -> Vec<(usize, usize)> {
    find_occurrences(text.as_bytes(), anchor.as_bytes())
        .into_iter()
        .filter(|(start, end)| named_node_covers(tree.block_tree().root_node(), *start, *end))
        .collect()
}

fn full_reparse_oracle_candidates(
    tree: &MarkdownTree,
    text: &str,
    anchor: &str,
) -> Vec<(usize, usize)> {
    // Deliberately separate implementation from `cst_candidates`: enumerate
    // byte offsets one at a time, then use the fresh tree only as a coverage
    // check. This oracle shares raw bytes/schema and the published grammar, but
    // no edited tree, recovery state, candidate list, or canonicalizer.
    let bytes = text.as_bytes();
    let needle = anchor.as_bytes();
    if needle.is_empty() || needle.len() > bytes.len() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for start in 0..=bytes.len() - needle.len() {
        let end = start + needle.len();
        if &bytes[start..end] == needle
            && named_node_covers(tree.block_tree().root_node(), start, end)
        {
            result.push((start, end));
        }
    }
    result
}

fn named_node_covers(node: Node<'_>, start: usize, end: usize) -> bool {
    if node.start_byte() > start || node.end_byte() < end {
        return false;
    }
    let mut cursor = node.walk();
    if node
        .children(&mut cursor)
        .any(|child| named_node_covers(child, start, end))
    {
        return true;
    }
    node.is_named() && node.kind() != "document"
}

fn find_occurrences(haystack: &[u8], needle: &[u8]) -> Vec<(usize, usize)> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(start, window)| (window == needle).then_some((start, start + needle.len())))
        .collect()
}

fn classify_candidates(candidates: &[(usize, usize)]) -> ExpectedOutcome {
    match candidates.len() {
        0 => ExpectedOutcome::Lost,
        1 => ExpectedOutcome::Recovered,
        _ => ExpectedOutcome::Ambiguous,
    }
}

fn render_expected_anchor(candidates: &[(usize, usize)], expected: ExpectedOutcome) -> String {
    match expected {
        ExpectedOutcome::Recovered if candidates.len() == 1 => {
            format!("{}..{}", candidates[0].0, candidates[0].1)
        }
        ExpectedOutcome::Ambiguous => "<ambiguous>".into(),
        ExpectedOutcome::Lost => "<lost>".into(),
        ExpectedOutcome::Recovered => "<missing>".into(),
    }
}

fn render_observed_anchor(candidates: &[(usize, usize)], outcome: ExpectedOutcome) -> String {
    render_expected_anchor(candidates, outcome)
}

fn tree_fingerprint(tree: &MarkdownTree) -> String {
    let mut fingerprint = tree.block_tree().root_node().to_sexp();
    for inline in tree.inline_trees() {
        fingerprint.push('\n');
        fingerprint.push_str(&inline.root_node().to_sexp());
    }
    fingerprint
}

fn seed_set_hash(cases: &[FrozenCase]) -> String {
    let mut seeds: Vec<_> = cases.iter().map(|case| case.seed).collect();
    seeds.sort_unstable();
    seeds.dedup();
    let mut digest = Sha256::new();
    for seed in seeds {
        digest.update(seed.to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn panic_capture_canary() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        panic!("M17 synthetic panic-capture canary")
    }))
    .is_err()
}

fn wilson_interval(successes: u64, total: u64) -> (f64, f64) {
    if total == 0 {
        return (0.0, 0.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let n = total as f64;
    let p = successes as f64 / n;
    let denominator = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denominator;
    let margin = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((centre - margin).max(0.0), (centre + margin).min(1.0))
}

/// Check that the frozen run explicitly covers all M17.1 synthetic classes.
///
/// # Errors
/// Returns a diagnostic when coverage or a row-level invariant is missing.
pub fn verify_synthetic_coverage(measurement: &Measurement) -> Result<(), String> {
    let positive = measurement
        .rows
        .iter()
        .any(|row| !row.malformed && row.expected_outcome == ExpectedOutcome::Recovered);
    let ambiguous = measurement
        .rows
        .iter()
        .any(|row| row.expected_outcome == ExpectedOutcome::Ambiguous);
    let lost = measurement
        .rows
        .iter()
        .any(|row| row.expected_outcome == ExpectedOutcome::Lost);
    let malformed = measurement.rows.iter().filter(|row| row.malformed).count() >= 3;
    if !(positive && ambiguous && lost && malformed && measurement.panic_capture_passed) {
        return Err("synthetic coverage lacks positive, both negative, malformed, or panic-capture evidence".into());
    }
    if measurement.rows.iter().any(|row| {
        row.expected_outcome != row.observed_outcome
            || !row.incremental_matches_full
            || !row.undo_correct
    }) {
        return Err("one or more synthetic real-CST rows violates its frozen oracle".into());
    }
    if measurement.merge_total < 8 {
        return Err("merge-class inventory has fewer than eight frozen cases".into());
    }
    Ok(())
}

/// Independently derive the maximum supported capability from every D17.3
/// conjunct. The result is capped at Level 2 when any conjunct is false.
#[must_use]
pub fn derived_level(measurement: &Measurement) -> u8 {
    let merge_bar = measurement.merge_total > 0 && measurement.merge_rate >= 0.95;
    if merge_bar
        && measurement.canonical_roundtrip_supported
        && measurement.malformed_nonpanic
        && measurement.selection_and_mark_boundaries_survive
        && measurement.undo_correct
        && measurement.no_undeclared_loss
    {
        3
    } else {
        2
    }
}

/// Render the deterministic reviewed report. Provenance values are supplied by
/// the caller because the report deliberately names the fixed measurement base.
#[must_use]
pub fn render_report(
    measurement: &Measurement,
    git_commit: &str,
    measurement_tree: &str,
) -> String {
    let mut out = String::new();
    writeln!(out, "status: measured").unwrap();
    writeln!(out, "grammar_adr: {GRAMMAR_ADR}").unwrap();
    writeln!(out, "substrate: {SUBSTRATE}").unwrap();
    writeln!(out, "git_commit: {git_commit}").unwrap();
    writeln!(out, "measurement_tree: {measurement_tree}").unwrap();
    writeln!(out, "declared_level: {DECLARED_LEVEL}").unwrap();
    writeln!(out, "merge_recovered: {}", measurement.merge_recovered).unwrap();
    writeln!(out, "merge_total: {}", measurement.merge_total).unwrap();
    writeln!(out, "merge_rate: {:.6}", measurement.merge_rate).unwrap();
    writeln!(out, "wilson95_low: {:.6}", measurement.wilson95_low).unwrap();
    writeln!(out, "wilson95_high: {:.6}", measurement.wilson95_high).unwrap();
    writeln!(out, "seed_set_sha256: {}", measurement.seed_set_sha256).unwrap();
    writeln!(out, "elapsed_ms: {}", measurement.elapsed_ms).unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "level_3_blocker: no independently qualified lossless native CST serializer"
    )
    .unwrap();
    writeln!(
        out,
        "upstream_risk: tree-sitter-md documents known Markdown output inaccuracies"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| case ID | edit class | seed | expected anchor | observed anchor | outcome | declared loss |").unwrap();
    writeln!(out, "|---|---|---:|---|---|---|---|").unwrap();
    for row in &measurement.rows {
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            row.case_id,
            row.edit_class,
            row.seed,
            row.expected_anchor,
            row.observed_anchor,
            row.observed_outcome.label(),
            row.declared_loss.as_deref().unwrap_or("none")
        )
        .unwrap();
    }
    out
}

/// Redact only the measured wall-clock value for deterministic golden compare.
#[must_use]
pub fn redact_elapsed(report: &str) -> String {
    let mut out = String::with_capacity(report.len());
    for line in report.lines() {
        if line.starts_with("elapsed_ms: ") {
            out.push_str("elapsed_ms: <MEASURED>\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
