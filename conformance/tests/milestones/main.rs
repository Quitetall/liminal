//! Milestone-specific tests, one module per milestone, exercising the concrete
//! APIs each milestone delivers. The Phase -1 gate tests live in
//! `phase_minus_1.rs`; these are the finer-grained checks a work order's exit
//! gate names.
//!
//! **m03–m11 are born-passing. m18–m23 are `#[ignore]`d under AM-17.2**, because
//! M18–M24 are authored but unauthorized and F-02 makes pre-greening a Phase 1
//! exit gate the specific failure this phase exists to prevent. They are real
//! tests against real surfaces — every one passes under `--ignored` — so
//! un-ignoring one is an authorization decision, not a rewrite.
//!
//! There is no `m24`, and seven of the packet's 27 declared Phase 1 milestone
//! tests are deliberately unwritten: their subjects do not exist yet (`lim fmt`,
//! a benchmark baseline, the aggregate gate). See `m20.rs` and `m23.rs` for the
//! reasoning — in short, a test whose subject is missing can only be a stub
//! that fails unconditionally, and under F-28's gate such a stub would
//! "witness" the kill of any mutant naming it. An honest absence beats a
//! dishonest witness.

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
