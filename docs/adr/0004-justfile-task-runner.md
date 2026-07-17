# 0004. Use just as the task runner; defer cargo-xtask until a task needs real logic

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** `justfile`, `.github/workflows/ci.yml`; R4 §2.2 (locked
  held-out corpus — the recorded xtask activation trigger); v4 Law 14

## Context

The repo needs one memorable entry point for the dev loop (fmt, lint, test,
crash matrix, spec-debt meter, docs, deny, bench) and a guarantee that what
a contributor runs locally is what CI runs. The candidates are a `justfile`
(declarative recipe runner, near-zero ceremony) and the cargo-xtask pattern
(a workspace `xtask` crate — arbitrary Rust, but every task is a program to
maintain and compile).

Every current task is a short sequence of shell commands. Under Law 14,
building an automation crate before any task needs real logic is
optimization of tooling ahead of falsification of architecture.

## Decision

We will use **`just`** as the sole task runner now (`shell := ["bash",
"-euo", "pipefail", "-c"]`), with the invariant that **`just ci` runs
exactly what CI runs, in CI order** — drift between the two is a bug. An
`xtask` crate is added the moment a task needs real logic rather than
command sequencing; the recorded **activation trigger** is the conformance
corpus import/lock tooling at Phase -1.5 (R4 §2.2 manifest hashing,
trace-format validation, dev/held-out splitting), which is genuinely
programmatic. `just` recipes then delegate to xtask — the entry points do
not change.

**What would falsify or reverse this:** a recipe accumulating nontrivial
logic in bash (parsing, hashing, state) before -1.5 — that is the trigger
firing early, and the logic moves to xtask immediately rather than being
debugged as shell.

## Consequences

- **Positive:** zero-compile task running; recipes are self-documenting
  (`just` lists them); the local/CI equivalence invariant is checkable by
  reading two files side by side.
- **Negative:** one non-cargo tool to install (mitigated by `just setup`
  and taiki-e/install-action in CI); bash recipes are less portable than
  Rust — anything platform-sensitive belongs in xtask when it exists.
- **Follow-ups:** create the `xtask` crate at Phase -1.5 for corpus
  tooling; keep `ci.yml` and `just ci` reconciled in every PR that touches
  either.

## Alternatives considered

- **cargo-xtask from day one** — rejected: every current task is command
  sequencing; an automation crate now is maintenance surface with no logic
  to hold, contrary to Law 14's ordering.
- **Bare cargo aliases / shell scripts in `scripts/`** — rejected: aliases
  cannot express multi-step recipes with dependencies; loose scripts have
  no discoverable index and drift from CI silently.
- **make** — rejected: we need a command runner, not a build-dependency
  graph; make's file-target semantics and portability quirks (tabs,
  platform make variants) buy nothing here.
