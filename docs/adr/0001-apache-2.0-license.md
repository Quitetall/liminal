# 0001. License the project Apache-2.0, single license

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** v4 §131 (licensing and governance), `LICENSE`,
  `[workspace.package] license` in the root `Cargo.toml`

## Context

v4 §131 recommends "Rust-style permissive code licensing" and explicitly
states "the exact license remains a project decision." Liminal is intended
as long-lived open infrastructure; the license must be settled before the
first public commit because relicensing after external contributions
requires chasing every contributor.

The practical fork in permissive licensing is whether to carry an explicit
patent grant and whether to carry one license or two. Apache-2.0 includes an
express patent license and patent-retaliation clause; MIT is silent on
patents. The Rust ecosystem default is the `MIT OR Apache-2.0` dual license,
originally adopted for GPLv2 compatibility and downstream flexibility.

## Decision

We will license Liminal under **Apache-2.0 only**: a single verbatim
`LICENSE` file at the repo root, `license = "Apache-2.0"` inherited by every
workspace crate, and inbound contributions accepted under the same license
(inbound = outbound). This is an owner decision taken while the sole
copyright holder can still take it cheaply.

**What would falsify or reverse this:** a concrete, named integration or
distribution channel blocked by Apache-2.0's terms (e.g., a GPLv2-only
downstream that matters) — reversed by a superseding ADR while contributor
count still makes relicensing feasible, which is precisely why this is
decided at commit zero.

## Consequences

- **Positive:** explicit patent grant and retaliation clause protect users
  of an infrastructure project; one license file, one SPDX expression, no
  per-file dual-license boilerplate; unambiguous inbound terms.
- **Negative:** incompatible with GPLv2-only downstreams (GPLv3 is fine);
  mildly nonstandard in the Rust crates ecosystem, which may surprise
  contributors expecting the dual default.
- **Follow-ups:** `deny.toml` license allowlist (ADR-0006) must keep
  dependencies compatible with Apache-2.0 distribution; revisit only if a
  real blocker surfaces.

## Alternatives considered

- **MIT OR Apache-2.0 dual (Rust default)** — rejected by owner decision
  for single-license clarity: the dual form exists for GPLv2 compatibility
  Liminal does not need, and it lets downstreams opt out of the patent
  clause, which defeats the point of carrying it.
- **MIT only** — rejected: no patent grant; for a runtime others will build
  on, patent silence is a liability we can eliminate for free today.
- **Defer the decision** — rejected: every external contribution makes
  relicensing harder; §131 left this open precisely so it would be decided
  deliberately, not by default.
