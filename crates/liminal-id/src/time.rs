//! Timestamps. Deliberately dependency-free for Phase -1; jiff/chrono may
//! arrive with real freshness policies (v4 §40).

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// The current wall-clock time.
    ///
    /// Conformance scenarios use a scripted fake clock instead; wall time never
    /// participates in deterministic replay (v4 §110).
    #[must_use]
    pub fn now() -> Self {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Self(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_positive_and_monotonic_enough() {
        let a = Timestamp::now();
        let b = Timestamp::now();
        assert!(a.0 > 0);
        assert!(b >= a);
    }
}
