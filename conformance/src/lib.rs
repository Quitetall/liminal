//! The Liminal conformance harness (v4 §112–114; R4 §2, §10).
//!
//! Test-first law of this repository: every test the spec requires exists NOW
//! as a named, compiled test — passing where its API exists, `#[ignore]`d with
//! a phase-tagged reason where it does not. Ignored tests ARE the multi-year
//! backlog; a milestone is done when its tests flip to passing. `just gates`
//! renders the meter.
//!
//! The suite is deliberately separable from the implementation (v4 §114):
//! fixtures are plain TOML/NDJSON a second implementation could consume.

pub mod debt;
pub mod denominator;
pub mod harness;
pub mod identity;
pub mod laws;
pub mod memo;
pub mod pandoc;
pub mod replay;
pub mod scenario;
pub mod slo;
pub mod trace;
pub mod tracegen;

pub use debt::{DebtReport, scan_workspace_debt};
pub use denominator::DenominatorCounts;
pub use harness::{
    CrashState, HitTrace, RecoveryReport, ToyRun, assert_silent, runnable_crash_scenarios,
};
pub use scenario::{Expectation, ScenarioScript, SetupFile, SetupGraph, Step};
pub use slo::Scorecard;
pub use trace::{Trace, TraceEvent, TraceParseError};
