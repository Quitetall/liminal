# Regression corpus

**Rule:** every falsified assumption lands here, minimized, and is
retained forever.

**Spec:** R4 §2.2 set 3 — each stable profile keeps a "regression
corpus: every previously discovered failure, minimized and retained".
This project applies the rule to the whole system, not only to
profiles: Phase -1 is a falsification laboratory, and this directory is
where its falsifications become permanent.

## What lands here

- Any input that ever made an architecture claim false: a scenario that
  reached a hidden half-state, a merge that was auto-applied unsafely,
  an identity strategy that over-claimed, a chimeric Basis, a lost
  byte.
- Crash-matrix cases that recovered wrongly, with the world state that
  exposed them.
- Fuzz and property-test counterexamples (proptest regressions may also
  persist in per-crate `proptest-regressions/`; cross-cutting ones are
  minimized into here).
- Field failures and bug-report reproductions, once minimized.

## Rules

1. **Minimize first.** An entry is the smallest input that still
   demonstrates the failure, plus a short provenance note: what
   assumption it falsified, which test now covers it, and the
   commit/ADR that fixed or reframed it.
2. **Append-only.** Entries are never deleted or "cleaned up", even
   when the subsystem that produced them is rewritten — a rewrite that
   cannot pass the old failures has not fixed them.
3. **Every entry is executable.** Each entry is wired to a named test
   (typically in `tests/`), so the corpus is replayed by CI rather than
   archived as folklore.
4. **No fixing the bytes.** Like all corpus data, entries are excluded
   from spell-checking and formatting; adversarial input must stay
   byte-exact.

## Naming

`<yyyy-mm-dd>_<area>_<short-slug>/` per entry, containing the minimized
input plus a `NOTE.md` with the provenance fields above. The first
entries are expected from M2's crash matrix onward.
