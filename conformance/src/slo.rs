//! The R4 §2.3 ergonomics SLO scorecard, thresholds hard-coded.
//!
//! These numbers are the GRADUATION FLOOR for a stable profile, measured over
//! held-out traces with frozen denominators (v4 §7.4; Phase -1.5 / M11 builds
//! the pipeline). The experienced target for a sound routine session is zero
//! ownership UI — 99% is a floor, not the product metric.

use serde::{Deserialize, Serialize};

/// Per-profile conformance metrics over one corpus run (R4 §2.3; -1.5 list).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    /// The profile measured.
    pub profile: String,
    /// The corpus version measured against (e.g. `heldout/v1`).
    pub corpus: String,
    /// Automatically resolved Jurisdiction-sensitive operations / all such
    /// operations. Falling back to an UNDECLARED Overlay does not count as
    /// resolved (R4 §2.3).
    pub auto_resolution_rate: f64,
    /// Ordinary sessions requiring no manual Jurisdiction interaction / all
    /// ordinary sessions.
    pub intervention_free_session_rate: f64,
    /// User-facing Jurisdiction diagnostics across all sound sessions.
    pub sound_session_diagnostics: u64,
    /// Bytes of checker output on entirely sound workspaces.
    pub sound_checker_output_bytes: u64,
    /// Unexpected reconciliation items created across sound ordinary sessions.
    pub unexpected_reconciliation_items: u64,
    /// Worst-case visible incidents for one root cause in one session.
    pub max_visible_incidents_per_root_cause: u64,
    /// Manual Contract authoring events in ordinary standard-profile sessions.
    pub manual_contract_authoring: u64,
}

impl Scorecard {
    /// Check every R4 §2.3 threshold. Empty vec = the profile graduates.
    #[must_use]
    pub fn violations(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.auto_resolution_rate < 0.99 {
            v.push(format!(
                "auto_resolution_rate {} < 0.99 (R4 §2.3)",
                self.auto_resolution_rate
            ));
        }
        if self.intervention_free_session_rate < 0.99 {
            v.push(format!(
                "intervention_free_session_rate {} < 0.99 (R4 §2.3)",
                self.intervention_free_session_rate
            ));
        }
        if self.sound_session_diagnostics != 0 {
            v.push(format!(
                "sound_session_diagnostics {} != 0 (R4 §2.3)",
                self.sound_session_diagnostics
            ));
        }
        if self.sound_checker_output_bytes != 0 {
            v.push(format!(
                "checker output on sound workspace: {} bytes != 0 (Law 3E)",
                self.sound_checker_output_bytes
            ));
        }
        if self.unexpected_reconciliation_items != 0 {
            v.push(format!(
                "unexpected_reconciliation_items {} != 0 (R4 §2.3)",
                self.unexpected_reconciliation_items
            ));
        }
        if self.max_visible_incidents_per_root_cause > 1 {
            v.push(format!(
                "visible incidents per root cause {} > 1 (R4 §2.3)",
                self.max_visible_incidents_per_root_cause
            ));
        }
        if self.manual_contract_authoring != 0 {
            v.push(format!(
                "manual_contract_authoring {} != 0 (R4 §2.3)",
                self.manual_contract_authoring
            ));
        }
        v
    }

    /// Whether the profile passes the graduation floor.
    #[must_use]
    pub fn graduates(&self) -> bool {
        self.violations().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perfect() -> Scorecard {
        Scorecard {
            profile: "external-file".into(),
            corpus: "heldout/v1".into(),
            auto_resolution_rate: 1.0,
            intervention_free_session_rate: 1.0,
            sound_session_diagnostics: 0,
            sound_checker_output_bytes: 0,
            unexpected_reconciliation_items: 0,
            max_visible_incidents_per_root_cause: 1,
            manual_contract_authoring: 0,
        }
    }

    #[test]
    fn perfect_scorecard_graduates() {
        assert!(perfect().graduates());
    }

    #[test]
    fn ninety_nine_percent_of_operations_does_not_excuse_a_noisy_session() {
        // The session SLO is load-bearing: 99% of 10,000 operations with 100
        // interruptions has failed ergonomically (R4 §2.3).
        let mut s = perfect();
        s.auto_resolution_rate = 0.999;
        s.intervention_free_session_rate = 0.90;
        assert!(!s.graduates());
    }

    #[test]
    fn hidden_overlay_debt_does_not_count_as_success() {
        let mut s = perfect();
        s.unexpected_reconciliation_items = 1;
        assert!(!s.graduates());
    }

    #[test]
    fn one_root_cause_may_surface_at_most_once() {
        let mut s = perfect();
        s.max_visible_incidents_per_root_cause = 2;
        assert!(!s.graduates());
    }

    #[test]
    fn sound_sessions_must_be_byte_silent() {
        let mut s = perfect();
        s.sound_checker_output_bytes = 1;
        assert!(!s.graduates());
    }
}
