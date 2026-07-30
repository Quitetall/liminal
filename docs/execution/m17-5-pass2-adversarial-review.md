# M17.5 — Adversarial review, PASS 2 (ADR-0020 §6)

**Reviewer:** codex (OpenAI family) via MCP — a DIFFERENT model family from
Pass 1 (Claude), as §6 requires. **Base:** `1474bc8`. **Isolation:** launched
before `m17-5-pass1-adversarial-review.md` existed, on a prompt naming only
the profile and the code. Reviewer attests it did not open the Pass-1 file.
Held-out corpus contents never read, listed, or hashed. Tracked files
unchanged.

**Result: 22 attempts — 4 refuted, 18 CONFIRMED DEFECTS.** Pass 1 found 2.

## Independent corroboration of Pass 1's A6

Three separate routes, found without sight of Pass 1:

- **#14** — mutants marked `killed` with no mutation report; the helper accepts
  statuses without executing anything.
- **#15** — ran `just haq-generated 1`, then `just haq-inventory`: the artifact
  says **1 accepted**, the packet says **100000**, and inventory **exits 0**.
- **#21** — no commit/tree/toolchain/lockfile/command/artifact binding exists
  in the packet schema at all.

That is A6 confirmed from a different family by a different method.

## Defects Pass 1 MISSED

- **#5, #6, #7 — three more §112 laws are vacuous.** `check_semantic_diff_matches`,
  `check_resolver_replay_deterministic`, and `check_replicas_converge`
  (`laws.rs:204-270`) all pass against no-op implementations. Pass 1 hardened
  only the three Phase 1 laws and never revisited the Phase 6/7 laws.
- **#8 — the `content_witness` oracle added for F-01 is weak.** Source `"a,b"`
  against output `"b a"` satisfies it: the token multiset survives while
  punctuation and ORDER do not. A normalizing parser passes every assertion.
- **#9, #10 — the new generators' metamorphic relations short-circuit.** If
  `three_way` always returns `Conflict` the identity branch never runs; the
  interchange byte-canonical check passes against a constant serializer.
- **#16 — the generated evidence digest omits outcomes.** Removing an
  `ensure!` without changing RNG consumption leaves the hash identical.
- **#18 — the generated artifact is nondeterministic**: `elapsed_ms` varies
  run to run (50→57, 94→110, 12→15), so the artifact cannot be hash-compared.
- **#19 — the crash lane built this session is unbound to the packet**:
  artifact says `pass`, packet says `registered`, inventory exits 0.
- **#11, #12, #13, #17, #20, #22** — test names checked only as strings
  (renaming to `does_not_exist` is unchecked); orphan non-critical requirements
  accepted; a fake mutant inventory with arbitrary family/operator labels
  accepted; the fuzz lane is purely declarative in the packet; review records
  forgeable with an empty `independently_reproduced`; canary metadata ignored
  because the runner dispatches by hardcoded ID.

## Refuted (suite correctly caught)

Content-deleting formatter, constant parser, input-ignoring compiler (the three
F-01 canaries all bite), and the locked-corpus flag bypass (`all 16 canaries
caught`).

## Verdict

"Not fit for foundational Phase 1 qualification. `just haq-verify` correctly
fails today because the packet is `not-run`, but the verifier has major
false-green paths."

Required changes, verbatim:

1. Bind packet to fixed commit/tree, exact commands, toolchain, lockfile, and
   raw artifact hashes; recompute generated/crash/review evidence.
2. Replace self-oracles and weak metamorphic checks with independent semantic
   expectations; enforce real mutant source/operator/family identity and actual
   kill reports.
3. Activate tests, implement real sanitizer fuzz/fault/review evidence,
   validate deterministic replay, and reject fabricated status-only rows.

## Disposition

§6: "Zero verified findings may remain unresolved." Twenty findings across both
passes were open at the time of this review. `qualification_state` stays
`not-run`; the packet stays `proposed`; Phase 1 remains unauthorized.

### Status after the M17.5 remediation campaign

Every numbered item from this pass is now closed. Each fix is paired with a
canary that was verified to bite, and each strengthening was verified NOT to
produce a false red against the real implementation — twice that check caught a
wrong assumption in the fix itself rather than a defect in the code.

| item | disposition |
| --- | --- |
| #5, #6, #7 | vacuous §112 laws hardened (`3eb5c56`) |
| #8 | content oracle compared word MULTISETS; order now checked as a subsequence |
| #9 | identity merge relation could short-circuit; `Disjoint` is now required |
| #10 | byte-canonical check passed a constant codec; decoded fields now compared |
| #11, #12, #13 | fabricated inventories rejected (`feda854`) |
| #16 | evidence digest was blind to the code under test; witnesses now absorbed |
| #17 | fuzz lane bound to real targets; two halves escalated as F-17 |
| #18 | `elapsed_ms` made the artifact unhashable; timing separated from evidence |
| #19 | crash lane wrote to gitignored `target/`; evidence now committed and bound |
| #20 | review rows were four self-asserted numbers; now bound to a record |
| #22 | canary prose was decorative; bound to the executed mutation |

Findings raised BY the remediation, recorded in
`m17-5-adversarial-findings.md`: F-11 through F-20. Of those, F-03, F-06, F-09,
F-17 and F-20 remain open by decision rather than by oversight.

Three of this pass's items were closed only after the fix's own verification
step rejected the first attempt:

- **#8** — the first ordering rule failed the real formatter, because the
  explicit surface hoists a durable id into its node header (`alpha {#a}` emits
  the id ahead of the text). Ids are position-mobile by declared transform;
  content is not. The rule now excludes ids and still catches a scrambler.
- **#9/#10** — verified against 20,000 accepted cases per family, confirming the
  now-unconditional relations are satisfied by the real implementations rather
  than merely un-short-circuited.
