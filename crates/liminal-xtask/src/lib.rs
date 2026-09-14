//! Workspace maintenance helpers.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;

/// Return the workspace root for commands launched from any crate.
pub fn repo_root() -> Result<Utf8PathBuf> {
    let mut dir = std::env::current_dir().context("current directory")?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("docs/execution/00-protocol.md").is_file() {
            return Utf8PathBuf::from_path_buf(dir).map_err(|path| {
                anyhow::anyhow!("workspace path is not UTF-8: {}", path.display())
            });
        }
        if !dir.pop() {
            anyhow::bail!("could not find liminal workspace root");
        }
    }
}

pub mod bench;
/// Candidate-only formal-safety registry verification (v4 §§7.4 and 7.8).
pub mod formal;
/// HAQP-1 packet verification.
pub mod haq;
mod vstd_admission;
