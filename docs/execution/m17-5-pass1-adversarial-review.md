# M17.5 — Adversarial review, PASS 1 (ADR-0020 §6)

**Reviewer:** T1 executor (Claude Opus 5). **Base:** `1474bc8`.
**Method:** every attempt carries a reproduction. Refuted attempts and my own
false positives are recorded, not deleted (§6: "False positives remain recorded
with reproduction evidence").

**Pass 2 independence:** launched BEFORE this document was written, on a clean
prompt naming only the profile and the code, so it cannot read these findings.

## Attempts

| # | Attack | Verdict |
|---|---|---|
| 1 | Degenerate formatter vs the three §112 laws | REFUTED — canaries bite |
| 2 | Held-out corpus leakage outside sanctioned paths | REFUTED |
| 3 | Packet generated-rows vs recorded run hashes | REFUTED — no drift |
| 4 | `tracegen` nondeterminism | **FALSE POSITIVE (mine)** |
| 5 | Forge packet statuses to `complete` | REFUTED — digest catches |
| 6 | Forge packet **and** digest with invented evidence | **CONFIRMED — CRITICAL** |
| 7 | Mutation testing on a second crate | **CONFIRMED — MAJOR** |
| 8 | Do the 16 gate canaries actually bite? | REFUTED — 16/16 caught |
| 9 | Crash-boundary exhaustiveness / dead spec | REFUTED — 8/8 exercised |
| 10 | Generator reproducibility from recorded seeds | REFUTED — byte-identical |
| 11 | Fuzz the §112 laws for 150 target-minutes | **CONFIRMED — see F-09** |
| 12 | Mutation testing on `liminal-cir` after fixes | REFUTED — 74/74 killed |

---

### A6 — CONFIRMED, CRITICAL. The entire qualification gate is self-attested.

**Reproduction.** Edit `conformance/haqp/packet.json`: set
`qualification_state: "complete"`, every mutant `killed`, every canary
`caught`, every generated family `pass` **with an invented seed
(`1234567890`) and a fabricated evidence hash (`"bbbb…"`, 64 chars)**, every
crash boundary `pass`, every review `pass`. Run `haq verify` once to learn the
expected digest, write that digest into
`docs/execution/phase1-suite-review.md`, and run again:

```
A6 forged packet WITH matching digest -> verify exit: 0
   (accepted silently)
```

**Impact.** The gate can be satisfied by editing two files. Nothing re-runs a
lane, and nothing cross-references the evidence the lanes actually produce.
Every lane in this campaign — generators, fuzz, canaries, crash — writes its
evidence to `target/haqp/*.json`, which is **gitignored and never read by the
verifier**. The `seed` and `evidence_hash` fields added for F-04 are
shape-checked (64 hex) but never checked against a run.

This means the strongest statement HAQP-1 can currently make is "someone typed
numbers that are internally consistent" — not "these numbers were measured."
A5 shows the digest binding does work; it just binds the packet to the
markdown, not to reality.

**Fix (not landed — the design decision is Brian's):** `verify_qualified_repo`
must, for each lane, read the recorded evidence artifact and require the
packet's counts, seeds and hashes to match it exactly; and those artifacts must
be committed (or their digests must be), not left in `target/`. Ideally the
qualified layer re-runs the cheap lanes (canaries, crash, generators) rather
than trusting any recorded file.

### A7 — CONFIRMED, MAJOR. The 100% mutation kill rate does not generalize.

**Reproduction:** `cargo mutants --file crates/liminal-format/src/lib.rs`

```
59 caught / 13 missed / 3 unviable   → 82% of viable
```

`liminal-cir` reaching 100% (F-08) was a local result. Survivors cluster in
exactly the wrong place:

- **four in `basis_semantic_eq`** (`crates/liminal-format/src/lib.rs:92,99,100`)
  — this function IS `Phase1Document`'s `PartialEq`, which means it is the
  comparison authority the §112 round-trip laws use in `assert_eq!(round, doc)`.
  A broken equality here weakens every law that depends on it. This is F-01's
  problem one level deeper: the laws were anchored with an independent content
  oracle, but the `==` they still use is itself untested.
- three in `component_address_eq` (`:110,127,128`);
- one in `emit_compact` (`:276`) — **my own AM-17.3 code**, whose `|| → &&`
  branch I never covered.

**Fix (not landed):** mutation-test every Phase 1 crate, not one; and treat any
crate that supplies an equality/comparison used by a law as critical surface.

### A4 — FALSE POSITIVE (mine), recorded per §6.

I claimed `tracegen` was nondeterministic because two runs differed. The only
differing fields were `trace_id` and `source`, which derive from the **output
filename** by design — I had written to `t1.ndjson` and `t2.ndjson`. Retested
against a single path: byte-identical. No defect. Recorded because a reviewer's
own errors are part of the evidence trail.

## Judgment

**Not fit to qualify a foundational phase yet.** The suite is far stronger than
it was — laws anchored to an independent oracle, `liminal-cir` at 100%
mutation kill, a real fuzz lane that found a genuine round-trip defect, a crash
lane with exhaustiveness reconciliation — but three things must change first:

1. **A6** — evidence must be verified, not attested. Until then every other
   number in the packet is a claim, not a measurement.
2. **A7** — extend mutation coverage to every Phase 1 crate, starting with the
   equality functions the laws compare through.
3. **F-09** — the canonical round-trip defect is still open, and §1 requires a
   full qualification-lane rerun after it is fixed.
