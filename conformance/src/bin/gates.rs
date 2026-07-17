//! `gates` — render the spec-debt meter (`just gates`).

use camino::Utf8PathBuf;

fn main() -> anyhow::Result<()> {
    // Workspace root = two levels up from this crate's manifest dir.
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .map(camino::Utf8Path::to_path_buf)
        .unwrap_or(manifest);
    let report = liminal_conformance::scan_workspace_debt(&root)?;
    print!("{}", report.render());
    Ok(())
}
