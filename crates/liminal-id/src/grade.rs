//! Identity grades (v4 §19.1) — Law 12: identity strength must match Relation
//! durability.

use serde::{Deserialize, Serialize};

/// Declared identity grade of a Node exposed across revisions (v4 §19.1).
///
/// Relations declare the minimum grade they require: a transient syntax
/// highlight may accept [`Anchored`](Self::Anchored) identity, while a durable
/// cross-document comment must require [`Explicit`](Self::Explicit),
/// [`Managed`](Self::Managed), or [`External`](Self::External) identity, and a
/// reproducible dependency may require
/// [`ContentAddressed`](Self::ContentAddressed).
///
/// SHAPE PROVISIONAL: [`satisfies`](Self::satisfies) models the grades as a
/// total strength order (the spec's listing order). The Phase -1.1 identity
/// torture corpus decides whether the true relation is partial; no strategy may
/// claim stronger continuity than the corpus demonstrates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentityGrade {
    /// Valid only inside one parse or view.
    Ephemeral,
    /// Located within one exact source/resource basis.
    Anchored,
    /// Continuity across revisions was matched heuristically; carries
    /// confidence and provenance, and is labeled as heuristic (Law 12).
    Inferred,
    /// A durable identifier is serialized in Holder-controlled source.
    Explicit,
    /// A graph-native Holder preserves the logical identity.
    Managed,
    /// An outside Holder supplies the identifier.
    External,
    /// Identifies one immutable semantic or byte version.
    ContentAddressed,
}

impl IdentityGrade {
    /// Whether this grade satisfies a Relation's declared minimum (v4 §19.1).
    #[must_use]
    pub fn satisfies(self, required: IdentityGrade) -> bool {
        self.strength() >= required.strength()
    }

    fn strength(self) -> u8 {
        match self {
            Self::Ephemeral => 0,
            Self::Anchored => 1,
            Self::Inferred => 2,
            Self::Explicit => 3,
            Self::Managed => 4,
            Self::External => 5,
            Self::ContentAddressed => 6,
        }
    }
}

impl std::str::FromStr for IdentityGrade {
    type Err = ParseGradeError;

    /// Parse the kebab-case name used in fixtures and Contract literals
    /// (matches the serde `rename_all = "kebab-case"` form).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ephemeral" => Ok(Self::Ephemeral),
            "anchored" => Ok(Self::Anchored),
            "inferred" => Ok(Self::Inferred),
            "explicit" => Ok(Self::Explicit),
            "managed" => Ok(Self::Managed),
            "external" => Ok(Self::External),
            "content-addressed" => Ok(Self::ContentAddressed),
            _ => Err(ParseGradeError {
                input: s.to_owned(),
            }),
        }
    }
}

/// Failed to parse an [`IdentityGrade`] from its kebab-case name.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid identity grade: {input:?}")]
pub struct ParseGradeError {
    /// The rejected input.
    pub input: String,
}

#[cfg(test)]
mod tests {
    use super::IdentityGrade::*;

    #[test]
    fn grade_ladder_matches_spec_listing_order() {
        // v4 §19.1 examples: transient highlight accepts Anchored…
        assert!(Anchored.satisfies(Anchored));
        assert!(Explicit.satisfies(Anchored));
        // …durable comments require Explicit or stronger…
        assert!(!Inferred.satisfies(Explicit));
        assert!(Managed.satisfies(Explicit));
        assert!(External.satisfies(Explicit));
        // …reproducible dependencies may require content addressing.
        assert!(!Managed.satisfies(ContentAddressed));
        assert!(ContentAddressed.satisfies(ContentAddressed));
        // Heuristic continuity is never a substitute for declared identity (Law 12).
        assert!(!Inferred.satisfies(Managed));
        assert!(Ephemeral.satisfies(Ephemeral));
        assert!(!Ephemeral.satisfies(Anchored));
    }
}
