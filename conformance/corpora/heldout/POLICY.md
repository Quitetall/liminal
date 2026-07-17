# Held-out corpus policy

**This document is binding.** It implements v4 §7.4 and R4 §2.2 for
everything under `corpora/heldout/`. It exists because held-out data
only judges an implementation while the implementation has never been
shaped by it.

## 1. Physical separation

Development fixtures (`../../fixtures/`) and held-out acceptance
corpora (this directory) are physically separate trees with opposite
rules. Profile authors may inspect and tune against fixtures freely.
Held-out corpora are **independently sourced** — real Git histories,
consented and anonymized editor sessions, foreign-tool rewrites,
offline/online transitions, adversarially generated mutations — and are
never authored or curated by the implementer of the profile they judge
(R4 §2.2: "a profile cannot graduate by passing only self-authored
fixtures").

## 2. Versioning and the manifest

Corpora are versioned as `v<N>/`. Each version contains a
`MANIFEST.b3` listing the BLAKE3 digest of every file in that
version. The CI test
`tests/classes/trace_replay.rs::heldout_manifest_locked` recomputes
every digest on every run; any drift — added, removed, or modified
bytes — fails CI.

## 3. The freeze rule

**Once any profile score computed against `v<N>` has been observed, `v<N>`
is frozen forever.**

- No file in a frozen version may be added, removed, edited,
  re-encoded, reformatted, or "corrected" — not even to fix a genuine
  defect in a trace.
- Any change — corrections, new trace categories, better anonymization —
  creates `v<N+1>` with its own manifest.
- Scores previously reported against `v<N>` are **retained** and remain
  part of the project record alongside `v<N+1>` scores (v4 §7.4: the
  implementer "may not alter the locked acceptance corpus after seeing
  profile results without creating a new version and retaining the old
  score").

## 4. Tuning against held-out data is a process violation

Reading held-out traces to debug a specific failure, selecting corpus
entries a profile happens to pass, or iterating a profile against
held-out scores are all the same violation: they silently convert
acceptance data into development data. The mechanical enforcement is
the manifest check plus the freeze rule; the human enforcement is that
any workflow which routes held-out bytes into profile development must
be treated as invalidating the affected scores. Debugging happens by
reproducing the failure class in `../../fixtures/` or by minimizing
into `../regression/` — never by editing here.

## 5. Provenance requirements

Every trace records its source, capture tool, and consent/anonymization
status (R4 §11.5: editor sessions require consent, minimization, and
anonymization, and the corpus design must not bias toward the project's
own authoring habits). A trace whose provenance cannot be stated does
not enter the corpus.

## 6. Denominators are frozen before tuning

The operation and session denominators (semantic-transaction
coalescing, the 30-minute session timeout, root-cause incident
coalescing — v4 §7.4) are fixed by `denominator_counts_golden` *before*
any profile tuning begins, so the metric cannot be gamed by
redefining what counts as an operation.
