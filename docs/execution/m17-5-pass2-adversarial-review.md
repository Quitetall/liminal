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
passes are now open. `qualification_state` stays `not-run`; the packet stays
`proposed`; Phase 1 remains unauthorized.
