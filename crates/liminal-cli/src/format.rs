//! Phase 1 source-file pipeline for `lim fmt`, `lim check`, and `lim expand`
//! (v4 §§8.4, 20, 22; M18–M20).

use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use liminal_format::{Formatter, MarkdownFormatter, Phase1Document};
use liminal_hir::SourceDialect;
use liminal_id::{ContentHash, JurisdictionKey, PathId, SourceId, TransactionId};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};

/// Result of one formatting pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOutcome {
    /// Number of files atomically replaced.
    pub changed_files: usize,
}

/// Format every Markdown Holder below `root` after a complete preflight.
///
/// All parsing and emission happens before the first write. Changed files are
/// staged and replaced one at a time with hash-checked atomic replacement. A
/// deliberate `LIMINAL_FMT_FAIL_AFTER=N` fault stops before replacement `N+1`,
/// leaving completed replacements durable and remaining staged files cleaned.
pub fn format_workspace(root: &Utf8Path) -> Result<FormatOutcome> {
    format_workspace_inner(root, None)
}

/// Format with deterministic pre-commit interruption injection for conformance
/// tests. `Some(n)` permits exactly `n` replacements, then fails before the
/// next replacement. This does not alter normal CLI behavior.
pub fn format_workspace_with_failure_after(
    root: &Utf8Path,
    fail_after: Option<usize>,
) -> Result<FormatOutcome> {
    format_workspace_inner(root, fail_after)
}

fn format_workspace_inner(root: &Utf8Path, fail_after: Option<usize>) -> Result<FormatOutcome> {
    let plans = preflight(root)?;
    if plans.is_empty() {
        bail!("no governed Markdown Holder found below {root}");
    }

    let mut staged = Vec::new();
    for plan in plans.iter().filter(|plan| plan.bytes != plan.original) {
        match liminal_source::stage(&plan.path, &plan.bytes) {
            Ok(write) => staged.push((plan, write)),
            Err(error) => {
                for (_, pending) in staged {
                    let _ = pending.abandon();
                }
                return Err(error).with_context(|| format!("stage {}", plan.path));
            }
        }
    }

    let mut committed = 0usize;
    let mut pending_writes = staged.into_iter();
    while let Some((plan, pending)) = pending_writes.next() {
        if fail_after.is_some_and(|limit| committed >= limit) {
            let path = pending.target_path().to_owned();
            let _ = pending.abandon();
            for (_, leftover) in pending_writes {
                let _ = leftover.abandon();
            }
            return Err(anyhow::anyhow!(
                "injected formatter interruption before replacement {path}"
            ));
        }
        if let Err(error) = pending.commit_if(Some(ContentHash::of(&plan.original))) {
            for (_, leftover) in pending_writes {
                let _ = leftover.abandon();
            }
            return Err(error).with_context(|| format!("replace {}", plan.path));
        }
        committed += 1;
    }
    Ok(FormatOutcome {
        changed_files: committed,
    })
}

/// Parse and validate every governed Markdown Holder without writing.
pub fn check_workspace(root: &Utf8Path) -> Result<()> {
    let plans = preflight(root)?;
    if plans.is_empty() {
        bail!("no governed Markdown Holder found below {root}");
    }
    Ok(())
}

/// Whether `root` contains at least one governed Markdown Holder.
#[must_use]
pub fn has_markdown_files(root: &Utf8Path) -> bool {
    markdown_files(root).is_ok_and(|files| !files.is_empty())
}

/// Emit one canonical M19 stable debug-JSON document for `root`.
pub fn expand_workspace(root: &Utf8Path) -> Result<Vec<u8>> {
    let plans = preflight(root)?;
    let plan = plans
        .first()
        .ok_or_else(|| anyhow::anyhow!("no governed Markdown Holder found below {root}"))?;
    let graph = liminal_cir::to_debug_v1(
        &plan.document.graph,
        plan.holder.clone(),
        plan.source.clone(),
    );
    liminal_cir::serialize_debug_v1(&graph).map_err(|error| anyhow::anyhow!(error))
}

#[derive(Debug)]
struct FilePlan {
    path: Utf8PathBuf,
    original: Vec<u8>,
    bytes: Vec<u8>,
    document: Phase1Document,
    holder: JurisdictionKey,
    source: liminal_source::SourceBasis,
}

fn preflight(root: &Utf8Path) -> Result<Vec<FilePlan>> {
    let paths = markdown_files(root)?;
    let mut plans = Vec::with_capacity(paths.len());
    for path in paths {
        let original = fs::read(&path).with_context(|| format!("read {path}"))?;
        let source = std::str::from_utf8(&original)
            .with_context(|| format!("{path}: governed source must be UTF-8"))?;
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .as_str()
            .trim_start_matches('/')
            .to_owned();
        let source_id = SourceId::from_name(&relative);
        let path_id = PathId(relative.clone().into());
        let holder = JurisdictionKey::Path(path_id.clone());
        let source_basis = liminal_source::SourceBasis {
            source: source_id,
            content_hash: ContentHash::of(&original),
        };
        let basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::FileContent {
                    path: path_id,
                    hash: source_basis.content_hash,
                },
            )]),
        };
        let formatter = MarkdownFormatter::new(
            source_id,
            holder.clone(),
            basis,
            if source.starts_with("#!liminal-explicit-v1") {
                SourceDialect::ExplicitV1
            } else {
                SourceDialect::CompactOrExplicitV1
            },
        );
        let document = formatter
            .parse(source)
            .with_context(|| format!("parse {path}"))?;
        let first = formatter
            .emit_in_dialect(&document, formatter_dialect(source))
            .with_context(|| format!("format {path}"))?;
        let second_document = formatter
            .parse(&first)
            .with_context(|| format!("reparse formatted {path}"))?;
        let second = formatter
            .emit_in_dialect(&second_document, formatter_dialect(&first))
            .with_context(|| format!("reformat {path}"))?;
        if first != second {
            bail!("{path}: formatter is not idempotent");
        }
        if document != second_document {
            bail!("{path}: formatting changed semantic graph");
        }
        plans.push(FilePlan {
            path,
            original,
            bytes: first.into_bytes(),
            document,
            holder,
            source: source_basis,
        });
    }
    Ok(plans)
}

fn formatter_dialect(source: &str) -> SourceDialect {
    if source.starts_with("#!liminal-explicit-v1") {
        SourceDialect::ExplicitV1
    } else {
        SourceDialect::CompactOrExplicitV1
    }
}

fn markdown_files(root: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let metadata = fs::metadata(&dir).with_context(|| format!("stat {dir}"))?;
        if metadata.is_file() {
            if dir.extension() == Some("md") {
                found.push(dir);
            }
            continue;
        }
        for entry in fs::read_dir(&dir).with_context(|| format!("read directory {dir}"))? {
            let entry = entry.with_context(|| format!("read entry in {dir}"))?;
            let path = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|path| anyhow::anyhow!("non-UTF-8 path: {}", path.display()))?;
            if path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.starts_with('.') || matches!(name, "target" | "fuzz"))
            {
                continue;
            }
            stack.push(path);
        }
    }
    found.sort();
    Ok(found)
}
