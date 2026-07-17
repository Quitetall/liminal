# Golden reports

Byte-exact reports produced by the Phase -1 experiments. A golden file
is not a test artifact; it is a **published claim** about what the
architecture demonstrably guarantees. The specs require the honest
version of each claim — "no strategy may claim stronger continuity than
the corpus demonstrates" (v4 §-1.1), "record semantic loss … and the
exact projection capability level achieved" (v4 §-1.4) — and these files
are where those measurements become fixed text.

## Files (each lands with its milestone)

| File | Milestone | Content | Guarded by |
| --- | --- | --- | --- |
| `identity_matrix.md` | M7 | The identity guarantee matrix from the -1.1 torture corpus: strategy × operation → demonstrated outcome and `IdentityGrade`. | `matrix_matches_golden`, `claims_never_exceed_evidence` |
| `anchor_recovery.md` | M9 | Anchor recovery under foreign edits for the annotated-source spike (-1.2), and the projection level it honestly supports. | `anchor_recovery_report_golden`, `declared_level_matches_report` |
| `pandoc_loss.md` | M10 | Round-trip loss report for the Pandoc adapter boundary (-1.4): semantic loss, ID survival, foreign-node preservation, capability level. | `pandoc_roundtrip_loss_report_golden` |
| `denominator_counts.md` | M11 | The frozen v4 §7.4 operation/session denominators computed over the trace corpus — fixed **before** profile tuning so the 99% metrics cannot be gamed. | `denominator_counts_golden` |

## The byte-exact comparison rule

1. **Comparison is byte-for-byte.** No whitespace normalization, no
   line-ending tolerance, no "semantically equivalent" diffing. If the
   bytes differ, the test fails.
2. **Generators must be deterministic.** Stable ordering, pinned tool
   versions (`git`, `pandoc`), no timestamps, no absolute paths, no
   locale-dependent formatting. A report that cannot be regenerated
   bit-identically from its inputs is a bug in the generator.
3. **Regeneration is a reviewed diff.** Golden files change only
   through deliberate regeneration, and the diff is reviewed as a
   change to the project's claims — citing the spec section, milestone,
   or ADR that justifies the new claim. Weakening a claim (e.g. an
   identity grade demoted by new torture cases) is an expected,
   honest outcome; strengthening one requires evidence in the same
   commit.
4. **Claims may never exceed evidence.** Where a declared value exists
   elsewhere in the tree (a profile's declared `IdentityGrade`, an
   adapter's declared projection level), tests assert declared ≤
   demonstrated, with the golden file as the demonstrated bound.

Format: Markdown, rendered tables, LF line endings, UTF-8, trailing
newline — fixed so that "byte-exact" and "readable" stay compatible.
