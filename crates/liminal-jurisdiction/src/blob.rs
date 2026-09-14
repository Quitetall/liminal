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
    if let Some(value) = store.get_aux(SYS_BLOB, &hash.to_hex())? {
        let text = value
            .as_str()
            .ok_or_else(|| StoreError::Corrupt("text blob has wrong shape".into()))?;
        if ContentHash::of(text.as_bytes()) != hash {
            return Err(StoreError::Corrupt("text blob hash mismatch".into()));
        }
        return Ok(Some(text.to_owned()));
    }
    // Committed file mirrors also carry bytes, not authority by their path.
    // Recompute the requested hash before using them; do not add a write or
    // silently substitute a different file version for a missing preimage.
    for (key, value) in store.scan_aux(SYS_BLOB)? {
        if key.starts_with("file/")
            && let Some(text) = value.as_str()
            && ContentHash::of(text.as_bytes()) == hash
        {
            return Ok(Some(text.to_owned()));
        }
    }
    Ok(None)
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
