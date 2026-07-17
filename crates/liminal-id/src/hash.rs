//! Content hashing (v4 §19.3 immutable versions, §47 content-addressed resources).

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// BLAKE3 content hash.
///
/// Backs the *content-addressed* identity grade (v4 §19.1) and the toy store's
/// record/object integrity checks (v4 §92). Serialized as lowercase hex so
/// intent logs and overlay payloads stay human-inspectable (Phase -1
/// falsification ethos: debuggability over speed).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// Hash raw bytes with BLAKE3.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Lowercase hex form (64 characters).
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            use fmt::Write as _;
            // Writing to a String cannot fail.
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    /// Parse a 64-character hex string.
    pub fn from_hex(s: &str) -> Result<Self, ParseHashError> {
        let s = s.trim();
        if s.len() != 64 {
            return Err(ParseHashError {
                input: s.to_owned(),
            });
        }
        let mut out = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
            let hi = hex_val(chunk[0]).ok_or_else(|| ParseHashError {
                input: s.to_owned(),
            })?;
            let lo = hex_val(chunk[1]).ok_or_else(|| ParseHashError {
                input: s.to_owned(),
            })?;
            out[i] = (hi << 4) | lo;
        }
        Ok(Self(out))
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.to_hex())
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl FromStr for ContentHash {
    type Err = ParseHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(D::Error::custom)
    }
}

/// Failed to parse a [`ContentHash`] from hex.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid content hash (expected 64 hex chars): {input:?}")]
pub struct ParseHashError {
    /// The rejected input.
    pub input: String,
}

/// Immutable identity of exact canonical content and dependencies (v4 §19, §19.3).
///
/// `VersionId = hash(kind, canonical payload, semantic attributes, dependency
/// versions)`. Distinct from [`crate::EntityId`], which carries *logical*
/// continuity across accepted revisions (Law 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VersionId(pub ContentHash);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_hex_round_trips() {
        let h = ContentHash::of(b"liminal");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(ContentHash::from_hex(&hex).unwrap(), h);
    }

    #[test]
    fn hash_is_deterministic_and_content_sensitive() {
        assert_eq!(ContentHash::of(b"a"), ContentHash::of(b"a"));
        assert_ne!(ContentHash::of(b"a"), ContentHash::of(b"b"));
    }

    #[test]
    fn hash_serde_is_hex_string() {
        let h = ContentHash::of(b"x");
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, format!("\"{}\"", h.to_hex()));
        let back: ContentHash = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn bad_hex_is_rejected() {
        assert!(ContentHash::from_hex("zz").is_err());
        assert!(ContentHash::from_hex(&"g".repeat(64)).is_err());
    }
}
