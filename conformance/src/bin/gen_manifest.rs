//! `gen-manifest` — held-out corpus MANIFEST.b3 generator (AM-11.1; D11.6:
//! a conformance bin, never a `lim` subcommand).
//!
//! ```text
//! gen-manifest <version-dir>
//! ```
//!
//! Walks `<version-dir>` (e.g. `conformance/corpora/heldout/v1`), BLAKE3s
//! every file except the manifest itself, and writes `MANIFEST.b3` in the
//! `verify_heldout_manifest` wire format (`<hex>  <relpath>`, two spaces,
//! sorted, LF). Part of the D11.7 locking ceremony: assemble →
//! gen-manifest → commit → golden flip → one scorecard run per profile.

use camino::Utf8PathBuf;

fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: gen-manifest <version-dir>"))?;
    let dir = Utf8PathBuf::from(dir);
    anyhow::ensure!(dir.is_dir(), "{dir}: not a directory");
    liminal_conformance::harness::generate_heldout_manifest(&dir)?;
    // Round-trip immediately: a manifest that does not verify is a bug in
    // the generator, caught before the ceremony commits it.
    liminal_conformance::harness::verify_heldout_manifest(&dir)?;
    Ok(())
}
