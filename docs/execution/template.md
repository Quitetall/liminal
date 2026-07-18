# Work-order template

Every work order in `docs/execution/` follows this skeleton exactly. Sections may not
be omitted; an empty section is written as "None." so its absence is never ambiguous.
Copy this file to author a new order (Phase 0+ orders are authored at the preceding
gate — see `phases.md`).

```markdown
# M<nn> — <Title> (<spec citations>)

**Status:** work order — <duration>. Authored at the M<nn-1> gate. The executor
implements mechanically; deviation = amendment or discovered gap, never silent.

## Scope

One paragraph: what this milestone builds and what it explicitly does not
(cite the docs/implementation-plan.md §4 exclusions it touches).

## Spec inputs

Exact sections to read before executing: v4 §…, R4 §…,
docs/implementation-plan.md §…, prior work orders consumed.

## Frozen surfaces

Types, test names, file formats, CLI subcommands, and golden files this order
may NOT change without an amendment. (Adding a `lim` subcommand or a
conformance `[[bin]]` is ALWAYS an amendment.)

## Pre-made decisions

Numbered D<nn>.1, D<nn>.2, … — every choice the executor would otherwise have
to make, each with a one-line justification. The executor does not decide;
the executor transcribes.

## Data schemas

Field-for-field structs / file formats / fixture layouts introduced, with
serde attributes and exact on-disk framing.

## Algorithms

Numbered, with constants named — and for each constant, WHERE it gets frozen
(usually a golden test).

## Exit gate

| Test | Location | Mode |
|---|---|---|
| <exact test fn name> | conformance/tests/… | flip \| born-passing |

## Predicted `just gates` delta

Phase -1 (or Phase N) ignored −X; passed +Y. A mismatch at the end of
execution is a discovered gap.

## Steps

- [ ] M<nn>.1 [T<k>] <imperative step> — command: `<exact command>` — expected: <exact observable output>
- [ ] M<nn>.2 [T<k>] …

(One command and one observable expectation per step; checkboxes are ticked
in that step's commit. `[T<k>]` is the executor tier — the level of
responsibility the step demands, defined in 00-protocol.md §7. The tag is a
floor of care, not a model assignment; the §7 blanket rules — golden
acceptance is T1, every escalation trigger promotes to T1 — override any
step tag.)

## Amendments

AM-<nn>.<k>: <frozen surface touched> — <what changed> — <why unavoidable>.
"None." means none; every AM is ratified or reverted at M12 / the phase gate.

## Discovered gaps

Anything this order under-specified, found during execution. Feeds the next
order's authoring; never patched silently.

## Exit criteria

Bullet list: all exit-gate tests green at the predicted delta; amendments
logged; goldens committed; (milestone-specific ceremonies, e.g. corpus lock).
```
