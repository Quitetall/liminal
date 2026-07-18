//! Content-addressed text blobs in the coordinating store's `SYS_BLOB`
//! namespace (M04). Keyed by BLAKE3 hex; value is the UTF-8 text as a JSON
//! string, so blobs stay `grep`-able in the NDJSON log (falsification > speed).
//!
//! Repair planning persists the base and merged bytes here so the safety
//! predicate can recompute structural disjointness later without the raw
//! editor buffer (DG-4.1 reading (a)).

use liminal_graph::ns::SYS_BLOB;
use liminal_graph::{GraphStore, GraphTxn, StoreError};
use liminal_id::ContentHash;

/// Read a blob by its content hash. `Ok(None)` if absent.
pub fn get(store: &GraphStore, hash: ContentHash) -> Result<Option<String>, StoreError> {
    Ok(store
        .get_aux(SYS_BLOB, &hash.to_hex())?
        .and_then(|v| v.as_str().map(str::to_owned)))
}

/// Queue a blob write (`SYS_BLOB[hash] = text`) in an open transaction.
/// Idempotent: content-addressed, so re-storing identical bytes is a no-op
/// overwrite.
pub fn put(txn: &mut GraphTxn<'_>, text: &str) -> Result<ContentHash, StoreError> {
    let hash = ContentHash::of(text.as_bytes());
    txn.put_aux(
        SYS_BLOB,
        &hash.to_hex(),
        serde_json::Value::String(text.to_owned()),
    )?;
    Ok(hash)
}
