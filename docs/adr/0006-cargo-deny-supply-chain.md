# 0006. cargo-deny as the single supply-chain policy tool

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** `deny.toml`, ADR-0001 (Apache-2.0),
  `.github/workflows/ci.yml` + `scheduled.yml`; v4 §113 (held-out
  Git/foreign-tool trace replay — motivates the git2 ban)

## Context

An infrastructure project accumulates dependencies for years; license
compatibility (with Apache-2.0 distribution, ADR-0001), known
vulnerabilities, dependency sources, and banned crates all need machine
enforcement from commit zero, because retrofitting policy onto a grown tree
means relitigating every dependency.

Two crates deserve pre-emptive bans. `openssl`/`openssl-sys` drag in a
C toolchain and system-library divergence across the 3-OS CI matrix;
rustls covers any future TLS need. `git2` (libgit2) is subtler: the
Phase -1.7 identity torture corpus (v4 §113 "held-out Git … trace replay",
Part XXII -1.1) must reproduce what *users'* Git actually does — merge-ort
behavior, rename detection, real config handling — and libgit2 does not
faithfully reproduce merge-ort. The corpus therefore drives the real `git`
CLI in hermetic repos; gitoxide is the preferred in-process library later
if one is ever needed.

## Decision

We will use **cargo-deny alone** as the supply-chain policy tool — its
`advisories` check subsumes cargo-audit, so we do not run both. Policy in
`deny.toml`: a license **allowlist** for dependencies (Apache-2.0
+LLVM-exception, MIT, BSD-2/3, ISC, Zlib, Unicode-3.0, CC0-1.0; exceptions
only per-crate with written justification); **bans** on
`openssl`/`openssl-sys` (rustls-only policy) and `git2` (real git CLI for
the torture corpus; gitoxide preferred later — overriding this ban requires
an ADR); unknown registries and unknown git sources **denied**; any git
dependency requires its own ADR and a **pinned rev** — never a branch.
CI runs `cargo deny check bans licenses sources` on every push; the
scheduled workflow runs `advisories` weekly so new CVEs surface without a
code change.

**What would falsify or reverse this:** a load-bearing dependency genuinely
requiring a banned crate or disallowed license with no viable alternative —
handled as a scoped, justified exception in its own ADR, not by loosening
the default; or cargo-deny failing to model a policy we need (then we add a
tool, by ADR).

## Consequences

- **Positive:** license drift, yanked crates, and surprise git dependencies
  fail CI instead of accumulating; the audit trail for every exception is
  an ADR; one tool, one config, one CI step.
- **Negative:** advisory noise requires triage (every `ignore` entry must
  carry an inline reason); the allowlist will occasionally block a
  fine-in-practice license until reviewed.
- **Follow-ups:** SBOM/cargo-auditable activate at first distributed
  binary (see implementation plan defer list); revisit the allowlist at
  first publish.

## Alternatives considered

- **cargo-audit alongside cargo-deny** — rejected: cargo-deny's advisories
  check consumes the same RustSec database; a second tool is duplicate CI
  time and duplicate ignore-lists that drift apart.
- **License denylist instead of allowlist** — rejected: a denylist fails
  open on licenses nobody anticipated; infrastructure distribution needs
  fail-closed.
- **Allow git2** — rejected: the torture corpus exists to measure fidelity
  to real-world Git behavior; testing against libgit2's reimplementation
  (non-merge-ort) would let identity-grade claims exceed real evidence.
- **No git-dependency policy** — rejected: unpinned git deps are
  unreproducible builds, which Phase -1 evidence discipline (ADR-0002)
  cannot tolerate.
