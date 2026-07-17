# 0005. Test stack: cargo-nextest, insta, proptest, divan

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** v4 §113 (test classes), §92 (daemon-kill tests in CI), §116
  (performance benchmarks), R4 §7.5 (Recover); `.config/nextest.toml`,
  `conformance/`, `benches/`

## Context

v4 §113 enumerates the test classes this project must eventually run, and
two of them constrain the runner itself: crash-recovery tests terminate a
real subprocess (SIGABRT) after every ILRP durable boundary (§92, R4 §7.5),
and property-based tests must accumulate a regression corpus. The default
libtest runner executes tests as threads in one process — a crashing test
can take unrelated tests down with it, and global state (cwd, env,
lockfiles) leaks between tests.

Benchmarks (§116) need a harness now only to record baselines; Law 14
forbids optimization and CI gating until Phase 1, so the bench framework is
chosen for low ceremony, not for statistical depth.

## Decision

We will standardize on: **cargo-nextest** as the mandatory test runner —
process-per-test isolation is a **correctness requirement** for the
crash-injection tests, not a convenience; a serialized `crash` test-group
(`max-threads = 1`) because crash tests share the on-disk toy store; and
**`retries = 0` everywhere, permanently — a flaky crash-recovery test is a
finding about the recovery protocol, never noise to be retried away**.
**insta** for snapshot tests (golden checker output, matrices, reports).
**proptest** for property tests, with `proptest-regressions/` **committed**
— it is the §113 regression corpus in miniature: every discovered failure,
minimized and retained (mirroring R4 §2.2's regression-corpus rule).
**divan** for §116 benchmarks.

**What would falsify or reverse this:** a §113 class nextest cannot express
(e.g., a multi-process handshake its lifecycle cannot host when real-daemon
SIGKILL tests land in Phase 2); or Phase 1 CI regression gating needing
statistical rigor divan cannot provide — either produces a superseding ADR
for that component only.

## Consequences

- **Positive:** a SIGABRT in one test cannot poison another; the crash
  matrix runs under the same runner as everything else; snapshot review via
  `just snap`; the regression corpus grows automatically and travels with
  the repo; divan's `AllocProfiler` covers the §116 memory-per-Node/Relation
  benchmark without a custom allocator harness.
- **Negative:** `cargo test` alone no longer runs the suite correctly
  (mitigated: `just test` and CI both invoke nextest; doctests still run
  via `cargo test --doc`); four dev-dependencies to track; divan is younger
  than criterion.
- **Follow-ups:** CodSpeed integration via `codspeed-divan-compat` when
  Phase 1 activates bench gating; extend the `crash` group filter if crash
  tests appear outside the conformance crate.

## Alternatives considered

- **Default libtest runner** — rejected: thread-based execution cannot
  isolate deliberately-aborting tests; correctness, not preference.
- **criterion (over divan)** — rejected for now: heavier API and slower
  runs for a phase where benchmarks are recorded baselines, never gates
  (Law 14); divan's allocation profiling covers §116's memory benchmark;
  CodSpeed compatibility keeps the Phase 1 path open.
- **quickcheck (over proptest)** — rejected: no persisted failure corpus
  or integrated shrinking config; the committed regression corpus is the
  point, not an accident.
- **Custom crash-test harness outside the test runner** — rejected: a
  second runner splits reporting and the spec-debt meter; nextest groups
  make the crash matrix a first-class citizen of the one suite.
