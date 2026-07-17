# Liminal specification

This directory is the specification root for Liminal. It has three layers with
very different editing rules.

## Layout

```text
spec/
├── v4/                 CANONICAL — verbatim, never edited in place
│   ├── liminal_master_architecture_plan_v4.md   ("v4")
│   ├── liminal_architecture_revision_4.md       ("R4")
│   └── liminal_master_architecture_v3_to_v4.diff
├── kernel.md           topical index — semantic kernel, laws, Jurisdiction
├── syntax.md           topical index — surface language and identity syntax
├── ir.md               topical index — IR levels and incremental compiler
├── transforms.md       topical index — rewrite calculus and macro system
├── protocol.md         topical index — editor boundary, LDP, deployment
└── rfc/                numbered RFCs for spec-observable changes
```

## spec/v4/ — the canonical text

The two documents in `spec/v4/` — the master architecture plan ("v4") and
Architecture Revision 4 ("R4") — are the constitution of the project, together
with the v3→v4 diff that records how the plan reached its current form. They
are **verbatim and immutable**: nothing in this repository ever edits them.
When code, tests, or docs cite the spec, they cite these files by section, in
the forms `v4 §N` and `R4 §N`.

Changing what the canonical text *means* is done only through an accepted RFC
(see below), never by touching the files.

## Topical files — curated indexes, future chapters

`kernel.md`, `syntax.md`, `ir.md`, `transforms.md`, and `protocol.md` are the
five topical files that v4 §117 places in `spec/`. Today each one is a
**curated index**: a scope statement plus normative pointers into `spec/v4/`
by Part and section, with one-sentence summaries. They add navigation, not
meaning — on any conflict the canonical text wins.

At Phase 0 — whose deliverable is the ratified constitution (v4 Part XXII,
Phase 0) — these files grow into self-contained chapters, incorporating every
amendment the Phase -1 falsification laboratory forced. Until then, each ends
with the line "Status: index only — becomes a self-contained chapter at
Phase 0."

## spec/rfc/ — spec-observable changes

`spec/rfc/` holds numbered RFCs, one file per decision, for changes that are
**spec-observable**: anything a conformance fixture, corpus, or external
implementer could notice. Accepted RFCs amend the constitution and must list
the conformance fixtures they affect. See `spec/rfc/README.md` for the
process and `spec/rfc/0000-template.md` for the mandatory template.

Repository and implementation decisions that are *not* spec-observable —
toolchain pins, dependency choices, workspace layout mechanics, test-stack
selection — are recorded as ADRs in `docs/adr/`, not here. Rule of thumb: if
a conforming reimplementation from the spec alone would need to know it, it
is an RFC; if only this repository needs to know it, it is an ADR.
