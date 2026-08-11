//! Milestone-specific tests, one module per milestone, exercising the concrete
//! APIs each milestone delivers. The Phase -1 gate tests live in
//! `phase_minus_1.rs`; these are the finer-grained checks a work order's exit
//! gate names.
//!
//! **m03–m11 are born-passing. M18–M23 now exercise the Phase 1 source-to-HTML
//! surfaces; M24 remains an aggregate command gate.** Phase 1 tests are active
//! only after their implementations exist; future Phase 2+ tests stay ignored.

mod m03;
mod m04;
mod m05;
mod m06;
mod m07;
mod m08;
mod m09;
mod m10;
mod m11;
mod m18;
mod m19;
mod m20;
mod m21;
mod m22;
mod m23;
