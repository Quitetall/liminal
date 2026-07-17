# Identity fixtures

**Populated by:** M7 (persistent-identity torture corpus, killer
experiment #4). Empty until then by design.

**Spec:** v4 Part XXII §-1.1 (identity torture corpus), v4 Laws 12
(identity strength must match Relation durability) and 13 (entity vs
version identity), v4 §114.

## What lives here

Small **hand-authored** identity cases — the readable seed set alongside
the generated torture corpus (which M7's harness produces in hermetic
temp repos with the real `git` CLI). Each case pairs:

- An identity strategy: inline IDs (`{#id}`), sidecar files, immutable
  content hashes, structural matching, revision anchors, managed graph
  IDs.
- One operation from the §-1.1 list: rename/move, split, merge,
  copy/paste, duplicated identical blocks, delete-and-recreate,
  formatter rewrite, arbitrary external edit, Git merge / rebase /
  cherry-pick / conflict resolution.
- The expected outcome: does identity survive, and at which demonstrated
  `IdentityGrade`?

The aggregated result is the identity guarantee matrix rendered to
`../../golden/identity_matrix.md`. The governing law is Law 12 made
executable: no strategy may claim stronger continuity than these cases
demonstrate — a declared `IdentityGrade` must be bounded by the worst
demonstrated outcome.

## Consuming tests

M7's `matrix_matches_golden` and `claims_never_exceed_evidence`
(spec: "no strategy may claim stronger continuity than the corpus
demonstrates", v4 §-1.1).
