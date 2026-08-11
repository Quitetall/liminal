//! Shared Phase 1 source pipeline used by `lim` and conformance tests.

mod format;

pub use format::{
    FormatOutcome, check_workspace, expand_workspace, format_workspace,
    format_workspace_with_failure_after, has_markdown_files,
};
