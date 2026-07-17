//! The spec-debt meter: per-phase `passing surface vs ignored backlog` counts,
//! scanned statically from test sources.
//!
//! Convention it relies on: every deferred test is
//! `#[ignore = "Phase X…: reason"]`, so the declared backlog is greppable.
//! v0 is a static scan (declared debt); wiring actual nextest pass/fail JSON
//! in is M-milestone work noted in docs/implementation-plan.md.

use std::collections::BTreeMap;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

/// Declared test debt, grouped by phase tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebtReport {
    /// phase tag → ignored (deferred) test count.
    pub ignored_by_phase: BTreeMap<String, u64>,
    /// Number of active (non-ignored) `#[test]` functions found.
    pub active_tests: u64,
}

impl DebtReport {
    /// Total declared backlog.
    #[must_use]
    pub fn total_ignored(&self) -> u64 {
        self.ignored_by_phase.values().sum()
    }

    /// Render the meter.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Liminal spec-debt meter (declared test surface)");
        let _ = writeln!(out, "================================================");
        for (phase, count) in &self.ignored_by_phase {
            let _ = writeln!(out, "{phase:<12} {count:>4} deferred");
        }
        let _ = writeln!(
            out,
            "{:<12} {:>4} active",
            "(passing now)", self.active_tests
        );
        let _ = writeln!(
            out,
            "\ntotal backlog: {} tests — a phase is done when its tests flip from ignored to passing.",
            self.total_ignored()
        );
        let _ = writeln!(out, "run `just test` for pass/fail of the active surface.");
        out
    }
}

/// Scan a workspace root for declared debt: every `#[ignore = "…"]` whose
/// reason starts with `Phase` is bucketed by its phase tag (the text before
/// the first `:`); every `#[test]` not preceded by an ignore is active.
pub fn scan_workspace_debt(root: &Utf8Path) -> anyhow::Result<DebtReport> {
    let mut report = DebtReport::default();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            let name = path.file_name().unwrap_or_default();
            if path.is_dir() {
                if name != "target" && name != ".git" && name != "spec" {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs") {
                scan_file(&path, &mut report);
            }
        }
    }
    Ok(report)
}

fn scan_file(path: &Utf8Path, report: &mut DebtReport) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let mut pending_ignore: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#[ignore = \"") {
            let reason = rest.split('"').next().unwrap_or_default();
            pending_ignore = Some(reason.to_owned());
        } else if trimmed.starts_with("fn ") && trimmed.contains('(') {
            if let Some(reason) = pending_ignore.take() {
                let phase = reason.split(':').next().unwrap_or("unphased").trim();
                let key = if phase.starts_with("Phase") {
                    phase.to_owned()
                } else {
                    "unphased".to_owned()
                };
                *report.ignored_by_phase.entry(key).or_default() += 1;
            }
        } else if trimmed == "#[test]" {
            // A bare #[test] with no pending ignore counts as active once its
            // fn line arrives; approximate by counting here when nothing is
            // pending (the fn branch above consumes ignored ones).
            if pending_ignore.is_none() {
                report.active_tests += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_finds_the_declared_backlog() {
        let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let report = scan_workspace_debt(&root).unwrap();
        assert!(
            report.total_ignored() >= 12,
            "at minimum the 12 R4 §10 gate tests must be declared, found {}",
            report.total_ignored()
        );
        assert!(
            report
                .ignored_by_phase
                .keys()
                .any(|k| k.starts_with("Phase -1")),
            "Phase -1 backlog must be visible: {:?}",
            report.ignored_by_phase
        );
    }
}
