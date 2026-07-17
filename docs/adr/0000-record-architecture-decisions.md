# 0000. Record architecture decisions

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** v4 §131 (licensing and governance), v4 Part XXIII (required
  ADRs), R4 §1 (complexity-budget admission)

## Context

v4 §131 recommends an RFC-and-ADR process as part of project governance, and
v4 Part XXIII enumerates decisions the spec deliberately leaves open
(§119–§131) that must be decided by evidence rather than by drift. This is a
multi-year solo project in a falsification phase: without written decision
records, the reasons behind reversals evaporate, and R4 §1's
complexity-budget discipline (every mechanism must name what it displaces)
has nowhere to be enforced for repo-level choices.

The spec itself is canonical and lives in `spec/v4/`, amended only through
`spec/rfc/`. That leaves a gap for decisions that are real and binding but
not spec-observable: toolchain, workspace layout, test infrastructure,
throwaway substrates. This ADR fills that gap.

## Decision

We will record every repository and implementation decision as a numbered,
immutable ADR in `docs/adr/`, using the MADR-lite template in `template.md`.
Spec-observable changes go through `spec/rfc/`, never through an ADR. ADRs
are immutable once accepted; reversals are new ADRs that mark the old one
superseded. Every ADR must state what would falsify or reverse it. Phase
gate outcomes (v4 Part XXII, R4 §10) land as ADRs referencing the
conformance tests and corpus evidence that decided them.

**What would falsify or reverse this:** the process producing paperwork
instead of decisions — e.g., ADRs written after the fact to rationalize code
already merged, or gate ADRs that cite no runnable conformance evidence. If
that happens the process is theater and gets replaced, by ADR.

## Consequences

- **Positive:** decisions carry their kill-conditions; reversals are cheap
  and honest; the Part XXIII open questions have a designated landing place;
  future contributors can reconstruct why, not just what.
- **Negative:** small per-decision writing overhead; the index needs manual
  maintenance.
- **Follow-ups:** ADRs 0001–0007 record the scaffold-time decisions; the
  first phase-gate ADR lands at the -1.0 exit gate (R4 §10).

## Alternatives considered

- **No records (commit messages only)** — rejected: commit messages capture
  the what, not the alternatives rejected or the reversal condition, and
  they are unsearchable as a decision log across years.
- **Everything through spec/rfc/** — rejected: RFCs govern spec-observable
  semantics and carry conformance-impact obligations; forcing toolchain
  pins through that process would dilute it.
- **A wiki or issue tracker** — rejected: decisions must live in the repo,
  versioned with the code they govern, reviewable in the same diff.
