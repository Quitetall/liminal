# 0002. Pin exact stable toolchain; edition 2024; MSRV is not a promise pre-publish

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** `rust-toolchain.toml`, root `Cargo.toml`
  (`edition`/`rust-version`), `justfile` (`bump-toolchain`),
  `.github/workflows/scheduled.yml` (beta canary); v4 Law 14 / Part XXII
  (Phase -1 reproducibility)

## Context

Phase -1 is a falsification laboratory: crash-matrix results, torture-corpus
matrices, and held-out scorecards are only evidence if they are
reproducible, and a floating toolchain silently changes codegen, lints, and
std behavior underneath them. At the same time, every workspace crate is
`publish = false` — nobody depends on us — so a Minimum Supported Rust
Version compatibility promise would be a cost paid to no consumer.

Edition 2024 is the current edition and there is no legacy code to migrate;
starting on it avoids a future mechanical migration commit polluting the
history of a young codebase.

## Decision

We will pin the **exact current stable** (1.97.1 at time of writing) in
`rust-toolchain.toml`, use **edition 2024** workspace-wide, and set
`rust-version` in `[workspace.package]` to track the pin. MSRV is
explicitly **not a promise** while `publish = false`: `rust-version` is
documentation of the pin, nothing more. The pin is bumped via
`just bump-toolchain <version>` within a week of each stable release, as its
own commit so any behavior change bisects to it. A scheduled CI job runs a
beta-toolchain canary (`continue-on-error`) so breakage lands as a heads-up,
not a surprise.

**What would falsify or reverse this:** a first `cargo publish` (activates a
real MSRV policy, by superseding ADR); or the weekly-bump cadence repeatedly
blocking work because a stable release breaks us — that would argue for
pinning looser or investing in the canary earlier.

## Consequences

- **Positive:** every CI run, crash matrix, and benchmark baseline is tied
  to one exact compiler; `rustup` auto-installs the right toolchain from
  the committed file (single source of truth — CI does not name a version);
  new stable features usable within a week.
- **Negative:** weekly-ish bump commits; contributors on older system
  toolchains get a forced download; no compatibility promise for anyone
  vendoring the code early.
- **Follow-ups:** define the real MSRV policy in the ADR that accompanies
  the first publish; keep the beta canary in `scheduled.yml` honest
  (investigate failures, do not mute them).

## Alternatives considered

- **Floating `stable` channel** — rejected: non-reproducible; a toolchain
  update and a code change could land in the same run, making crash-test
  regressions unattributable.
- **N-2 MSRV policy (support two releases back)** — rejected: meaningless
  with `publish = false`; it would forbid current-stable features in
  exchange for a promise with zero beneficiaries.
- **Pin nightly** — rejected: no nightly feature is load-bearing for
  Phase -1, and nightly churn is the opposite of the reproducibility the
  falsification lab needs.
