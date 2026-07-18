//! The minimal M06 query `render_paragraphs(notes.md)` — the resolved
//! component's text, semantic block texts joined `"\n\n"`. It reads buffer
//! bytes from `SYS_BLOB["buf/<client>/<buffer>/<generation>"]` when the
//! component is a `BufferGeneration`, else the durable file bytes. This is the
//! computation whose memoization proves component-granular invalidation.

use camino::Utf8Path;
use liminal_graph::GraphStore;
use liminal_graph::ns::SYS_BLOB;
use liminal_id::{JurisdictionKey, PathId};
use liminal_revision::{BasisComponent, WorkspaceBasis};

/// Render the paragraphs of `path` as resolved by `basis`: the block texts of
/// the resolved bytes, joined by blank lines.
#[must_use]
pub fn render_paragraphs(
    store: &GraphStore,
    root: &Utf8Path,
    basis: &WorkspaceBasis,
    path: &PathId,
) -> String {
    let key = JurisdictionKey::Path(path.clone());
    let bytes = match basis.components.get(&key) {
        Some(BasisComponent::BufferGeneration {
            client,
            buffer,
            generation,
            ..
        }) => {
            let blob_key = format!("buf/{client}/{buffer}/{generation}");
            store
                .get_aux(SYS_BLOB, &blob_key)
                .ok()
                .flatten()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        }
        // Durable file bytes (FileContent or absent → read from disk).
        _ => std::fs::read_to_string(root.join(&path.0)).unwrap_or_default(),
    };

    liminal_source::paragraph::parse(&bytes)
        .into_iter()
        .map(|b| b.text)
        .collect::<Vec<_>>()
        .join("\n\n")
}
