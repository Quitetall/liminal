//! Exact UTF-8 Holder view for the Phase 1 CST boundary (v4 §5.3, §9, §22).

use liminal_id::{ContentHash, SourceId};
use ropey::{Rope, RopeSlice};
use serde::{Deserialize, Serialize};

/// Rowan's `TextSize` upper bound, expressed in bytes.
pub const MAX_SOURCE_BYTES: u64 = u32::MAX as u64;

/// Exact source identity selected by Jurisdiction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBasis {
    /// Durable or external source identity.
    pub source: SourceId,
    /// Hash of exact source bytes.
    pub content_hash: ContentHash,
}

/// Immutable UTF-8 rope view over one selected Holder.
#[derive(Debug, Clone)]
pub struct Utf8HolderView {
    basis: SourceBasis,
    text: Rope,
}

impl Utf8HolderView {
    /// Validate bytes, hash, and Rowan size before constructing the rope.
    pub fn from_bytes(basis: SourceBasis, bytes: &[u8]) -> Result<Self, SourceLoadError> {
        let actual = u64::try_from(bytes.len()).expect("usize fits u64");
        if actual > MAX_SOURCE_BYTES {
            return Err(SourceLoadError::TooLarge {
                actual,
                maximum: MAX_SOURCE_BYTES,
            });
        }
        let actual_hash = ContentHash::of(bytes);
        if actual_hash != basis.content_hash {
            return Err(SourceLoadError::HashMismatch {
                expected: basis.content_hash,
                actual: actual_hash,
            });
        }
        let text = std::str::from_utf8(bytes).map_err(|error| SourceLoadError::InvalidUtf8 {
            valid_up_to: u64::try_from(error.valid_up_to()).expect("usize fits u64"),
            error_len: error
                .error_len()
                .map(|len| u64::try_from(len).expect("usize fits u64")),
        })?;
        Ok(Self {
            basis,
            text: Rope::from_str(text),
        })
    }

    /// Exact Holder basis.
    #[must_use]
    pub fn basis(&self) -> &SourceBasis {
        &self.basis
    }

    /// UTF-8 byte length.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.text.len_bytes()
    }

    /// Borrow a UTF-8 byte-aligned slice.
    pub fn byte_slice(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<RopeSlice<'_>, SourceSliceError> {
        if range.start > range.end || range.end > self.len_bytes() {
            return Err(SourceSliceError::OutOfBounds {
                start: range.start,
                end: range.end,
                len: self.len_bytes(),
            });
        }
        let start = self.text.try_byte_to_char(range.start).map_err(|_| {
            SourceSliceError::NotCharBoundary {
                offset: range.start,
            }
        })?;
        if self.text.char_to_byte(start) != range.start {
            return Err(SourceSliceError::NotCharBoundary {
                offset: range.start,
            });
        }
        let end = self
            .text
            .try_byte_to_char(range.end)
            .map_err(|_| SourceSliceError::NotCharBoundary { offset: range.end })?;
        if self.text.char_to_byte(end) != range.end {
            return Err(SourceSliceError::NotCharBoundary { offset: range.end });
        }
        Ok(self.text.slice(start..end))
    }

    /// Copy exact UTF-8 source text.
    #[must_use]
    #[allow(
        clippy::inherent_to_string,
        reason = "frozen M18 API requires this exact method"
    )]
    pub fn to_string(&self) -> String {
        self.text.to_string()
    }
}

/// Source load failure at the Holder/CST boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceLoadError {
    /// Source exceeds Rowan's representable byte range.
    #[error("source exceeds rowan TextSize: {actual} > {maximum}")]
    TooLarge {
        /// Actual byte length.
        actual: u64,
        /// Maximum representable byte length.
        maximum: u64,
    },
    /// Bytes do not match selected Holder basis.
    #[error("source hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        /// Expected Holder hash.
        expected: ContentHash,
        /// Observed byte hash.
        actual: ContentHash,
    },
    /// Holder bytes are not UTF-8.
    #[error("source is not UTF-8 at byte {valid_up_to}")]
    InvalidUtf8 {
        /// Prefix length accepted as UTF-8.
        valid_up_to: u64,
        /// Invalid sequence length when known.
        error_len: Option<u64>,
    },
}

/// UTF-8 source slice failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceSliceError {
    /// Range exceeds source length or reverses.
    #[error("byte slice {start}..{end} exceeds source length {len}")]
    OutOfBounds {
        /// Requested start byte.
        start: usize,
        /// Requested end byte.
        end: usize,
        /// Source byte length.
        len: usize,
    },
    /// Offset splits a UTF-8 code point.
    #[error("byte offset {offset} is not a UTF-8 boundary")]
    NotCharBoundary {
        /// Offending byte offset.
        offset: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::{SourceBasis, SourceLoadError, SourceSliceError, Utf8HolderView};
    use liminal_id::{ContentHash, SourceId};

    fn view(text: &str) -> Utf8HolderView {
        Utf8HolderView::from_bytes(
            SourceBasis {
                source: SourceId::from_name("unit"),
                content_hash: ContentHash::of(text.as_bytes()),
            },
            text.as_bytes(),
        )
        .expect("valid view")
    }

    #[test]
    fn rejects_hash_mismatch_and_invalid_utf8() {
        let basis = SourceBasis {
            source: SourceId::from_name("unit"),
            content_hash: ContentHash::of(b"expected"),
        };
        assert!(matches!(
            Utf8HolderView::from_bytes(basis, b"actual"),
            Err(SourceLoadError::HashMismatch { .. })
        ));
        let basis = SourceBasis {
            source: SourceId::from_name("unit"),
            content_hash: ContentHash::of(&[0xff]),
        };
        assert!(matches!(
            Utf8HolderView::from_bytes(basis, &[0xff]),
            Err(SourceLoadError::InvalidUtf8 { .. })
        ));
    }

    #[test]
    fn byte_slice_requires_utf8_boundaries() {
        let source = view("aéz");
        assert_eq!(source.byte_slice(1..3).expect("slice").to_string(), "é");
        assert!(matches!(
            source.byte_slice(2..3),
            Err(SourceSliceError::NotCharBoundary { offset: 2 })
        ));
    }
}
