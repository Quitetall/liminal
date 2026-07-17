//! Jurisdiction subject addressing (v4 §7.2, §7.5).

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ContentHash, NodeId, PathId, RelationId, SourceId};

/// What Jurisdiction ultimately governs — Nodes and Relations, nothing else
/// (v4 §7.2). A facet is schema shorthand, never a third primitive (Law 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JurisdictionSubject {
    /// A governed Node.
    Node(NodeId),
    /// A governed Relation.
    Relation(RelationId),
}

impl fmt::Display for JurisdictionSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(id) => id.fmt(f),
            Self::Relation(id) => id.fmt(f),
        }
    }
}

impl FromStr for JurisdictionSubject {
    type Err = ParseSubjectError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = s.parse::<NodeId>()
            && s.starts_with("node:")
        {
            return Ok(Self::Node(id));
        }
        if let Ok(id) = s.parse::<RelationId>()
            && s.starts_with("relation:")
        {
            return Ok(Self::Relation(id));
        }
        Err(ParseSubjectError {
            input: s.to_owned(),
        })
    }
}

/// Failed to parse a `node:<uuid>` / `relation:<uuid>` subject address.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid jurisdiction subject (expected node:<uuid> or relation:<uuid>): {input:?}")]
pub struct ParseSubjectError {
    /// The rejected input.
    pub input: String,
}

/// Key of a durable subject in a `WorkspaceBasis` component map (v4 §7.5).
///
/// SHAPE PROVISIONAL: the spec leaves `JurisdictionKey` abstract; Phase -1
/// revises it freely. Serialized as a canonical prefixed string
/// (`node:… | relation:… | path:… | object:… | source:…`) so it can be a JSON
/// map key and stay readable in NDJSON store records.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JurisdictionKey {
    /// A governed Node or Relation.
    Subject(JurisdictionSubject),
    /// A file Holder addressed by workspace-relative path.
    Path(PathId),
    /// A content-addressed immutable object.
    Object(ContentHash),
    /// An external source/service.
    Source(SourceId),
}

impl fmt::Display for JurisdictionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject(s) => s.fmt(f),
            Self::Path(p) => write!(f, "path:{p}"),
            Self::Object(h) => write!(f, "object:{h}"),
            Self::Source(s) => s.fmt(f),
        }
    }
}

impl FromStr for JurisdictionKey {
    type Err = ParseSubjectError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("path:") {
            return Ok(Self::Path(PathId(path.into())));
        }
        if let Some(hex) = s.strip_prefix("object:") {
            let hash = ContentHash::from_hex(hex).map_err(|_| ParseSubjectError {
                input: s.to_owned(),
            })?;
            return Ok(Self::Object(hash));
        }
        if s.starts_with("source:")
            && let Ok(id) = s.parse::<SourceId>()
        {
            return Ok(Self::Source(id));
        }
        s.parse::<JurisdictionSubject>()
            .map(Self::Subject)
            .map_err(|_| ParseSubjectError {
                input: s.to_owned(),
            })
    }
}

impl Serialize for JurisdictionKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for JurisdictionKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn subject_round_trips() {
        let n = JurisdictionSubject::Node(NodeId::new());
        assert_eq!(n.to_string().parse::<JurisdictionSubject>().unwrap(), n);
        let r = JurisdictionSubject::Relation(RelationId::new());
        assert_eq!(r.to_string().parse::<JurisdictionSubject>().unwrap(), r);
    }

    #[test]
    fn key_round_trips_including_paths_with_colons() {
        let cases = [
            JurisdictionKey::Subject(JurisdictionSubject::Node(NodeId::new())),
            JurisdictionKey::Path(PathId("notes/a:b.md".into())),
            JurisdictionKey::Object(ContentHash::of(b"obj")),
            JurisdictionKey::Source(SourceId::new()),
        ];
        for key in cases {
            assert_eq!(key.to_string().parse::<JurisdictionKey>().unwrap(), key);
        }
    }

    #[test]
    fn key_works_as_json_map_key() {
        let mut map = BTreeMap::new();
        map.insert(JurisdictionKey::Path(PathId("notes.md".into())), 1u32);
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"path:notes.md\""));
        let back: BTreeMap<JurisdictionKey, u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, map);
    }
}
