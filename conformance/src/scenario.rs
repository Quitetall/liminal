//! Scenario fixture model — re-export shim (AM-2.1).
//!
//! The canonical model lives in `liminal_daemon::scenario`. This module
//! re-exports it and keeps the `authored_fixtures_all_parse` test in
//! conformance (which owns `fixtures/`).

pub use liminal_daemon::scenario::{
    Expectation, ScenarioHeader, ScenarioScript, Setup, SetupBuffer, SetupFile, SetupGraph, Step,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_fixtures_all_parse() {
        let dir = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/scenarios");
        let scripts = ScenarioScript::load_dir(&dir).expect("fixtures must parse");
        assert!(
            scripts.len() >= 6,
            "the six R4 §10 toy scenarios must exist, found {}",
            scripts.len()
        );
        for s in &scripts {
            assert!(!s.scenario.id.is_empty());
        }
    }
}
