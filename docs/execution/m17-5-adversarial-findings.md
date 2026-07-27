# M17.5 — adversarial pass findings (round 1, sequential)

**Reviewer:** T1 executor (Claude Opus 5), sequential pass at Brian's direction.
**Base:** `3de2f40`. **Method:** mutation/refutation — a finding must carry a
reproduction, not an opinion. Findings verified before recording; one candidate
(crash-boundary drift) was checked and **dropped** as clean.

**Verdict: the packet is NOT eligible for HAQP-1, and the gate correctly says
so.** `qualification_state: not-run`, every mutant `predeclared`, every review
`planned`, and `phase1_suite_packet_is_complete_and_unratified` is `#[ignore]`d.
The machine gate fails closed exactly as designed — no false claim of
qualification exists. The findings below are about what must be true *before*
that campaign is worth running.

---

## F-01 — CRITICAL. **RESOLVED (this session).** All three flipped Phase 1 law gates were vacuous.

**Reproduction (verified, this session):** implement `liminal_format::Formatter`
such that `parse` returns a unit `Doc` for every input and `format`/`emit`
return `String::new()` — a formatter that **deletes all content**. Run
`laws::check_formatter_idempotence` and `laws::check_canonical_round_trip`
against it. **Both pass.**

Cause: every assertion compares the implementation to itself.

- `check_formatter_idempotence` (`conformance/src/laws.rs:20`) asserts
  `parse(format(x)) == parse(x)` and `format(format(x)) == format(x)`. A
  constant `parse` satisfies the first; an idempotent-by-triviality `format`
  satisfies the second.
- `check_canonical_round_trip` (`:46`) asserts `parse(emit(doc)) == doc` and
  `emit(round) == emitted`. Same defect.
- `check_incremental_equals_full` (`:67`) asserts
  `incremental(x, edits) == full(apply(x, edits))`. A compiler returning a
  constant from both passes trivially.

This is the exact prohibition in ADR-0020 §4: *"A relation is not proved by
calling the implementation's own equality or normalization path on both sides."*

**Impact:** `formatter_idempotence_law_holds`, `canonical_round_trip_law_holds`,
and `incremental_equals_full_compile_law_holds` are green and counted as active
progress, but constrain nothing. They are also the exit gates of M19/M20/M21.

**Fix — LANDED.** `conformance/src/laws.rs` gained `content_witness`, an
independent oracle implemented inside the law over RAW text (word multiset +
durable-id values), never calling the implementation's parser, canonicalizer,
or equality. All three laws now additionally assert:

- **content survival** — a non-empty input may not become empty, and every
  word token and durable-id value must survive into the output;
- **parse discrimination** — a parser mapping every declared source to one
  `Doc` is rejected;
- **edit sensitivity** — a compiler whose output ignores observable edits is
  rejected.

Id survival is checked surface-agnostically: `{#a}` legitimately becomes
`(id = "a")` in the explicit syntax, so the oracle requires the id VALUE to
survive, not its spelling. (Discovered while hardening: the real
`MarkdownFormatter::format` emits the explicit surface, not Markdown.)

Four permanent gate canaries in `conformance/tests/law_canaries.rs` pin this:
content-deletion vs both round-trip laws, constant-parser vs idempotence, and
input-ignoring compiler vs the incremental law. Each is `#[should_panic]`, so
a law going vacuous again turns a canary red. The real implementation still
passes all three laws.

## F-02 — CRITICAL. **RESOLVED (this session, AM-17.2).** Phase 1 was implemented while formally unauthorized, and its exit gates were pre-greened.

`docs/execution/M18.md`–`M24.md` show **0 of 47 steps done**; the packet records
`Phase 1 execution authorization: NOT_RUN`. Yet `crates/liminal-cir` (1,141
lines), `liminal-hir` (995), `liminal-format` (550), `liminal-cst` (403) contain
complete implementations with zero `todo!()`, landed by `2ab679e feat: build
phase1 qualification surfaces` and `1bc76c9 feat: implement M19 HIR CIR and
Phase1 formatter`.

The motive is understandable and partly legitimate — mutation testing cannot run
against code that does not exist. But three consequences are not acceptable as-is:

1. **M19's exit gate is green before M19 runs.** `canonical_round_trip_law_holds`
   can no longer measure whether M19 was performed correctly.
2. **The audit trail contradicts the tree**: checkboxes say not-started, code
   says done. M12's ledger discipline exists to prevent exactly this.
3. Phase 1's gates flipped without the Phase 1 authorization the whole M17 gate
   is built to withhold.

**Fix — LANDED as AM-17.2 (option (a)).** The crates are declared **HAQP
qualification substrate**: retained, not reverted, because HAQP-1 cannot mutate
or fuzz code that does not exist. But all **seven** Phase 1 exit-gate tests —
not just the three law gates; `malformed_source_never_panics_and_round_trips`,
`fuzz_regressions_stay_fixed`, `full_document_html_matches_golden`, and
`incremental_patch_equals_full_render` were green too — are returned to their
original `#[ignore = "Phase 1: ..."]` tags, recovered verbatim from `d76efd5`.
Each is un-ignored by the milestone that earns it (M18-M23), with M24
aggregating all seven.

Independent confirmation the quarantine is correct: the meter now reads
**Phase 1: 7 deferred / backlog 23**, and 23 minus the two Phase 0 gates that
flip at M17.5/M17.6 is **exactly the 21 M17.8 predicts**. The work order's own
arithmetic never expected these tests to be green at M17.

## F-03 — CRITICAL. Oracle independence is structurally compromised.

ADR-0020 §5 requires that incremental and full oracles share "raw bytes and
published data schemas only," and that expected values come from literals, spec
examples, or a separately implemented oracle. Two violations:

- **Mechanical** (see F-01): the laws use the implementation's own `PartialEq`
  over its own `Doc` type as the comparison authority.
- **Organizational**: the suite and the implementation it judges were authored
  by the same agent in the same session sequence. ADR-0020 §6 demands two
  reviewers with distinct identities, isolated state, and different model
  families; nothing analogous constrains *authorship*.

**Fix:** require that the Phase 1 oracle be implemented by a different executor
than the Phase 1 implementation, and record both identities in the packet.

## F-04 — MAJOR. **RESOLVED with F-07.** Generated evidence carried result-shaped numbers before any run.

All five families record `accepted: 100000, attempts: 100000, discards: 0` and
`fuzz_minutes: 31`. Five independent generators producing a **0% discard rate**
is not plausible, and each figure sits exactly at its ADR threshold (100,000;
30 minutes). They are marked `result: planned`, which is what saves them from
being fraud — but predeclaring targets in the same fields that will later hold
measurements invites silent drift into "already satisfied."

**Fix:** null/`NOT_RUN` these numeric fields until measured; keep thresholds in
the ADR only. Add a packet check rejecting a `pass` result whose `attempts`
equals `accepted` exactly.

## F-05 — MAJOR. **RESOLVED (this session).** Traceability was far short of the ADR's own bar.

36 requirements map to **8 tests**, and all 65 mutants name only **8 distinct
killing-test tuples**. ADR-0020 §2 requires positive, negative,
malformed/adversarial, basis/provenance, and deterministic-replay evidence for
every applicable law — roughly 5 × 36 ≈ 180 evidence points. Eight tests cannot
supply that, and a 100% kill rate carried by eight coarse tests is precisely the
"clustering around easy-to-kill paths" §3 prohibits.

**Fix — LANDED.** Root cause found in the verifier itself: `verify_packet_shape`
hard-coded `require_exact_ids(… (1..=8) …)`, structurally freezing the inventory
at eight tests. That ceiling is replaced with a dense-sequential check over the
actual count.

- `Test` gains an `evidence` field (`positive` | `negative` | `malformed` |
  `basis` | `replay` | `fault` | `recovery`).
- The inventory grows **8 → 38**: the eight exit gates are `positive`, and each
  Phase 1 milestone gains negative/malformed/basis/replay tests named for what
  they prove (M20 also gains injected-fault and idempotent-recovery for its
  fault-class requirement P1-R015). All 36 critical requirements now carry the
  five core kinds; P1-R015 carries all seven.
- `verify_evidence_coverage` enforces it, and `verify_kill_concentration`
  enforces ADR-0020 §3's anti-clustering rule (no test may be named killer for
  >25% of mutants; every named killer must exist).
- Mutant killers were redistributed from the coarse gates onto the targeted
  evidence tests: **8 → 35 distinct killers, worst share 32% → 14%**.

Two canaries prove the new checks bite (`shape_check_rejects_a_requirement_
missing_an_evidence_kind`, `shape_check_rejects_mutation_coverage_clustered_on_
one_test`) — the same discipline F-01 demanded of the laws.

## F-06 — MODERATE. Mutant selection is a uniform grid, not risk-weighted.

65 = 5 families × 13, and every operator appears exactly 5 times. This satisfies
the letter of §3 (no operator exceeds 25%) while defeating its intent: mutants
should concentrate where defects are subtle and consequential, not tile a
matrix evenly.

**Fix:** weight selection toward stateful/critical surfaces (ILRP transitions,
Basis staleness, dispatch completeness) and record the rationale per family.

---

## F-07 — CRITICAL. **RESOLVED (this session).** The generated-evidence lane tested nothing for four of five families, and its accept/discard counts were hardcoded.

Found by running the lane rather than reading the packet (`just haq-generated`,
then `haq generate --cases 100000`). Two independent proofs:

**1. The counts are literals, not measurements.** In
`crates/liminal-xtask/src/haq.rs::run_generated_repo`, every family records
`accepted: cases, attempts: cases, discards: 0` — the `cases` parameter echoed
back. No case is ever evaluated for acceptance, so ADR-0020 §4's ">=100,000
accepted" and "discard rate above one percent fails qualification" are satisfied
**by construction**. This is F-04's suspicion confirmed and worse: the 0% discard
rate is not implausible, it is structurally impossible.

**2. Four families do no work on the system under test.** By loop index:

| # | Family | What the loop actually does | 100k cases |
|---|---|---|---|
| 0 | source/CST/formatting | REAL: parse, assert lossless emit, assert format idempotence | 1078 ms |
| 1 | graph/interchange codecs | round-trips `{"seed":N}` through `serde_json` — tests a third-party library, not Liminal's codecs | 11 ms |
| 2 | transforms/projections | `three_way(base, base, base)` — identical inputs, the trivial no-op case; never a real transform | 104 ms |
| 3 | repair/ILRP/recovery | picks a `CrashPoint` and hashes its **name**; no driver, no repair, no recovery, no injection | 3 ms |
| 4 | Basis/revision/query invalidation | hashes the PRNG state; zero contact with Basis, revision, or invalidation | negligible |

Timing corroborates independently: 100,000 cases in 3 ms is ~33 million
cases/second — not achievable if a case exercises anything.

ADR-0020 §4 also requires per-family metamorphic relations (canonical reparse,
idempotent replay, inverse/undo, irrelevant-input invariance, commuting
independent transactions, incremental/full equivalence, deterministic
permutation). Only family 0 has any.

**Impact:** this is the finding that most endangers the whole profile. A
qualification run would have recorded "5 families x 100,000 accepted
deterministic cases, 0 discards" — 500,000 cases of evidence — while actually
exercising one surface. Every downstream conclusion resting on generated
evidence would have been false.

**Fix — LANDED.** All five families now generate structured inputs from a
recorded seed, decide acceptance from the candidate, and check metamorphic
relations that do not route through the implementation's own equality:

| Family | Metamorphic relation(s) | before → after |
|---|---|---|
| source/CST/formatting | lossless emit + format idempotence, over bytes | 1078 → 913 ms |
| graph/interchange codecs | **byte-canonical stability** (re-encode of a decode reproduces the bytes; a broken `Eq` cannot pass it) | 11 → 43 ms |
| transforms/projections | **identity** (an unchanged side must not perturb content) + **outcome-class symmetry** under swapping sides | 104 → 1286 ms |
| repair/ILRP/recovery | real `RepairPlan` DAGs through `topo_order`; **deterministic permutation** + dependency order verified against the generator's OWN edge set | 3 → 240 ms |
| Basis/revision/query invalidation | **irrelevant-input invariance** + **monotonicity**, checked against a key set the generator built itself | ~0 → 148 ms |

Counts are tallied from outcomes. The runner now targets **100,000 accepted**
(ADR-0020 §4 says accepted, not attempted) with an attempt cap that fails loudly
rather than silently reporting a short campaign. Measured result: 100,000
accepted per family at 0.00–0.26% discards, all under the 1% ceiling.

`Generated` gained `seed` and `evidence_hash`, **required once `result` is
`pass`** — a family can no longer claim a pass without a reproducible run behind
it — plus an internal-consistency check that `accepted + discards == attempts`.
Re-running from the recorded seeds reproduces the packet's counts and evidence
hashes byte-for-byte.

Two defects the new generators caught immediately, both fixed rather than
asserted away: a cycle probe that wasn't actually cyclic (a lone back edge is
not a cycle without a forward path), and two families whose domain targeting
pushed discards to 1.45% and 16.87%.

---

## F-08 — CRITICAL. **RESOLVED (this session): 53% → 100%.** Real mutation testing showed a 53% kill rate; `liminal-cir` was effectively untested.

The packet predeclares 65 mutants and ADR-0020 §3 requires **100% of
applicable, non-duplicate, non-equivalent mutants killed — one survivor blocks
eligibility**. Actual mutation testing (`cargo-mutants` 27.1.0) reports:

| outcome | count |
|---|---|
| caught | 38 |
| **missed (survived)** | **34** |
| unviable | 11 |
| timeout | 0 |

**53% kill rate over 72 viable mutants.** Every survivor is in
`crates/liminal-cir/src/lib.rs`.

**Hand-verified live, this session** (the run was dated 07-22; nothing since
touched `liminal-cir`, but a stale number is not evidence). I applied the
survivor `replace sorted_relations -> Vec<Relation> with vec![]` — making the
derived CIR graph report **zero relations** — and ran the whole workspace:

```
269 tests run: 269 passed, 23 skipped
```

Not one test noticed that the graph lost every relation. Reverted via
`git checkout` and re-verified clean.

Other survivors of the same character:
- `replace compare_shape -> Result<(), DebugJsonError> with Ok(())` — the
  debug-JSON shape comparison becomes a no-op;
- `replace payload_for -> ... with Ok("xyzzy".into())` — every payload becomes
  a constant;
- six independent `delete !` / `replace || with &&` mutations inside
  `validate_debug_graph` — validation inverted or short-circuited;
- three `replace == with !=` inside `resolve`.

**Impact:** `liminal-cir` (1,141 lines) is the derived-graph layer the Phase 1
projection rests on, and the suite cannot distinguish it from a broken
implementation. This is the single strongest piece of evidence that the suite
is not yet fit to qualify anything — and it is exactly the question HAQP-1
exists to answer, answered honestly.

**Fix — LANDED. Every viable mutant now dies.**

| measurement | caught | missed | rate |
|---|---|---|---|
| baseline | 38 | 34 | 53% |
| batch 1 (8 tests) | 48 | 26 | 65% |
| batch 2 (6 tests) | 64 | 10 | 86% |
| batch 3–4 (3 tests) | 69 | 5 | 93% |
| batch 5 (1 test) | 73 | 1 | 98.6% |
| **batch 6 (1 test)** | **74** | **0** | **100%** |

19 tests over six batches, each CI-gated and committed before the next
measurement. **No equivalent-mutant disposition was needed** — every mutant
died to a real assertion, so ADR-0020 §3's proof-and-concurring-verification
path was never invoked.

Three defects the exercise exposed that were larger than their mutants:

1. **The duplicate-id identity law was unguarded.** `resolve`'s
   `!duplicates.contains(id)` guard decides whether a repeated `{#id}` is
   promoted to `Explicit` identity. Only the *diagnostic* was tested, never
   the resulting identity — so the code could have handed two different nodes
   the same durable identity. That is the exact false promise M07/M09 spent
   milestones measuring.
2. **Containment document order was unasserted.** The three `resolve` `==`
   mutants all feed the containment ordinal — the number a projection replays
   to reconstruct a document. Nothing checked it.
3. **The dirty-buffer projection path was never validated.** No test built a
   graph on a `BufferGeneration` basis, so the path a live editor exercises on
   every keystroke was unverified.

Method notes worth keeping: `compare_shape`'s five mutants could only be
killed by calling it DIRECTLY — driving it through `deserialize_debug_v1`
never reaches it, because `deny_unknown_fields` rejects first. And an
`is_err()` assertion could not kill the framing mutant, because a byte-exact
canonical check downstream subsumes the framing guard; asserting the error
VARIANT was required. A test that asserts only "it errored" against layered
guards proves almost nothing about which guard ran.

---

## Verified clean (checked, not findings)

- **Crash-boundary inventory reconciles**: 8 declared in `packet.json` exactly
  match the 8 runtime `CrashPoint::all()` variants. No drift.
- **The gate fails closed correctly**: the packet gate is `#[ignore]`d with
  `qualification_state: not-run`; no unearned qualification is claimed anywhere.
- **`locked_acceptance_corpora_touched: false`** — held-out corpora untouched;
  nothing in this pass read them.

## Improvement list (ordered; do before re-running the campaign)

1. ~~Anchor all three §112 laws to independent oracles; add degenerate-implementation
   canaries that must fail.~~ **DONE** — `content_witness` oracle + 4 canaries in
   `tests/law_canaries.rs`. *(F-01; F-03's mechanical half)*
2. ~~Resolve the Phase 1 authorization contradiction by amendment; re-ignore the
   three law gates until M19-M21 execute.~~ **DONE** — AM-17.2 declares the
   crates qualification substrate; all SEVEN Phase 1 exit gates re-ignored
   verbatim. *(F-02)*
3. ~~**Supersedes F-04** — implement real per-family generators…~~ **DONE** —
   five real generators, measured accounting, 100,000 accepted each, seeds and
   evidence hashes required for any `pass`. *(F-04, F-07)*
4. ~~Expand tests to cover all five evidence kinds per requirement before
   qualification.~~ **DONE** — inventory 8→38, evidence-coverage and
   anti-clustering checks + 2 canaries. *(F-05)*
5. Re-weight mutant selection by risk with recorded rationale. *(F-06)*
6. Separate implementation authorship from oracle authorship; record identities.
   *(F-03)*

**Round-2 rule (per the agreed cap):** after these land, one full confirmation
sweep. A new confirmed escape in round 2 blocks GO unless Brian explicitly
extends. Reaching the cap with any confirmed escape unfixed and unaccepted
blocks GO by construction.

---

## F-11 — MAJOR. The suite's green status is runner-dependent, and this blocks workspace-wide mutation testing.

**Reproduction:**

```
cargo nextest run -p liminal-conformance scorecard_runs_end_to_end   # PASS
cargo test    -p liminal-conformance --test classes                  # FAIL
```

```
panicked at conformance/tests/classes/trace_replay.rs:96:
  replay failed: store is locked by another process:
  /tmp/liminal-pipeline/<pid>-<thread>-0-adversarial-delete-66_external-file/state
```

nextest runs every test in its own PROCESS; `cargo test` runs them as THREADS
in one process. `pipeline::scratch_root` originally keyed only on
`std::process::id()`, so isolation was accidental — it came from the runner,
not from the code.

**Consequence, and why this is more than a nit:** `cargo-mutants` drives
`cargo test`, so `cargo mutants --workspace` cannot even establish a baseline:

```
ERROR cargo test failed in an unmutated tree, so no mutants were tested
```

The full-workspace mutation run Brian asked for is **blocked** until this is
fixed. Per-file runs (`--file crates/liminal-cir/src/lib.rs`) still work, which
is why F-08 and pass-1 A7 were measurable at all.

**NOT FIXED. It is a genuine intermittent race.** Measured across five
consecutive threaded runs of the same binary: **3 pass, 2 fail** (~40%).

What was ruled OUT, decisively:

- **Path collision.** `scratch_root` now composes pid + a global atomic
  counter + a **UUID** + the trace id. A brand-new, globally unique directory
  still fails to take its own lock. Path uniqueness is not the cause.
- **Sequential double-open.** The AM-8.11 reopen correctly drops the old
  handle first (`self.ws = None;` before re-opening), so it is not that.
- **Single-threaded execution.** `cargo test -- --test-threads=1` passes
  every time.
- **The test in isolation.** Running only `scorecard_runs_end_to_end` under
  `cargo test` passes every time.

So the failure requires (a) threads in one process and (b) sibling tests
running concurrently, and it strikes on the FIRST store open of the run. Since
`flock(2)` associates a lock with the open file description, a second
descriptor on the same inode conflicts even inside one process — but with
globally unique paths there should be no second descriptor. That contradiction
is unresolved, and resolving it means reasoning about the store's locking
discipline, which is `liminal-graph` semantics under AM-17.2 quarantine.
Improvising there is exactly what protocol §3 forbids.

**Retained change:** the globally-unique scratch root stays. It does not fix
the race, but process-only isolation was a real latent weakness (isolation came
from the runner, not the code) and its removal is what proved collision is not
the cause.

**Severity note:** the project's own standing rule is that a flaky test is a
finding, never retried away. This one is flaky AND blocks workspace mutation
testing, so it gates step 3.

**Standing rule this implies:** `just ci` uses nextest, so CI has never
exercised the threaded path. A qualification suite whose result depends on the
harness is not qualified. `cargo test --workspace` should join the continuous
lane once this is fixed.
