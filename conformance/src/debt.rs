//! The spec-debt meter: per-phase `passing surface vs ignored backlog` counts,
//! scanned statically from test sources.
//!
//! Convention it relies on: every deferred test is
//! `#[ignore = "Phase X…: reason"]`, so the declared backlog is greppable.
//! v0 is a static scan (declared debt); wiring actual nextest pass/fail JSON
//! in is M-milestone work noted in docs/implementation-plan.md.

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

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

/// Scan a workspace root for declared debt: every ignored `#[test]` whose
/// reason starts with `Phase` is bucketed by its phase tag (the text before
/// the first `:`); every other `#[test]` is active.
pub fn scan_workspace_debt(root: &Utf8Path) -> anyhow::Result<DebtReport> {
    if is_git_repository(root) {
        return scan_tracked_workspace_debt(root);
    }

    let mut report = DebtReport::default();
    scan_filesystem_debt(root, &mut report);
    Ok(report)
}

fn is_git_repository(root: &Utf8Path) -> bool {
    let mut command = sanitized_git();
    command
        .current_dir(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success() && output.stdout == b"true\n")
        .unwrap_or(false)
}

/// Scan only repository-tracked Rust files. The meter is a statement about the
/// declared test surface, so an untracked checkout, fixture, or nested project
/// must not change its denominator or active count.
fn scan_tracked_workspace_debt(root: &Utf8Path) -> anyhow::Result<DebtReport> {
    let output = sanitized_git()
        .current_dir(root)
        .args(["ls-files", "-z", "--", "*.rs"])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "tracked debt scan could not list repository files"
    );

    let mut report = DebtReport::default();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
    {
        let relative = validate_git_path(raw)?;
        scan_file(&root.join(relative), &mut report);
    }
    Ok(report)
}

fn sanitized_git() -> Command {
    // Remove repository/index redirection supplied by the caller. Git config
    // remains host policy; this helper only protects repository selection.
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
}

fn validate_git_path(raw: &[u8]) -> anyhow::Result<&Utf8Path> {
    let relative = std::str::from_utf8(raw)
        .map_err(|_| anyhow::anyhow!("git returned a non-UTF-8 Rust path"))?;
    let relative = Utf8Path::new(relative);
    anyhow::ensure!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|component| matches!(component, camino::Utf8Component::Normal(_))),
        "git returned an unsafe Rust path"
    );
    Ok(relative)
}

/// Recursive fallback used by in-memory/scratch test roots that are not Git
/// repositories. Real repository meters always take the tracked path above.
fn scan_filesystem_debt(root: &Utf8Path, report: &mut DebtReport) {
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
            // Recurse only through real directories. Following a symlinked
            // directory can escape the workspace or loop back into its parent.
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if name != "target" && name != ".git" && name != "spec" {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs") {
                // Preserve existing behavior for a symlinked Rust file; the
                // path itself was selected by the caller's filesystem scan.
                scan_file(&path, report);
            }
        }
    }
}

fn scan_file(path: &Utf8Path, report: &mut DebtReport) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let mut pending_ignore: Option<String> = None;
    let mut pending_test = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#[ignore = \"") {
            let reason = rest.split('"').next().unwrap_or_default();
            pending_ignore = Some(reason.to_owned());
        } else if trimmed == "#[ignore]" {
            pending_ignore = Some(String::new());
        } else if trimmed.starts_with("fn ") && trimmed.contains('(') {
            if pending_test {
                if let Some(reason) = pending_ignore.take() {
                    let phase = reason.split(':').next().unwrap_or("unphased").trim();
                    let key = if phase.starts_with("Phase") {
                        phase.to_owned()
                    } else {
                        "unphased".to_owned()
                    };
                    *report.ignored_by_phase.entry(key).or_default() += 1;
                } else {
                    report.active_tests += 1;
                }
            }
            pending_test = false;
            pending_ignore = None;
        } else if trimmed == "#[test]" {
            pending_test = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_test_is_deferred_not_active() {
        let root = std::env::temp_dir().join(format!(
            "liminal-debt-meter-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create debt-meter probe directory");
        let test_attribute = "#[test]";
        let ignore_attribute = "#[ignore = \"Phase 3: deferred case\"]";
        let source = format!(
            "{test_attribute}\nfn active_case() {{}}\n\n\
             {test_attribute}\n{ignore_attribute}\nfn deferred_case() {{}}\n"
        );
        fs::write(root.join("surface.rs"), source).expect("write debt-meter probe source");
        let root = Utf8PathBuf::from_path_buf(root).expect("probe path is UTF-8");

        let report = scan_workspace_debt(&root).expect("scan debt-meter probe");

        fs::remove_dir_all(&root).expect("remove debt-meter probe directory");
        assert_eq!(report.active_tests, 1);
        assert_eq!(report.total_ignored(), 1);
        assert_eq!(report.ignored_by_phase.get("Phase 3"), Some(&1));
    }

    #[test]
    fn bare_ignore_is_unphased_debt_not_active() {
        let root = std::env::temp_dir().join(format!(
            "liminal-debt-meter-bare-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create bare-ignore probe directory");
        fs::write(
            root.join("surface.rs"),
            "#[test]\n#[ignore]\nfn deferred_without_reason() {}\n",
        )
        .expect("write bare-ignore probe source");
        let root = Utf8PathBuf::from_path_buf(root).expect("probe path is UTF-8");

        let report = scan_workspace_debt(&root).expect("scan bare-ignore probe");

        fs::remove_dir_all(&root).expect("remove bare-ignore probe directory");
        assert_eq!(report.active_tests, 0);
        assert_eq!(report.total_ignored(), 1);
        assert_eq!(report.ignored_by_phase.get("unphased"), Some(&1));
    }

    #[test]
    fn meter_finds_the_declared_backlog() {
        let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let report = scan_workspace_debt(&root).unwrap();
        assert!(
            report.total_ignored() >= 12,
            "at minimum the 12 R4 §10 gate tests must be declared, found {}",
            report.total_ignored()
        );
        // The meter must partition the backlog by phase and find a non-empty,
        // well-formed set. This originally pinned "Phase -1 must be visible",
        // but M11.8 completed Phase -1 (its ignored count is now 0 by design —
        // M12.4 targets exactly that), so the assertion is re-expressed as the
        // property it was really testing: the phase partition is populated and
        // every key is a real phase label (DG-11.5).
        assert!(
            !report.ignored_by_phase.is_empty()
                && report
                    .ignored_by_phase
                    .keys()
                    .all(|k| k.starts_with("Phase ")),
            "the phase-partitioned backlog must be populated with real phase \
             labels: {:?}",
            report.ignored_by_phase
        );
    }

    // ── M17.5 Stage 3 tier 2 (F-30 continued) ─────────────────────────────
    // Five survivors here. `render` is the SPEC-DEBT METER — the artifact
    // M17.8 reconciles the Phase 0 ADR against — and both whole-function
    // replacements survived, so a meter that printed nothing at all would have
    // passed every test in the tree.

    /// Kills `render -> String::new()` and `-> "xyzzy".into()`.
    ///
    /// The meter's numbers are the point, not its prose: a reader reconciling
    /// M17.8 needs the per-phase counts, the active total and the backlog to
    /// appear, and to be the numbers the report actually holds.
    #[test]
    fn the_meter_renders_the_numbers_it_holds() {
        let mut report = DebtReport {
            active_tests: 477,
            ..DebtReport::default()
        };
        report.ignored_by_phase.insert("Phase 4".to_owned(), 4);
        report.ignored_by_phase.insert("Phase 7".to_owned(), 2);

        let rendered = report.render();
        assert!(
            rendered.contains("Phase 4") && rendered.contains("4 deferred"),
            "each phase and its count must appear: {rendered}"
        );
        assert!(rendered.contains("Phase 7"), "every phase, not just one");
        assert!(
            rendered.contains("477"),
            "the active total must appear: {rendered}"
        );
        assert!(
            rendered.contains("total backlog: 6"),
            "the backlog must be the SUM of the phases, not a constant: {rendered}"
        );

        // ...and it must track the data, or it is a fixed string that happens
        // to contain the right digits once.
        report.active_tests = 1;
        let changed = report.render();
        assert_ne!(
            rendered, changed,
            "the meter must be a function of the report, not a constant"
        );
        assert!(changed.contains(" 1 active") || changed.contains("1 active"));
    }

    /// Kills the three `&&` -> `||` mutants in the directory and attribute
    /// walks: each excluded directory must be excluded for its OWN reason, and
    /// an ignored test must need BOTH `#[test]` and `#[ignore]`.
    #[test]
    fn the_scanner_excludes_each_directory_on_its_own_merit() {
        let scratch = liminal_scratch::ScratchDir::new("debt-scan").expect("scratch");
        let root: &Utf8Path = &scratch;

        // A live ignored test at the root.
        fs::write(
            root.join("live.rs"),
            "#[test]\n#[ignore = \"Phase 9: deferred\"]\nfn deferred() {}\n#[test]\nfn active() {}\n",
        )
        .expect("write");

        // Each excluded directory holds a test that must NOT be counted. With
        // `||` the exclusion widens and real source stops being scanned; the
        // counts below pin both directions.
        for skipped in ["target", ".git", "spec"] {
            fs::create_dir_all(root.join(skipped)).expect("mkdir");
            fs::write(
                root.join(skipped).join("hidden.rs"),
                "#[test]\n#[ignore = \"Phase 9: hidden\"]\nfn hidden() {}\n",
            )
            .expect("write");
        }
        // ...and a directory that is NOT excluded must still be walked.
        fs::create_dir_all(root.join("src")).expect("mkdir");
        fs::write(
            root.join("src").join("nested.rs"),
            "#[test]\nfn nested_active() {}\n",
        )
        .expect("write");

        let report = scan_workspace_debt(root).expect("scan");
        assert_eq!(
            report.total_ignored(),
            1,
            "only the root's deferred test counts; target/.git/spec are excluded"
        );
        assert_eq!(
            report.active_tests, 2,
            "the root's active test AND the nested one, so exclusion did not widen"
        );
    }

    #[test]
    fn a_non_repository_meter_uses_the_filesystem_fallback() {
        let scratch = liminal_scratch::ScratchDir::new("debt-fallback").expect("scratch");
        let root: &Utf8Path = &scratch;
        assert!(
            !is_git_repository(root),
            "fallback fixture must not be a repository"
        );
        fs::write(
            root.join("fallback.rs"),
            "#[test]\n#[ignore = \"Phase 9: fallback\"]\nfn deferred() {}\n#[test]\nfn active() {}\n",
        )
        .expect("write");

        let report = scan_workspace_debt(root).expect("scan non-repository workspace");
        assert_eq!(report.active_tests, 1);
        assert_eq!(report.total_ignored(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_fallback_does_not_follow_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let scratch = liminal_scratch::ScratchDir::new("debt-symlink-dir").expect("scratch");
        let root: &Utf8Path = &scratch;
        fs::create_dir(root.join("real")).expect("mkdir");
        fs::write(root.join("real/one.rs"), "#[test]\nfn active() {}\n").expect("write");
        symlink("..", root.join("real/parent")).expect("symlink");

        let report = scan_workspace_debt(root).expect("scan symlinked workspace");
        assert_eq!(report.active_tests, 1, "symlinked parent must not recurse");
    }

    #[test]
    fn tracked_path_validation_rejects_escape_and_non_utf8_bytes() {
        assert!(validate_git_path(b"src/lib.rs").is_ok());
        for raw in [
            b"/escape.rs" as &[u8],
            b"../escape.rs",
            b"src/../escape.rs",
            b"./src/lib.rs",
        ] {
            assert!(validate_git_path(raw).is_err(), "path should be refused");
        }
        assert!(validate_git_path(&[0xff, b'.', b'r', b's']).is_err());
    }

    #[test]
    fn a_repository_meter_ignores_untracked_rust_trees() {
        let scratch = liminal_scratch::ScratchDir::new("debt-tracked").expect("scratch");
        let root: &Utf8Path = &scratch;
        let init = Command::new("git")
            .current_dir(root)
            .args(["init", "--quiet"])
            .output()
            .expect("git init");
        assert!(init.status.success(), "git init failed");

        fs::write(root.join("tracked.rs"), "#[test]\nfn active() {}\n").expect("write");
        let add = Command::new("git")
            .current_dir(root)
            .args(["add", "tracked.rs"])
            .output()
            .expect("git add");
        assert!(add.status.success(), "git add failed");

        fs::create_dir_all(root.join("untracked-project/src")).expect("mkdir");
        fs::write(
            root.join("untracked-project/src/lib.rs"),
            "#[test]\n#[ignore = \"Phase 9: untracked\"]\nfn hidden() {}\n",
        )
        .expect("write");

        let report = scan_workspace_debt(root).expect("scan tracked workspace");
        assert_eq!(report.active_tests, 1, "only tracked tests count as active");
        assert_eq!(
            report.total_ignored(),
            0,
            "untracked deferred tests must not enter the declared backlog"
        );
    }

    /// Kills `starts_with("fn ") && contains('(')` -> `||` at the declaration
    /// test.
    ///
    /// Under `||` any line holding a paren reads as a function declaration, so
    /// an attribute between `#[test]` and `fn` — `#[cfg(unix)]`, and every
    /// `#[should_panic(expected = ...)]` in this repo — consumes the pending
    /// state and the real `fn` two lines later counts as nothing. The meter
    /// would silently under-report the surface it exists to measure.
    #[test]
    fn an_attribute_containing_a_paren_is_not_a_function_declaration() {
        let scratch = liminal_scratch::ScratchDir::new("debt-paren").expect("scratch");
        let root: &Utf8Path = &scratch;
        fs::write(
            root.join("attrs.rs"),
            "#[test]\n#[cfg(unix)]\n#[ignore = \"Phase 9: deferred\"]\nfn deferred() {}\n\
             #[test]\n#[should_panic(expected = \"boom\")]\nfn active() {}\n",
        )
        .expect("write");

        let report = scan_workspace_debt(root).expect("scan");
        assert_eq!(
            report.total_ignored(),
            1,
            "the deferred test must survive an intervening #[cfg(...)] attribute"
        );
        assert_eq!(
            report.active_tests, 1,
            "the active test must survive an intervening #[should_panic(...)]"
        );
    }

    /// An `#[ignore]` with no `#[test]` above it is not a deferred test, and a
    /// `#[test]` with no `#[ignore]` is not deferred either — the `&&` in
    /// `scan_file` decides both.
    #[test]
    fn deferral_requires_both_attributes() {
        let scratch = liminal_scratch::ScratchDir::new("debt-attrs").expect("scratch");
        let root: &Utf8Path = &scratch;
        fs::write(
            root.join("mixed.rs"),
            "#[ignore = \"Phase 9: not a test\"]\nfn bare_function() {}\n\
             #[test]\nfn plain_test() {}\n",
        )
        .expect("write");

        let report = scan_workspace_debt(root).expect("scan");
        assert_eq!(
            report.total_ignored(),
            0,
            "an #[ignore] without #[test] defers nothing"
        );
        assert_eq!(report.active_tests, 1, "the plain test is active");
    }
}
