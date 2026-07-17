# Liminal RFCs

This directory holds the project's RFCs: numbered design documents, **one
file per decision**, for every **spec-observable** change — anything a
conformance fixture, corpus, golden file, or external implementer working
from the spec alone could notice. Accepted RFCs amend the constitution
(`spec/v4/` plus the topical chapters) with the same authority as the
canonical text they modify.

Decisions that are *not* spec-observable — toolchain pins, dependency
choices, CI mechanics, internal refactors — are ADRs in `docs/adr/`, not
RFCs. If only this repository needs to know it, it is an ADR.

## Process

1. Copy `0000-template.md` to `NNNN-short-kebab-title.md`, taking the next
   unused number. Numbers are permanent; they are never reused, even for
   rejected RFCs.
2. Fill in **every** template section. `Conformance impact` is mandatory
   and Liminal-specific: an RFC that cannot say which fixtures or corpora
   it touches (even if the answer is a justified "none change shape") is
   not ready for review.
3. Set `Status: draft` and iterate in review.
4. On acceptance, set `Status: accepted` with the decision date. The RFC
   then amends the constitution: the topical chapter(s) it touches must
   cite it, and the conformance fixtures it lists MUST be updated (or
   created) in the same change series that implements it.
5. Rejected RFCs keep their number with `Status: rejected` and a short
   rationale — a recorded "no" prevents relitigating.
6. An RFC replaced by a later one becomes `Status: superseded by NNNN`.

## Rules

- One decision per RFC. Bundled decisions get bundled reversals.
- Cite the canonical text precisely (`v4 §N`, `R4 §N`) for everything the
  RFC amends, and never edit `spec/v4/` itself — the canonical files are
  verbatim; RFCs are how their meaning changes.
- During Phase -1, RFCs are bound by Law 14 (v4 §3): a change that adds
  optimization, compiled Jurisdiction plans, CRDTs, parsers, or editors
  before the falsification gates pass (R4 §10) is out of order regardless
  of its merits.

## Index

| RFC | Title | Status |
|-----|-------|--------|
| [0000](0000-template.md) | Template | — |
| [0001](0001-add-liminal-jurisdiction-crate.md) | Add `crates/liminal-jurisdiction` to the v4 §117 layout | accepted (2026-07-17) |
