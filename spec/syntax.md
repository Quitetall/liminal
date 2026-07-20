# Syntax — normative source-language constraints

**Status:** normative Phase 0 constraints. Exact production grammar remains a
reserved §119 decision. Phase -1 toy paragraphs are not the production language.

## Source goals

Human source MUST be compact, error-tolerant, incrementally parseable, pleasant
in existing editors, and fully expressible in an explicit form (v4 §§15–20).
Ordinary Markdown MUST be accepted by at least one frontend path. Repositories
declare edition and formatting policy. Invalid or unknown input MUST remain
losslessly representable at L0 rather than being rejected or silently repaired.

Three §119 alternatives remain open until explicit user acceptance:

- strict CommonMark-compatible superset;
- separate native `.lim` grammar;
- dual frontend with a shared explicit core.

All alternatives MUST lower to the same L1/L2 semantics and pass the same
projection laws. No parser work begins before the accepted grammar ADR.

## Explicit semantic core

Every frontend MUST express literal data, Node construction, Relation
construction, attribute assignment, ordered blocks, references, expressions,
and macro invocation (v4 §§16–17). Domain sugar lowers into these forms; it
cannot add a third semantic primitive. Compact and explicit forms at the same
Workspace Basis MUST resolve to equivalent canonical graph semantics.

Every implicit default MUST have an explicit spelling and explanation path.
`lim expand`, `lim explain`, and explicit formatter output expose lowered
meaning. Personal format choices may alter spelling, never interpretation.

## Identity spelling

**SYN-ID-01 — source identity law.** Compact `{#id}` and explicit
`entity="…"` spellings provide durable Holder-controlled evidence for Explicit
identity. Anonymous text remains Anchored. Duplicate, deleted, copied, or
foreign-rewritten identifiers MUST surface ambiguity or loss; parsers MUST NOT
invent continuity. Content hashes name immutable versions, not logical lineage.

Sidecars, graph-managed IDs, external IDs, inline IDs, and inferred anchors are
different strategies with different guarantees. A frontend MUST retain
provenance and declare its strategy. Relations declare minimum identity grade;
source syntax cannot upgrade a target past measured evidence.

## Formatting laws

Formatting MUST be idempotent. Parsing canonical formatting MUST recover the
same semantic graph for the declared supported subset. Formatting MUST preserve
unknown or malformed L0 content according to declared loss policy and MUST NOT
bless inferred identity. `fmt`, `check`, `fix`, `lint`, `expand`, `explain`,
`migrate`, `diff`, `graph`, `trace`, `doctor`, and `verify` share these laws.

## Measurement boundary

Real-CST evaluation in M17 uses only development/regression fixtures frozen
before measurement. It compares incremental recovery with an independent full
reparse oracle and preserves Level 2 unless every Level 3 conjunct passes. Test
failure changes evidence; it does not authorize changing this chapter or a
golden without review.
