# Syntax — human source language and built-in Ruff

**Scope.** The native surface language: its goals, the compact
Markdown-compatible form and the fully explicit form, the minimal syntax
kernel every piece of domain sugar compiles into, the defaults policy, the
way persistent identity is spelled in source, and the formatter/linter
command surface with its two laws. Corresponds to v4 Part V (§15–§20).

This file is a curated index into the canonical text in `spec/v4/`. On any
conflict the canonical text wins. Citation forms: `v4 §N`, `R4 §N`.

## Part V — Human source language

- **Surface syntax goals** — Markdown-compatible for ordinary prose,
  compact, unambiguous under a declared edition, error-tolerant,
  incrementally parseable, pleasant in modal editors, and always fully
  expressible in an explicit form; ordinary Markdown is accepted as a
  frontend dialect. v4 §15.
- **Compact and explicit forms** — the same document written as compact
  Markdown-style sugar and as explicit `node …` construction; both lower to
  the same resolved graph normal form at the same Workspace Basis. v4 §16.
- **Syntax kernel** — the grammar needs only a few general forms (literal,
  node construction, relation construction, attribute assignment, ordered
  block, reference, expression, macro invocation); all domain sugar compiles
  into those forms. v4 §17.
- **Defaults** — every implicit default has an explicit representation and
  an explanation path (`lim expand`, `lim explain`, `lim fmt --profile
  explicit`); the repository declares its formatter's default-writing
  policy. v4 §18.
- **Identity in source** — entity/version/anchor/alias separation, identity
  grades, the no-magical-round-trip rule, and the compact (`{#id}`) and
  fully explicit (`entity="…"`) identity spellings. v4 §19 (grades §19.1,
  round-trip limits §19.2, immutable versions and lineage §19.3). The full
  identity model is indexed in `spec/kernel.md`; this file owns only how
  identity is *written*.
- **Formatter and linter commands** — the Ruff-like `lim` toolchain
  (`fmt`, `check`, `fix`, `lint`, `expand`, `explain`, `migrate`, `diff`,
  `graph`, `trace`, `doctor`, `verify`) and the two formatter laws:
  formatting is idempotent, and parsing canonical formatting recovers the
  same semantic graph. v4 §20.

Phase -1 note: no production grammar or parser exists yet — Law 14 (v4 §3)
defers them until the Phase -1 gates pass (R4 §10). The grammar decision
itself is an open ADR (v4 §119). The toy paragraph format used by the
falsification laboratory is deliberately not this language.

Status: index only — becomes a self-contained chapter at Phase 0.
