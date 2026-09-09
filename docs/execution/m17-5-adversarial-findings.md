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

**RESOLVED. Root cause proven with a control; fix landed.**

### Root cause: `flock` ownership belongs to the open file description, and forks inherit it

`flock(2)` binds a lock to the OFD, not to the descriptor. When this process
spawns a subprocess, the child inherits a duplicate of **every** open
descriptor at `fork`, and `O_CLOEXEC` does not discard them until `execve`.
Inside that window the child co-owns our lock's OFD — so closing *our*
descriptor does **not** release the lock.

Three facts had to line up, which is why it was so hard to see:

1. `StepRunner::open` (`crates/liminal-daemon/src/runner.rs:1685`) deliberately
   **drops the workspace and reopens the same path** — M08.7's
   `seed_durable_inputs` ordering, mirroring a fresh process over the persisted
   store. That reopen is the second open of one inode, and it is the exact
   frame in the failure backtrace.
2. Sibling tests spawn subprocesses constantly — `conformance/src/harness.rs`
   spawns `lim-toy`, `tracegen.rs` and `identity/gitenv.rs` spawn `git`.
3. Only `cargo test` runs tests as threads in one process, so only there can a
   sibling's fork inherit this thread's lock descriptor. nextest gives each
   test its own process, which is why the suite was green there.

**Isolated proof with a control** (`scratchpad/f11repro/src/bin/fork_race.rs`):
a drop-then-reopen loop over 3000 iterations.

| sibling behaviour | reopen failures |
|---|---|
| control — no subprocess spawning | **0 / 3000** |
| sibling thread spawning children | **4 / 3000** |

Same mechanism, same syscalls, no Liminal code involved.

**Evidence that ruled out the alternatives**, in order:

- **fs4/kernel exonerated.** 16 threads × 200 unique-path acquisitions through
  the identical `File::create` + `try_lock_exclusive` sequence: **0 conflicts**.
  fs4 0.13 maps only `EWOULDBLOCK` to `Ok(false)`, so the failure was genuine
  contention, not an error-mapping artifact.
- **No second descriptor and no lock record.** A `#[cold]` probe at the failure
  site caught the live failure: exactly one fd in the process pointed at the
  path (our own), and `/proc/locks` held **no entry for that inode** among 86
  system-wide `FLOCK` lines. The holder was already gone microseconds later —
  precisely what a child that has since reached `execve` looks like.
- **The window is sub-microsecond.** Any inline instrumentation in the failure
  branch suppressed the bug outright (0/12 and 0/15 instrumented, vs 3/10
  pristine), as did running under `strace`. Only a `#[cold] #[inline(never)]`
  probe, which leaves `open`'s own code size intact, could observe it.

**Fix — LANDED.** `GraphStore::open` now calls `acquire_write_lock`, which
retries `try_lock_exclusive` against a 250 ms deadline. The budget dwarfs a
fork→exec window while staying far below human-visible open latency, and a
genuine second writer holds its lock for its whole lifetime — so this delays an
honest `Locked` by at most the budget and never suppresses it.

**This is not a flaky test retried away.** It is a lock acquisition that was
never sound in a process that forks. Two paired regression tests in
`crates/liminal-graph/tests/store.rs` pin both halves, and the pair must stay a
pair:

- `open_waits_out_a_transient_lock_holder` — a holder released after 20 ms must
  be waited out. Verified to **fail** when the fix is reverted.
- `open_still_rejects_a_lock_holder_that_never_releases` — the canary. A holder
  that never releases must still produce `StoreError::Locked`. If this stops
  failing, the single-writer guarantee has gone vacuous.

**Measured result:** 3/10 threaded failures before, **0/15 after**, and
`cargo test --workspace` — the cargo-mutants baseline — now passes cleanly
twice in a row. The full-workspace mutation run is **unblocked**.

**Standing rule, now honored:** `test-threaded` (`cargo test --workspace`) has
joined `just ci`. CI ran nextest only, so the threaded path — the one
cargo-mutants drives, and the one this defect lived in — had never been
exercised.

### Historical record: the state before this session

**Previously reported NOT FIXED.** Measured across five consecutive threaded
runs of the same binary: **3 pass, 2 fail** (~40%).

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

**Where that reasoning went wrong, kept per §6.** Two of those inferences were
false and cost the investigation a full session. "It strikes on the FIRST store
open" conflated the process's first *scratch root* (counter `0`) with the first
open of that *path* — it was always the second, the `runner.rs:1685` reopen.
And "with globally unique paths there should be no second descriptor" assumed
descriptors are only created by us; a forked child creates copies of all of
them without opening anything. The scratch-root uniqueness work that followed
from the first error was still worth keeping, but it was never going to fix
this.

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

---

## F-12 — MODERATE. The pipeline leaks every scratch workspace it creates, and this can take CI down.

**Found while investigating F-11**, not by looking for it.

`pipeline::scratch_root` creates `/tmp/liminal-pipeline/<pid>-<counter>-<uuid>-<trace>`
per replayed trace and **never removes it**. It only cleans a directory it is
about to reuse (`if dir.exists() { remove_dir_all }`), which a fresh UUID
guarantees never happens. So every trace of every run of every test binary
leaks one workspace, complete with its file world.

**Reproduction:** after this session's F-11 loops,
`ls /tmp/liminal-pipeline | wc -l` reported **2394** directories. `/tmp` is
tmpfs on this machine, so those are resident in RAM. It then filled:

```
the temp filesystem ... is full (0MB free) ... writes failed with ENOSPC
```

That aborted a `just ci` run mid-flight. A leak that consumes RAM without
bound and eventually fails the build is not a housekeeping nit — it makes the
suite's result depend on how many times it has been run before, which is the
same class of defect as F-11 (a result that depends on the harness rather than
the code).

**Not fixed — the disposition is a real trade-off and is Brian's call.**
Scratch workspaces are genuinely valuable for post-mortem debugging of a failed
replay, which is presumably why nothing deletes them. The options:

1. **Delete on success, retain on failure.** Keeps every artifact that could
   ever be wanted and bounds the leak to actual failures. Requires `replay_trace`
   to know its own outcome at cleanup time.
2. **Retain all, sweep on startup** — delete `liminal-pipeline` entries older
   than N hours at the first `scratch_root` call. Simplest, but a long CI run
   can still exhaust tmpfs within one session.
3. **Retain all, but honour an env var** (`LIMINAL_KEEP_SCRATCH=1`), deleting
   otherwise. Cleanest default; loses artifacts for anyone who did not think to
   set it before the run that failed.

Option 1 is the recommendation: it is the only one that never discards evidence
from a failure and never grows without bound.

---

## F-13 — MAJOR. **RESOLVED.** Two gate tests depended on a binary someone built earlier, and could silently test a stale one.

**Found by running the workspace mutation campaign F-11 unblocked** — the very
next thing that campaign did was fail.

`conformance/src/harness.rs::resolve_target_bin` searched
`target/{debug,release}/` for the `lim` and `lim-toy` binaries and panicked if
nothing was there. `lim` belongs to `liminal-cli`, so it is never reachable via
this crate's `CARGO_BIN_EXE_*`.

**Reproduction:**

```
rm -f target/debug/lim target/debug/lim-toy
cargo test -p liminal-conformance --test gate_final
  crash_gate_matrix_and_revert_hold ... FAILED
  overlay_debt_visible_without_agenda ... FAILED
  "lim binary not found; build the workspace binaries or run tests via
   `cargo nextest run`"
```

The panic message names the defect precisely: **the suite was documented as
runner-dependent and shipped that way.** `cargo nextest run` builds every
workspace binary up front, so the tests passed there. `cargo test -p
liminal-conformance` — exactly what cargo-mutants generates — builds no other
package's binaries, so the baseline died and no mutant ran. This is F-11's
lesson repeating in a second place.

**The staler hazard is worse than the failure.** When a leftover
`target/debug/lim` *did* exist, these two tests ran against whatever binary was
last built, which need not correspond to the source under test. A green
`crash_gate_matrix_and_revert_hold` could therefore be reporting on code that
no longer exists. It passed locally throughout this campaign for precisely that
reason.

**Fix — LANDED.** `resolve_target_bin` now builds the binary on demand
(`cargo build -p <owner> --bin <name>`), which is a no-op when it is already
current, and only panics if the build failed to produce it. Verified with the
reproduction above: after deleting both binaries, `gate_final` runs 10/10 green.

**Skip was never an option.** Making these tests skip when the binary is absent
would convert a loud failure into a false green, which is the exact defect class
this whole campaign exists to eliminate.

---

## F-14 — MAJOR. **RESOLVED.** The packet-digest gate had been red for three commits and nothing noticed.

**Found while landing the Track A verifier fixes**, by running `just haq-inventory`
— which turned out to have been failing at `HEAD` before I changed anything:

```
Error: review packet markdown missing packet digest 1e2aa209a387c7ac...
```

`verify_markdown_surface_text` hashes `serde_json::to_vec(packet)` — the
DESERIALIZED STRUCT re-serialized, not the file's bytes. So when `af7aa10` added
the `provenance`, `seed` and `evidence_hash` fields to bind the gate to real
evidence (A6's fix), the serialized form changed and the recorded digest went
stale. `packet.json` and `phase1-suite-review.md` were both last edited together
in `c4ba072`, so nothing looked out of sync in the history.

**The gate behaved correctly — it failed closed.** The defect is that nobody
heard it: `just ci` ran `fmt-check`, `lint`, nextest, doc tests, `doc` and
`deny`, but neither `haq-inventory` nor `haq-canaries`. A gate outside CI is a
gate that reports to no one, and this one stayed red across `af7aa10`,
`5da58a7` and `3eb5c56`.

**Fix — LANDED.** Digest updated to the current packet, and both lanes joined
`just ci`. They cost seconds. Confirmed: `all 16 canaries caught`.

**Note for the digest's design.** Hashing the re-serialized struct means any
schema change silently invalidates the recorded digest, which is a maintenance
trap even though it fails safe. Hashing the packet FILE BYTES would bind the
markdown to the artifact rather than to the verifier's current view of it.
Not changed here — it alters what the digest means, and the canary flow hashes
mutated in-memory packets — but it should be decided before qualification.

---

## F-15 — MAJOR. **RESOLVED (configuration).** Two Phase 0 gates cannot run outside a git checkout, which aborted the mutation campaign.

The third runner-dependency defect this session, found the same way as F-13 —
by running the campaign and reading why it died.

cargo-mutants builds each mutant in a COPY of the source tree, and that copy
omits `.git` by default. Two gates verify git-tracked state:

| test | what it needs git for |
|---|---|
| `phase0_fixture_inventory_is_versioned_and_owned` | enumerates TRACKED fixture paths; an untracked tree has no inventory to govern |
| `real_cst_anchor_recovery_report_matches_golden` | checks the frozen measurement's commit provenance against the reviewed golden |

Both failed the unmutated baseline with `fatal: not a git repository`, aborting
the campaign before a single mutant ran:

```
test result: FAILED. 14 passed; 2 failed
ERROR cargo test failed in an unmutated tree, so no mutants were tested
```

**Fix — LANDED as configuration**, `.cargo/mutants.toml` with `copy_vcs = true`.
Confirmed: the run reaches per-mutant checking, which cargo-mutants only does
after a clean baseline.

**Skipping those two tests in the mutation lane was the wrong answer**, and it
is worth recording why, because it is the tempting one: it shrinks the baseline
until the tool is happy. A suite that quietly tests less under measurement than
it does in CI reports a kill rate for a suite nobody actually runs. These two
tests are legitimate — provenance and governance gates genuinely need the
repository — so the environment was wrong, not the tests.

### The pattern across F-11, F-13 and F-15

Three defects this session, all the same shape: **the suite's result depended on
how it was invoked rather than on the code.** Threads vs processes (F-11), a
binary someone built earlier (F-13), a tree with or without `.git` (F-15). Each
was invisible under `just ci` because CI ran exactly one configuration.

That is worth stating as a standing principle rather than three fixes:
**a qualification suite must be invariant under its runner, and the only way to
know is to run it more than one way.** `just ci` now runs nextest AND the
threaded lane; the mutation lane is the third configuration, and it found two
of these three.

---

## F-16 — MAJOR. **RESOLVED.** The recorded generated-evidence seed did not reproduce a run.

**Found while fixing pass-2 #16**, and not on either reviewer's list.

`Generated.seed` exists so a campaign can be replayed (ADR-0020 §1), and its doc
comment said so. It could not. Three of the five families minted identifiers
through `NodeId::new()`, `RepairStepId::new()`, `IdempotencyKey::new()`,
`RepairId::new()` and `TransactionId::new()` — every one of which is
`Uuid::now_v7()`, wall-clock milliseconds plus OS randomness, drawn from outside
the seeded `Rng`. Two runs of seed *S* therefore explored different cases, and a
crash at case 45,231 could not be replayed from the seed that found it.

### Why it stayed invisible: #16 was hiding it

The evidence digest absorbed only `rng.0` after each attempt. Nothing the ids
touched ever reached the artifact, so the nondeterminism had no observable
consequence — the hash was stable *because* it was blind. #16 (a digest
independent of the code under test) and F-16 (a seed that reproduces nothing)
are one defect seen from opposite ends, and **#16 was not fixable without fixing
F-16 first**: absorbing real behaviour into the digest immediately turns
unseeded entropy into a nondeterministic artifact.

### Demonstrated, not argued

Absorbing witnesses *before* seeding the ids made
`generated_evidence_is_byte_identical_across_runs` fail on exactly two families:

| family | before seeding | after seeding |
| --- | --- | --- |
| `source/CST/formatting` | identical | identical |
| `graph/interchange codecs` | **differs** | identical |
| `transforms/projections` | identical | identical |
| `repair/ILRP/recovery` | **differs** | identical |
| `Basis/revision/query invalidation` | identical | identical |

The two that differ are precisely the two that mint UUIDs. That test is now the
standing guard on both findings.

**Fix:** `Rng::uuid()`, a v5 UUID over seeded bytes. v5 rather than a
deterministic v7 because the latter would have to invent a timestamp. Nothing in
these families depends on UUID version or time ordering, and it is a mild
strengthening: v7 ids arrive in ascending creation order, so `topo_order` was
only ever exercised on plans whose id order agreed with insertion order.

---

## F-17 — MODERATE. The fuzz lane's committed evidence cannot be audited after the fact, and half of #17 needs a protocol decision.

Two items here are **NOT fixed** because both change what ADR-0020 counts as
evidence, and §3 forbids improvising qualification semantics.

**1. The recorded logs are unauditable.** Every row of
`conformance/haqp/evidence/fuzz.json` carries `"log": "target/haqp/fuzz-<t>.log"`.
`target/` is gitignored. So the artifact names the file that would substantiate
it, and that file is guaranteed absent by the time anyone verifies. The execution
counts, timings and exit codes are unfalsifiable in the committed state. The
natural fix is a `log_blake3` per row, which makes a later-produced log checkable
— but adding a required field to the evidence schema is a protocol change.

**2. `fuzz_minutes` and `seconds` are two unreconciled claims about one
campaign.** The packet declares `generated[*].fuzz_minutes` per FAMILY
(≥31 each, ≥155 total); the artifact records `seconds` per TARGET (≥150 total).
No mapping between the five families and the five targets is declared anywhere
in ADR-0020 or the packet. `verify_fuzz_evidence` now takes no `packet` argument
at all, and its doc comment records why: inventing that mapping is exactly the
improvisation §3 prohibits. **Decision needed from Brian.**

### What #17 DID close

- `FuzzEvidence` gained `#[serde(deny_unknown_fields)]`. The artifact already
  carried `elapsed_s`, `seed` and `log`; the struct silently discarded all three.
  The campaign recorded its own reproduction seed and the verifier threw it away.
- The target set is bound to `fuzz/fuzz_targets/*.rs` in the tree. The old check
  compared `recorded.len()` against `packet.generated.len()` — a category error
  comparing targets to families, which passed only because both happen to be 5,
  and which accepted a row naming a target nobody wrote.
- A clean campaign that finished far short of its budget is now refused as
  incoherent. This check earned its place immediately: it caught a mistake in its
  own test fixture, because `canonical_round_trip` really did stop at 1675s of
  1800s and forcing its exit code to 0 reproduced exactly the contradiction.
- Duplicate target rows, empty log paths, and a zero campaign seed are refused.

### Verified clean while investigating (not a finding)

The committed fuzz evidence records `exit_code: 1, artifacts: 1` for
`canonical_round_trip`, which looked at first like an untracked crash. It is
**F-09**, correctly handled: minimized and promoted to
`conformance/corpora/regression/phase1/canonical_round_trip/f09-minimized.bin`
per ADR-0020 §4, and deferred to M19. `verify_fuzz_evidence` refuses to qualify
while it stands, which is the correct fail-closed behaviour. Worth noting only
that `haq verify` bails earlier, on `qualification_state: not-run`, so the crash
is latent rather than reported — the same shape as F-14.

---

## F-18 — MAJOR. **RESOLVED (option 1, Brian's ruling).** The packet's fuzz budget exceeded what ADR-0020 requires, and the packet was populated to fit the verifier rather than the campaign.

**Found by a failing test I wrote for the wrong threshold**, which is the only
reason the numbers were ever compared.

ADR-0020 line 90 is explicit: *"Each family also receives one **30-minute**
sanitizer-enabled fuzz campaign … Total fuzz budget is at least 150
target-minutes."*

Three places disagree about that:

| source | per unit | total |
| --- | --- | --- |
| **ADR-0020 (canonical)** | 30 minutes per family | ≥150 target-minutes |
| `verify_generated_inventory` | `fuzz_minutes < 31` → bail | `< 155` → bail |
| `verify_fuzz_evidence` | (no floor before this commit) | ≥150 |
| `haqp_fuzz_campaign.sh` | `SECS=1800` = 30 minutes | 150 target-minutes |
| `packet.json` | `fuzz_minutes: 31` per family | 155 claimed |

So the verifier demands a minute more per family than the ADR asks for, and
**the packet's numbers were evidently chosen to satisfy the verifier rather than
to describe the campaign**: it claims 155 target-minutes while the committed
artifact records 150. The packet overstates the evidence, and the artifact and
the packet contradict each other about the same campaign. This is the concrete
form of F-17 item 2 — the two claims are not merely unlinked, they disagree.

A verifier stricter than the ADR is a false RED rather than a false green, which
is the safer direction, but it did real harm here: it pulled a fabricated
figure into the packet.

**Brian's ruling: option 1 — follow the ADR.** The two branches differed in cost
by 150 minutes of machine time:

1. **Align the verifier to the ADR** (`fuzz_minutes >= 30`, total `>= 150`) and
   correct `packet.json` to 30. Cheap. Ripples: C13's `violation` and
   `expected_failure` both name "31", so the canary row changes; the packet
   digest changes; `docs/execution/phase1-suite-review.md` needs the new digest
   (F-14's lesson). No rerun needed — the committed campaign already satisfies
   the ADR.
2. **Treat 31/155 as a deliberate margin above the ADR** and rerun the campaign
   at `SECS=1860` so the artifact matches the packet. Costs a fresh 155-minute
   sanitizer campaign, and under §1 a rerun is required anyway once any verified
   fix lands, so this may be nearly free if sequenced with the eventual rerun.

Option 1 taken: the ADR is canonical, and a threshold invented by a verifier
does not get to redefine the protocol it is checking. No rerun needed — the
committed campaign already satisfies the ADR at 150 target-minutes.

### What the fix touched, and what nearly got missed

`verify_generated_inventory` now reads `< 30` and `< 150`, and `packet.json`
carries `fuzz_minutes: 30`. The ripple was wider than the digest alone:

- C13's `violation` AND `expected_failure` both quoted "below 31", so the canary
  row changed with the message it asserts on. The canary evidence hash moved
  from `53ea6c8a` to `6eec3a94` as a result, which is the expected consequence
  rather than a surprise.
- `mutate_canary`'s C13 arm returns that same string, now bound to the packet by
  pass-2 #22 — so the three had to move together or the runner would have caught
  the disagreement. That binding did its job on its first real edit.
- `docs/execution/phase1-suite-review.md` carried the stale numbers in **six**
  more places beyond the digest: five per-family "31 minutes" table cells and
  the "155 target-minutes" planned budget, plus the summary row "five families ×
  31 minutes; at least 155 target-minutes". The digest gate would NOT have
  caught these — it hashes the packet, not the prose around it. Found by
  grepping for the numbers rather than by trusting the gate.

**What this commit DID do:** added the per-target floor the ADR does state
(30 minutes), so one long target can no longer carry the 150-minute total while
another runs for seconds. It follows the ADR, not the other verifier, and says so
at the call site.

---

## F-12 — **RESOLVED (Brian's ruling: fix it, to unblock Track C).** The scratch leak was a hard blocker on the mutation measurement, and broader than first recorded.

Measured this session rather than estimated.

**Per-run cost.** One `cargo test --workspace --all-targets` — the runner
cargo-mutants drives — takes 35s and leaks **250 directories, ~400 MB**, none of
which is ever removed.

**Against the campaign.** `cargo mutants --workspace --list` reports **2915
mutants**. Each runs the suite once:

> 2915 × 400 MB ≈ **1.1 TB** of scratch, against 7.3 GB of tmpfs free at the time
> of measurement.

The campaign would exhaust the filesystem after roughly **18 mutants**. This is
not a housekeeping nit deferred behind Track C — it is the reason the Track C
measurement cannot be taken, on any filesystem, until the leak is fixed. A
background sweeper was considered and rejected: deleting a directory under a
still-running test would report a *false kill*, corrupting the exact number the
campaign exists to produce.

**Broader than `pipeline::scratch_root`.** The original write-up named one
function. The leak actually spans **~29 independent call sites** — every one an
ad-hoc `std::env::temp_dir().join(format!(...))` with no cleanup — producing
~90 distinct directory prefixes (`liminal-toyrun-*`, `liminal-store-test-*`,
`liminal-source-test-*`, `liminal-reactor-*`, `liminal-bench-*`, `liminal-asof-*`
and more). The real fix is one shared scratch helper with drop-based cleanup,
not 29 individual patches.

**It has now taken CI down twice.** Once in the previous session, and again this
session: three `just ci` runs' worth of leakage contributed to filling the
machine, and `just ci` failed with

```
replay failed: No space left on device (os error 28)
```

on three tests, all of which pass in isolation once space is freed. A suite
whose result depends on how many times it has been run before is the same defect
class as F-11.

**Aggravating environmental factor, and NOT something to fix by deleting the
user's data:** the root filesystem is at 99% (17 GB free of 928 GB) and swap is
41 GB deep, partly because 25 GB of leaked tmpfs scratch is RAM-backed. Only the
`liminal-*` portion is this project's to reclaim.

---

## F-19 — MAJOR. `just ci` was red at HEAD for four commits, and I reported it green.

Same shape as F-14, with me as the cause rather than the discoverer.

`typos` reads the fourth and fifth characters of the git SHA `c4ba072` — cited
in this very document, introduced by `feda854` — as a misspelling of "by" or
"be". Every `just ci` run since has failed at `fmt-check`, the first recipe.

**Why it went unnoticed:** I invoked CI as `just ci 2>&1 | tail -20`. In a
pipeline the shell reports the exit status of the LAST command, so what I read
as "exit code 0" was `tail`'s success, never the recipe's. Three separate
"CI green" claims this session rested on that measurement. A verification
harness that reports the status of the wrong process is precisely the failure
this campaign keeps finding, and it is worth recording that the reviewer of the
suite made it too.

**Fix:** an `extend-ignore-re` for backticked hex of 7–40 digits in
`typos.toml`. Every findings document cites commits in backticks, so ignoring
the construct is the durable fix rather than rewording one SHA and waiting for
the next hex collision — two-letter sequences that are both valid hex and common
misspellings recur constantly in abbreviated SHAs.

The bound is deliberate: it matches commit-length hex only, so short hex-looking
fragments in prose are still spell-checked. Writing this section proved the
point, since an earlier draft listed those fragments in backticks and took
`fmt-check` red again.

**Standing correction to method:** never read a gate's result through a pipe.
Redirect to a file and test `$?` directly.


### Resolution

`crates/liminal-scratch` — a `ScratchDir` guard that removes its directory on
drop. 27 sites across 21 files now use it.

**Measured, not asserted.** One `cargo test --workspace --all-targets`:

| | leaked directories | per-run scratch |
| --- | --- | --- |
| before | **250** | ~400 MB |
| after | **0** | 0 |

Projected against the campaign that was blocked: 2915 mutants × 400 MB ≈ 1.1 TB
becomes zero.

**Retention policy — the reason nothing deleted scratch before.** Deleting
unconditionally would throw away exactly the file world a failed replay needs.
So `Drop` checks `std::thread::panicking()`: a **failing** test keeps its
directory, with no flag, no foresight and no rerun needed. `LIMINAL_KEEP_SCRATCH`
retains a *passing* run's scratch for anyone watching one. Retained paths are
printed, so a failing run says where its evidence went. Nothing that could be
wanted from a failure is discarded, and success does not accumulate.

Two self-tests pin both halves, and the retention half matters as much as the
deletion half: if it ever stopped holding, a failing test would delete its own
diagnosis, and the pressure to revert to leaking-by-default would return.

**Migration shape.** `ToyRun` was the single largest source — one directory per
scenario per test. It keeps its public `root: Utf8PathBuf` field and gained a
private guard, so all 35 `ToyRun::new` call sites are untouched. Elsewhere
`ScratchDir` derefs to `Utf8Path`, so most helpers changed only their return
type.

**Known limit, documented in the crate:** `Drop` does not run when a process is
killed or aborts, so the crash-injection suite's deliberate `SIGABRT` still
leaks a handful of directories per run. Those are the parent's to own, which is
a larger change than F-12 needs.

**One rationalization found in the wild**, in `benches/benches/liminal.rs`:
*"Deliberately never cleaned up — the OS owns its temp dir."* Nothing reclaims
`/tmp` until reboot; it is tmpfs here, so the leak was resident in RAM. The
comment now records what replaced it.


---

## F-20 — MODERATE. The packet digest hashes the re-serialized struct, not the committed file.

Noted earlier as "should be decided before qualification"; this session gave it
a concrete demonstration.

**Second demonstration, 2026-08-01.** `conformance/haqp/packet.json` was
rewritten from 1-space to 2-space indentation — 1,864 changed lines, every byte
of the file's layout different — and `just haq-inventory` stayed green with the
digest **unchanged**. The digest is therefore blind to the committed artifact's
actual bytes. That is benign for whitespace, but it means the recorded digest
does not pin the file a reader would audit; two byte-different packets can carry
the same digest as long as they deserialize to the same struct.

`packet_digest` hashes `serde_json::to_vec(packet)` — the deserialized struct
re-serialized — rather than the bytes of `conformance/haqp/packet.json`. Two
consequences, both observed while closing pass-2 #20:

1. **Adding an optional field to a verifier struct changes the digest even when
   the packet file is untouched.** Giving `Review` its `findings`,
   `independently_reproduced` and `evidence` fields moved the digest from
   `d1890c2f…` to `6a56eedd…` with no edit to `packet.json` at all, because the
   empty defaults now appear in the re-serialization. The digest therefore
   tracks the SCHEMA as much as the content.
2. **Conversely, changes to the file that the struct does not model are
   invisible.** Whitespace and key order are already normalized away, which is
   arguably fine — but so is any field the struct does not declare, which is
   not. `deny_unknown_fields` on `Packet` would close that half.

Neither direction is a live false-green today: the digest still detects every
edit to a field the verifier reads, and `just haq-inventory` fails closed when
it drifts. But "the committed packet is this exact file" is what the digest
reads as, and that is not what it measures.

**Not fixed** — switching to file-bytes hashing changes what the recorded digest
means and would need the markdown line regenerated once more; worth doing
deliberately rather than as a side effect of another change. Recorded for the
decision before qualification.


---

## F-09 — MAJOR. **RESOLVED (Brian's ruling: fix now, rerun the lane).** The canonical round-trip law failed on an emitted document the parser could not read back.

Found by the HAQP-1 fuzz campaign (`canonical_round_trip`, 2026-07-25), minimized
to `regression/phase1/canonical_round_trip/f09-minimized.bin`. It was the reason
the committed fuzz evidence carried `exit_code: 1, artifacts: 1`, and therefore
the reason `verify_fuzz_evidence` refused to qualify.

### Root cause, reproduced before fixing

M19's grammar is explicit:

```ebnf
assignment = ident, spacing, "=", spacing, value ;
ident      = (ALPHA | "_"), { ALPHA | DIGIT | "_" | "." | ":" | "-" } ;
```

Two places ignored it:

1. **`parse_attributes` accepted any text before `=` as a name.** `emit_attrs`
   writes names BARE (`{name} = {value}`), so a name containing `"` emitted
   source the parser could not read back — the stray quote opened a string on
   the next pass and the `=` was swallowed.
2. **`split_top_level` left `quoted` open forever** after an unbalanced quote,
   so every later delimiter was consumed and the attribute COUNT changed across
   the round trip.

The emitted forms show it directly:

```
emit1: node {… (…"… = ",c");
emit2: node {… (…"… = "\"");
```

No improvisation was needed to fix it: the grammar already said what is legal,
and the parser was simply more permissive than the emitter could support.
Non-`ident` names are now dropped at parse time, and an unterminated quote makes
`split_top_level` rescan treating quotes as ordinary characters, so splitting is
total and gives the same answer on every pass.

Reporting malformed attributes as a TYPED DIAGNOSTIC remains M19's work. What
M17.5 needed is narrower and is now true: the HIR only ever holds attributes the
emitter can round-trip.

### The regression guard was vacuous, which is the more alarming half

`f09_attribute_escape_round_trip_holds` replayed its fixtures through
`std::str::from_utf8` and `continue`d when that failed. **Both fixtures contain
invalid UTF-8** — that is precisely what the fuzzer found — so both were skipped
and the test would have reported success no matter what the code did. It was
also `#[ignore]`d, so it never ran at all.

`fuzz_targets/canonical_round_trip.rs` uses `from_utf8_lossy`. A regression
harness that does not replay what the fuzzer ran is not a regression harness.
The test now uses lossy conversion, asserts emit stability as the fuzz target
does, and is no longer ignored — it is a regression guard, not a Phase 1
capability gate, and the defect it guards is fixed. Meter: Phase 1 backlog
8 → 7, active 316 → 317.

Non-vacuity evidence: before the fix, both fixtures reported
`round-trip equal = false` under lossy conversion; after it, both are true.


---

## F-21 — MAJOR. **RESOLVED.** An empty ordered block emitted a form the grammar does not have.

**Found by the qualification-lane rerun that F-09's fix required** — that is, by
doing the thing ADR-0020 §1 exists to force. F-09 was fixed, the lane was rerun
from a clean tree, and `canonical_round_trip` failed again at 305s of its
1800s budget with a fresh crash. Same defect CLASS as F-09, different cause:
**the emitter produced text the parser could not read back.**

### Root cause

`emit_item` wrote `ordered;` for an ordered block with no children. M19's
grammar has no such form:

```ebnf
ordered = "ordered", spacing, block ;
block   = "{", spacing, { form, spacing }, "}" ;
```

The block is mandatory, and an empty one is `{ }`. So the parser read `ordered;`
back as the literal string `"ordered;"`, and
`parse(emit(parse(x))) != parse(x)` for any document containing an empty ordered
block. The block is now always emitted.

Isolated to four cases before fixing, which is what kept the fix from
over-reaching:

| source | emits | round-trips |
| --- | --- | --- |
| `ordered;` | `literal "ordered;";` | yes — the parser never accepted it as a form |
| `node foo;` | `node foo;` | yes |
| `ordered { }` | `ordered;` | **NO** |
| `node foo { }` | `node foo;` | yes |

`node` was deliberately left alone. `node foo;` is the same grammar deviation —
the grammar makes `node`'s block mandatory too — but the parser accepts a
block-less node and reproduces it exactly, so the law holds. Tightening the
parser there is M19's call, not something this lane needs, and changing it would
have been scope the finding did not justify.

Promoted as `f21-empty-ordered-block.bin`; the corpus guard now requires ≥25
fixtures and both tests were renamed, since the corpus is no longer F-09's alone.

### What this says about the campaign

The first campaign found F-09. Fixing F-09 and rerunning found F-21 — a defect
the first campaign never reached, because it spent its `canonical_round_trip`
budget crashing on F-09 after 305 of 1800 seconds. **A fuzz lane that stops early
has not measured the budget it claims.** The `elapsed_s` coherence check added
for F-17 flags exactly this shape, and here it is again in live data: 305s
against an 1800s budget.


---

## F-22 — MAJOR. **RESOLVED.** Every identifier position the emitter writes bare was unvalidated. Three campaigns found three instances of one defect.

The third `canonical_round_trip` crash, at 660s of an 1800s budget. At that point
the pattern was unmistakable and worth naming rather than patching again:

| finding | position | campaign |
| --- | --- | --- |
| F-09 | attribute name in `(name = value)` | 1st |
| F-21 | empty ordered block emitted as `ordered;` | 2nd |
| F-22 | node name, relation kind/source/target, `attribute` form name, reference target, macro name | 3rd |

**All three are one defect:** `emit_item` writes identifiers BARE — `node {name}`,
`-[{kind}]->`, `attribute {name} =`, `@{name}`, `macro {name}(..)` — while the
parser accepted arbitrary bytes in every one of those positions. Emit therefore
produced source the parser could not read back, and
`parse(emit(parse(x))) != parse(x)`.

Fixing them one at a time was the wrong shape: each fix cost a 2.5-hour campaign
rerun that then surfaced the next instance. So all five remaining positions were
validated at once against M19's `ident` rule, degrading to a `Literal` with a
`MalformedSyntax` diagnostic — the fallback the malformed-relation branch already
used, and one that always round-trips because literals are JSON-escaped.

### A fourth defect the validation exposed for free

`explicit_surface_covers_all_eight_forms` failed on entirely legal source. The
relation kind was parsed with `"-[links]->".trim_matches(['-', '[', ']'])`, which
leaves **`links]->`** — `>` is not in the trim set. So every relation kind had
been parsed wrong since the form was written, and emit produced
`-[links]->]->`. Nothing caught it because nothing validated the kind; adding
`is_ident` surfaced it on the first run.

That is the argument for validating identifiers at the boundary rather than
trusting them: the check paid for itself before it ever ran in anger.

### Method change

A 10-minute in-process probe at a fresh seed (20260730) now runs 205,015 execs
clean with zero artifacts. Iterating with short probes before committing to the
full 150-target-minute campaign is the sequence that should have been used from
the start — three 2.5-hour cycles bought what one afternoon of probes would
have.

Promoted as `f22-bare-identifier-forms.bin`; corpus guard raised to >=26.

## F-23 — MODERATE. **RESOLVED.** The blind-review runner blamed the model for a dead API key.

`scripts/haqp_blind_review.py` calls lamu's `cloud_query` over stdio MCP. lamu
reports **provider failures as ordinary MCP content**, not as an MCP-level
`error`: a dead key comes back as the string

```
error: provider API: {"code":"invalid_request_error","message":"Authentication Fails, Your api key: ****9d40 is invalid",...}
```

`mcp_call` returned that happily and `parse_json` rejected it with **"reviewer
JSON lacks attempts array"**. True, and useless — it accuses the reviewer of a
malformed response when the reviewer was never reached.

The runner stayed fail-closed throughout, so nothing false was ever banked;
`blocked.json` was written and no pass recorded. The cost was diagnostic, not
evidentiary: **two full runs were spent chasing `max_tokens`** (8k → 32k, commit
`6ea781c`) on the strength of a reason that was never about tokens.

### Fix

1. `provider_failure(model, text)` runs on every response before parsing and
   raises naming the vendor and the provider's own message. It catches both the
   `error:`-prefixed form and a bare error object with no `attempts` key.
2. `vendor_liveness(models)` pings each reviewer with a one-token prompt
   **before** the ~200 KB context is sent. A dead vendor now costs three seconds
   and names itself instead of costing a quarter-hour and blaming the model.
3. A distinctness precondition: the runner refuses to start if both passes name
   the same model, because ADR-0020 §6 requires distinct model families and a
   runner that silently reviewed twice with one family would satisfy the letter
   of the record while producing no independence at all.
4. `--self-test` exercises all five guards against the degenerate inputs they
   exist to reject, and against a well-formed review that must NOT be rejected.
   `just haq-blind-review` runs it first. Verified non-vacuous: stubbing
   `provider_failure` to a no-op makes the self-test fail with both messages.

### What this does NOT fix

Only one cloud vendor still answers. Probed 2026-08-01:

| vendor | state |
|---|---|
| MiMo (Xiaomi) | live |
| DeepSeek | `Authentication Fails` — key invalid |
| OpenRouter (all families routed through it) | `402` — "can only afford 72 tokens" |
| Zhipu / Moonshot / DashScope / Anthropic direct | no key configured |

ADR-0020 §6 requires **two blinded reviews from distinct model families**. With
one live vendor that is unsatisfiable, and the distinctness precondition above
now enforces the refusal rather than letting a same-family pair be recorded.
This is a provisioning blocker, not a code defect; it is escalated, not
worked around.

## F-24 — INFRASTRUCTURE. **RESOLVED.** The mutation lane copies `fuzz/target` into every worker and fills `/tmp`.

The scoped `liminal-format` re-run died with `No space left on device (os error 28)`
copying `fuzz/target/.../libserde_json-*.rlib` into a worker tree.

`/tmp` on this machine is a **32 GB tmpfs** (8.5 GB free). `.gitignore` excludes
`/target` at the root, which cargo-mutants honors, but **`fuzz/target` is a
second 656 MB build directory it copies in full** — once per concurrent worker.
At `-j 6` that is ~4 GB of RAM-backed copies for a build cache no mutant reads.

Fixed by pointing the copies at the NVMe, which has 155 GB free:

```
TMPDIR=$HOME/.cache/liminal-mutants cargo mutants -p liminal-format -j 6
```

Pinned in the `mutants` recipe rather than left here, so a fresh checkout gets
it without reading this ledger. The directory must live **outside** the repo:
pointing `TMPDIR` at `.cache/` inside the tree makes cargo-mutants copy its own
worker copies into each new worker copy, and the run dies on `File name too
long` after nesting the path roughly eighty levels deep. Observed, not theorized
— it was the first thing tried.

This is load-bearing for the deferred 2915-mutant campaign, not just the scoped
run: that campaign is the same copy repeated ~2915 times, and would have failed
the same way after hours of work.

**Observed failure mode, stated precisely:** cargo-mutants logged one `ERROR
Worker thread failed` per affected worker and then aborted the whole run with a
non-zero exit. It did *not* silently report a kill rate over a subset. That
distinction matters — a partial-credit failure would have been an evidence
integrity problem, and this is only a cost problem.

Related, and already recorded: the ENOSPC that wrote a corrupt
`.proptest-regressions` file earlier in M17.5 came from the same full `/tmp`.

## F-25 — MAJOR. **OPEN — must close before the ADR-0020 §1 clean-tree rerun.** The F-01 oracle can go vacuous without any canary noticing.

Found by the full mutation campaign (3000 mutants, run of 2026-08-01). At the
~1000-mutant mark `conformance/src/laws.rs` had **21 survivors, every one of
them in `content_witness`** — the single function that makes the §112 laws
non-vacuous:

| survivors | mutation class | effect |
|---|---|---|
| 15 | whole-function constant returns | the witness stops witnessing |
| 1 | `+=` → `*=` (`laws.rs:30`) | every word count stays 0, so `seen >= count` is always true |
| 2 | `+` → `-`, `+` → `*` (`laws.rs:37`) | id-scan index arithmetic |
| 3 | `delete !`, `&&` → `\|\|`, `\|\|` → `&&` (`laws.rs:40,41,43`) | id validation predicate |

`content_witness` is the independent oracle added to close F-01 — the anchor
outside the implementation that makes the §112 self-referential relations mean
anything. **It has no test of its own.** It is exercised only through the laws,
and the laws' other assertions pass without it, so a mutant that replaces it
with a constant survives the entire suite including all nine law canaries.

### Why every canary still passes

Traced, not assumed:

1. **`assert_content_survives` checks emptiness directly, not through the
   witness** (`laws.rs:56-59`). The `erased a non-empty document` assertion
   reads `output.trim().is_empty()`. So the two content-deletion canaries
   (`law_canaries.rs:139,146`) trip that line and pass no matter what
   `content_witness` returns.
2. **`assert_order_survives` tokenizes independently** (`laws.rs:116-121`). It
   uses the witness only for the `ids` exclusion set; under the mutant that set
   is `{"xyzzy"}`, which excludes nothing real, so the order check keeps working
   at full strength and the two `reordered content` canaries still pass.

What actually dies is the part with no canary: the **token-count survival loop**
(`laws.rs:62-69`) and the **durable-id survival loop** (`laws.rs:76-81`). Under
the mutant both compare a constant to itself and are tautologies.

### The gap, stated exactly

Every canary exercises **total** content destruction or **reordering**. Nothing
exercises **partial** content loss: an implementation that drops *some* word
tokens while leaving the output non-empty and the survivors in order. That is
the only failure `content_witness`'s multiset comparison uniquely catches, and
it is precisely the case no canary covers.

This is F-01's shape one layer up. F-01 was "the law compares the
implementation to itself"; F-25 is "the fix for that is verified only through
canaries that do not depend on it". A canary that can pass on an adjacent
assertion does not pin the assertion it was written for.

The general lesson, which is the one worth keeping: **the canary discipline was
applied to the laws but not to the oracle**. Every check needs a canary —
including the checks written to BE canaries. Indirect verification of an oracle
through the thing it verifies is not verification.

### Fix — deferred to the post-campaign batch, by Brian's sequencing

**Direct unit tests of `content_witness` itself** against known inputs — the
word multiset, the id set, and the id-validation predicate each pinned by value,
not inferred from a law's verdict. Plus a canary formatter that drops a SUBSET
of tokens (non-empty output, survivors in order) asserting
`should_panic(expected = "dropped content")`, and one that drops only a durable
id. The unit tests are the load-bearing half: the six operator mutants inside
the function cannot be reached by any canary that only observes the law's
pass/fail. Not landed yet: the campaign is mid-flight and adding
tests now would force a rerun, which is the sequence Brian explicitly ruled out.

### Addendum: the order oracle too, and a question it raises

Later in the same campaign:

```
MISSED conformance/src/laws.rs:118:29: delete ! in assert_order_survives
```

That `!` is in `.filter(|token| !token.is_empty() && ...)`. Deleting it keeps
ONLY empty tokens, so both token vectors empty and the subsequence check becomes
vacuous. The `SortingFormatter` canary should then stop panicking — and a
`#[should_panic(expected = "reordered content")]` test that does not panic
FAILS, which would mean the mutant is caught.

It was not caught. Either the canary still panics for a reason not yet traced,
or **the canary tests are not being executed by the mutation lane at all**. The
second possibility is much the worse one: it would make every canary in
`law_canaries.rs` decorative under mutation, and the canaries are the mechanism
the whole F-01 fix rests on.

**Resolve this before writing any fix**, since which defect is real determines
what the fix has to be. Do not assume the first explanation.

**Tracked at:** M17.5, post-campaign test batch — the same batch that consumes
the campaign's survivor list, before the ADR-0020 §1 clean-tree rerun. This
finding must be closed before the rerun, not after: §1 makes any verified fix
invalidate eligibility until the lane reruns, so a fix landed afterwards costs
the whole lane a second time.

## F-26 — INFRASTRUCTURE. **PARTIAL.** ADR-0020 §6 needs two model families and the cloud roster collapsed to one.

Probed 2026-08-01: **MiMo is the only cloud vendor that answers.** DeepSeek's
key is invalid; OpenRouter — the route to ~370 models covering every other
family — returns `402` and "can only afford 72 tokens"; Zhipu, Moonshot,
DashScope and Anthropic have no key configured.

**Brian's rulings:** use the MiMo plan already paid for; spend nothing on
DeepSeek or OpenRouter; and review in the **cloud**, not locally.

The second family therefore comes from the **Codex CLI** (`gpt-5.6-sol`), which
authenticates against a ChatGPT account rather than an API key — so it is
independent of every dead key in `api-keys.env` and adds no bill. OpenAI against
Xiaomi satisfies §6's distinct-families requirement with two cloud reviewers.

### What the runner gained

- `backend_of()` routes `codex:` names to the Codex CLI and everything else to
  lamu, canaried both directions. A misroute is invisible in the recorded
  evidence: a `codex:` name handed to lamu is reported as an unreachable vendor,
  while a lamu alias handed to Codex is reviewed by whatever model Codex
  defaults to. Either produces a record naming a reviewer that never ran.
- A canary that **both passes do not land on one backend**. §6's independence is
  a claim about families, so two Codex sessions would pass the name-distinctness
  check while sharing a vendor.
- `--output-last-message` rather than stdout scraping. Codex interleaves hook
  lines, tool traces and a token summary with the answer; a runner that scraped
  that stream would eventually mistake a trace line for a review.
- The Codex session runs `--sandbox read-only` in a throwaway directory and is
  never `--cd`'d into the repo. The review context is supplied entirely in the
  prompt, so both passes see byte-identical material — a reviewer free to wander
  the working tree would not be reviewing the same fixed base as its
  counterpart.

Verified end to end: a structured probe through `mcp_call('codex:gpt-5.6-sol')`
returned `{"attempts":[],"probe":"ok"}` in 8 s with no trace contamination.

### Rejected: a locally served second family

An earlier iteration routed pass 1 to a local Gemma. Recorded because the reason
it failed is worth keeping: `gemma-4-26b-a4b-it-q4_k_m` needs 17.6 GB and only
~13 GB was free, so the only model that fit was a 4B — which would have given §6
the FORM of two-family independence with none of the substance. Brian ruled the
local path out entirely; a cloud reviewer of comparable capability to MiMo is
what the requirement is actually for.

Also uncovered along the way, and still true of lamu: each `lamu start` is an
independent stdio server with its own model map, so a model loaded by one MCP
client reads as "marked loaded but missing from" another.

## F-27 — the ADR-0020 §6 blind lane ran for the first time. Both passes FAIL, and they converge.

Run of 2026-08-01 from clean `2e8a2b1`, reviewers `codex:gpt-5.6-sol` (OpenAI,
via the Codex CLI) and `mimo-v2.5-pro` (Xiaomi, via lamu).

| | pass 1 `gpt-5.6-sol` | pass 2 `mimo-v2.5-pro` |
|---|---|---|
| result | fail | fail |
| attempts | 12 | 12 |
| verified defects | 9 | 6 |
| caught violations | 3 | 4 |
| self-declared false positives | 0 | 2 |
| unresolved verified findings | 9 | 4 |

§6 requires **zero** unresolved verified findings, so the packet stays
unqualified. That is the lane working. Nothing was banked as release evidence:
records live under `target/haqp/blind-review/` and `qualification_claim` is
`false`.

### Independent convergence

Two model families, blind to each other, landed on the same two defects:

- **mutant kills are unbound to any execution artifact** (P1-A02 / P2-F02)
- **`generated.evidence_hash` is never checked against a committed artifact**
  (P1-A04 / P2-F03)

Convergence across families is much stronger evidence than either pass alone,
and it is the specific thing §6's distinct-families requirement buys.

### Verified at the cited lines (3 of pass 1's 9 so far)

`verify_qualified_repo` binds **fuzz, crash, review, provenance and test names**
to committed artifacts — M17.5's own earlier work — and was never extended to
**mutants and canaries**. Five of seven evidence kinds are bound; two are not,
and those two are satisfiable by editing a status string.

- **P1-A02 CONFIRMED.** No `verify_mutant_evidence` exists. `killed` is a claim.
- **P1-A03 CONFIRMED.** No `verify_canary_evidence` exists. `run_canary_suite`
  writes `target/haqp/canaries.json`, which the qualified path never reads.
- **P1-A04 CONFIRMED.** `evidence_hash` is checked for **length 64 only** — not
  hexadecimal, never recomputed, and `target/haqp/generated.json` is never read.
  Any 64-character string passes.

**Correction to this session's own record:** a lamu reviewer raised
`canaries.json`/`generated.json` earlier and it was dismissed as a false
positive on the grounds that "nothing reads them; they are gitignored". The fact
was right and the conclusion was backwards — *nothing reading them is the
defect*. Two independent blind passes then found the same gap. Recorded because
the failure was in the verification, not in the reviewer.

### The runner's own defects, found by `gpt-5.6-terra` reviewing the commit

All four verified and fixed in the follow-up:

1. **The backend-diversity guard never ran during `--run`.** It lived only in
   `--self-test`. `just haq-blind-review` happened to catch a same-backend pair
   because it runs the self-test first, but `python3 scripts/haqp_blind_review.py
   --run` with two `codex:` models would have recorded two OpenAI reviews as
   independent. Moved into `main()`. **This also corrects a claim made in this
   session**: the runner was described as refusing that configuration outright,
   and it did not.
2. **An empty `codex:` alias** omitted `--model`, reviewed with an unrecorded
   default, and filed the result under the literal reviewer name `codex:` —
   valid-looking evidence for a reviewer that never existed as named. Rejected.
3. **Parsed review text was persisted unredacted.** `attempts` and `findings`
   ARE model output, the same untrusted text the raw response is deliberately
   never written for. "No credentials" in the prompt is a request, not a
   control. `redact_deep` now covers every persisted string.
4. **`--ephemeral` was missing** from the Codex invocation. This machine's
   config appears not to persist sessions, but a runner that keeps untrusted
   model output off disk must not depend on an unstated default.

### F-27 verification results — 7 of pass 1's 9 confirmed

| finding | claim | verdict |
|---|---|---|
| P1-A02 | mutant kills are status strings; no execution artifact read | **CONFIRMED** — no `verify_mutant_evidence` exists |
| P1-A03 | canary catches likewise; `canaries.json` never consumed | **CONFIRMED** — no `verify_canary_evidence` exists |
| P1-A04 | `evidence_hash` unvalidated | **CONFIRMED** — length-64 check only; not hex, never recomputed |
| P1-A05 | review record's attempts not checked as records | **CONFIRMED** — `verify_review_evidence` counts the array and stops; 12 empty objects pass |
| P1-A06 | findings/resolution not reconciled with the record | **CONFIRMED** — packet fields are cross-checked against each other, never against the record's own `findings`/`unresolved_verified_findings` |
| P1-A07 | reviewer independence, family, isolation, blindness unenforced | **CONFIRMED** — nothing reads `reviewer.backend`, `model_family`, `identity_hash` or `blindness_proof`; the runner writes them and the verifier ignores them |
| P1-A09 | sanitizer requirement not representable | **CONFIRMED** — `FuzzEvidence` has no sanitizer field; the campaign runs ASan and the artifact cannot say so |
| P1-A01 | incremental/full share a parser and oracle | not yet verified |
| P1-A08 | crash scenario evidence ignored | not yet verified |
| P2-F04 | `double_recovery` collapses §5's two-run identity into one string | plausible; needs §5 read |

### The shape of it

Every confirmed finding is the same defect wearing different clothes: **the
verifier checks the packet's shape and its self-consistency, and binds only some
fields to artifacts a run produced.** Where M17.5 already did the binding work —
fuzz, crash, review, provenance, test names — the gate is sound. Where it did
not — mutants, canaries, generated hashes, review record contents, reviewer
identity — the gate reads a claim and believes it.

A07 is the sharpest instance and the most embarrassing: this session BUILT the
`reviewer.backend` / `blindness_proof` fields to make §6's independence auditable,
and the qualification verifier never looks at them. Producing evidence is not the
same as checking it.

### Fix batch (all before the §1 clean-tree rerun)

1. `verify_mutant_evidence` + `verify_canary_evidence`, binding both to
   committed artifacts, each with a canary that a fabricated status is rejected.
2. Recompute `evidence_hash` from the generated artifact; require hex.
3. Reconcile the review record: per-attempt field validation, findings and
   resolution counts, and the reviewer identity/blindness proof.
4. Add a sanitizer field to `FuzzEvidence` and require it.
5. F-25's `content_witness` unit tests.
6. The campaign survivor list.

### F-27 fix batch — five findings resolved

| finding | fix | canaries |
|---|---|---|
| F-25 | unit tests pinning `content_witness` and `assert_order_survives` by value; boundary canary for `assert_parse_discriminates` | 6 unit + 1 canary; **laws.rs 21 survivors → 1** |
| P1-A03 | `verify_canary_evidence` binds every `caught` row to a committed run, including its violation prose | 5 |
| P1-A04 / P2-F03 | `verify_generated_evidence` binds digest, counts and seed; inventory requires hex | 5 |
| P1-A05/A06/A07 | the review record is parsed into typed structs and read: per-attempt fields, result/count reconciliation, reviewer independence and blindness proof | 6 |
| P1-A09 | `FuzzEvidence.sanitizer`, required to be one ADR-0020 §4 accepts; campaign passes `-s` explicitly | 1 |

Two runners now write committed artifacts instead of gitignored ones:
`conformance/haqp/evidence/canaries.json` and `.../generated.json`, joining
`crash.json` and `fuzz.json`.

**Each check is paired with a canary asserting the MATCHING case is accepted**,
not only that degenerate cases are rejected — a verifier that rejects everything
satisfies a rejection-only suite.

#### Residual limits, stated rather than papered over

- **A09 binds configuration, not instrumentation.** The evidence records the
  sanitizer the campaign was *built with*. A clean libFuzzer run emits no
  sanitizer marker to grep for, so nothing stronger is available from the
  artifact. `-s address` is now explicit rather than inherited from
  cargo-fuzz's default, because a requirement satisfied by a tool default is one
  a tool update can silently withdraw.
- **A04 binds to the recorded digest, not a recomputation.** The hash covers a
  100,000-case accept/discard stream that cannot be recomputed at verification
  time. Determinism is pinned separately by
  `generated_evidence_is_byte_identical_across_runs`.
- **F-25's last survivor is equivalent**, documented in place: `close + 1` →
  `close * 1` leaves `rest` starting at `}`, which cannot begin `{#`.

#### Still open

- **P1-A02 — mutant kills remain unbound.** This is the one finding in the
  batch that needs a RUNNER, not a verifier: proving `killed` means applying
  each of the 65 declared semantic mutants and showing the named killing test
  fails. Nothing in the tree does that today, so `qualification_state:
  complete` must stay unreachable until it exists.
- **P1-A01** (incremental/full share a parser) and **P1-A08** (crash scenario
  evidence) remain unverified.
- **P2-F01, F04, F05** unverified.
- The campaign's 1345 survivors outside `laws.rs`.

## F-28 — CRITICAL. The mutation requirement cannot be demonstrated at all yet, and nothing said so.

Found while starting to build the mutant runner P1-A02 asks for. Before writing
it, the obvious question: what would it run?

ADR-0020 §4 requires **at least 64 semantic mutants at a 100% kill rate**. The
packet declares 65, each naming the tests that would catch it. Those 65 rows
name **35 distinct killing tests**. Measured 2026-08-03:

| killing tests | state |
|---|---|
| 27 | **do not exist** — they appear only as strings in `packet.json` |
| 8 | exist but are `#[ignore]`d under the AM-17.2 Phase 1 quarantine |
| **0** | **runnable** |

The 27 are `milestones::m18::…` through `m24::…`. `conformance/tests/milestones/`
contains m03–m11 and no m18–m24 at all, because M18–M24 are authored but
unauthorized. Verified by grepping the whole tree for one of them: the only hit
is `conformance/haqp/packet.json:353`.

### Why this is bigger than P1-A02

A02 said mutation kills are self-asserted status strings with no execution
evidence, and proposed binding them to a runner. That fix is not available: a
runner built today would have nothing to run. **The mutation requirement is
gated on the Phase 1 milestones writing their tests**, and no amount of M17.5
work can satisfy it.

That is not a defect in the packet — predeclaring tests a future milestone will
write is exactly what an inventory is for, and every mutant is honestly marked
`predeclared`. The defect is that **nothing prevented the gap from being closed
by editing 65 dispositions to `killed`**, and nothing stated the dependency.

### Fix

`verify_mutant_killing_tests` at the qualified layer: a mutant claiming `killed`
must name killing tests that the packet declares AND that are not `#[ignore]`d.
An ignored test never runs, so it cannot witness anything; a test that does not
exist witnesses even less (`verify_test_names_exist` already covers that half).

The check is **deliberately unsatisfiable today**, and that is the correct
state. It converts an unstated dependency into a gate that fails with the
reason: *"P1-M001 claims killed by P1-T01 (laws::formatter_idempotence_law_holds),
which is #[ignore]d — an ignored test never runs, so it cannot witness a kill."*

Three canaries, including one proving the `#[ignore]` scanner attaches the
attribute to the following test and not the one after it.

### Consequence for the milestone

`qualification_state: complete` is unreachable until M18–M24 run. That was
already true; it is now machine-enforced and stated. Any plan that plots HAQP-1
qualification before Phase 1 execution is wrong, and this is the proof.

### F-28 follow-up — the M18–M23 milestone tests now exist

Brian's instruction: write the tests for m18–m24. 20 of the packet's 27 declared
Phase 1 milestone tests are written and pass; **7 are deliberately not written.**

| module | written | passing under `--ignored` |
|---|---|---|
| m18 CST frontend | 3 | 3 |
| m19 HIR lowering | 4 | 4 |
| m20 formatter | 2 of 5 | 2 |
| m21 incremental | 4 | 4 |
| m22 renderer | 4 | 4 |
| m23 fuzz corpus | 3 of 4 | 3 |
| m24 aggregate gate | 0 of 3 | — |

All remain `#[ignore]`d under AM-17.2. They exercise real surfaces and all 20
pass when run, so un-ignoring one is an authorization decision at M17.6, not a
rewrite. **No mutant disposition was touched**, so F-28's gate stays closed:
`killed` still requires a killing test that is not `#[ignore]`d.

#### The seven not written, and why that is the stronger choice

| test | missing subject |
|---|---|
| `lim_fmt_interrupted_mid_write_leaves_no_partial_file` | no `lim fmt` command exists |
| `lim_fmt_rerun_after_interruption_is_idempotent` | same |
| `lim_fmt_survives_malformed_input_without_data_loss` | same |
| `benchmark_gate_fails_on_regression_beyond_threshold` | `benches/baselines/` has only `phase1-candidate.json`; there is no baseline to regress against |
| `aggregate_gate_fails_when_any_phase1_gate_is_red` | M24 unimplemented; `bin/gates.rs` is the spec-debt meter, not a Phase 1 aggregate gate |
| `aggregate_gate_records_the_basis_of_every_input` | same |
| `aggregate_gate_rejects_incomplete_evidence` | same |

A test whose subject does not exist can only be a stub that fails
unconditionally — and under F-28's new gate an unconditionally-failing test is a
**vacuity vector**: once un-ignored it would "witness" the kill of any mutant
naming it, with the mutation having nothing to do with the failure. That is
strictly worse than the gap it appears to close.

One near-miss worth recording: the first draft of the benchmark test asserted
`verdict.is_ok() || verdict.is_err()`, which is a tautology — the exact defect
class this milestone exists to find, written by the same hand that has been
finding it. It was caught before commit, and the test was removed rather than
weakened into something that passes.

#### F-03 exposure, restated

These tests were authored by the same agent that authored the implementation
they exercise. F-03 (oracle authorship independence) remains OPEN and this
enlarges it: the Phase 1 suite and the Phase 1 implementation now share an
author across 20 more tests. That is an argument for M17.9 — Brian's own
adversarial review of the suite — not something the author can resolve.

## F-29 — MAJOR. Two of the five critical families have no fuzz target at all.

Found while implementing F-17's family→target mapping. Requiring the mapping is
what made the gap visible, which is the argument for requiring it.

ADR-0020 §4: "Each family also receives one **30-minute sanitizer-enabled fuzz
campaign**." The packet declared `fuzz_minutes: 30` for all five families. What
the five targets actually exercise:

| family | fuzz targets |
|---|---|
| source/CST/formatting | `cst_parse`, `canonical_round_trip`, `format_idempotent` |
| transforms/projections | `html_render` |
| Basis/revision/query invalidation | `incremental_full_equivalence` |
| **graph/interchange codecs** | **none** |
| **repair/ILRP/recovery** | **none** |

So two of the five per-family budgets were backed by nothing, and the packet's
own numbers said otherwise. Not fraud — `result` was `planned` throughout — but
the numbers were shaped to look satisfied, which is F-04's pattern.

A bijection over families and targets is therefore impossible, and the mapping
is declared as the honest partition above: two families name an empty list.

### Fix

- `Generated.fuzz_minutes` is gone. It was one of two independently-assertable
  claims about a single campaign, which is how drift enters; the artifact's
  per-target `seconds` is now the only assertion.
- `Generated.fuzz_targets` names which targets evidence which family — a
  RELATION, not a number. Nothing in it can be asserted independently of a run.
- `verify_fuzz_budget` derives each family's minutes by summing its targets'
  recorded seconds and enforces §4's 30-per-family and 150-total floors against
  the DERIVED value.
- `verify_fuzz_target_mapping` refuses a target claimed by two families: one
  campaign must not satisfy two budgets.
- A family claiming `pass` with no target is refused. Empty is legal at the
  inventory layer, which describes a plan; `pass` for it is not.

### Still open — this is a blocker for HAQP-1a

`graph/interchange codecs` and `repair/ILRP/recovery` need fuzz targets before
either family can reach `pass`, and therefore before HAQP-1a can complete. The
surfaces exist (`liminal-graph`, and the repair/ILRP paths under
`liminal-jurisdiction` and `liminal-cli`), so this is authoring work plus one
60-minute campaign, not a structural problem.

The gate now states the gap rather than hiding it:
*"graph/interchange codecs claims pass but names no fuzz target; ADR-0020 §4
gives every family its own sanitizer campaign, and a family with no target has
had none."*

### Also from this work

- **`log_blake3`** (F-17): every fuzz row now carries the BLAKE3 of its
  libFuzzer log. `log` names a path under `target/`, gitignored and guaranteed
  absent at verification time, so execs/timings/exit codes were unfalsifiable
  once the run ended. Required 64-hex at the qualified layer, defaulted at the
  inventory layer so the pre-F-17 artifact still parses.
  **The committed `fuzz.json` cannot pass the qualified gate until the lane is
  rerun** — the digest of a log nobody kept cannot be reconstructed.
- **`--keep-logs`** commits the logs themselves, opt-in via `KEEP_LOGS=1`.
  Default off: the digest is enough unless someone actually wants to read them.
- **`haq hash`** subcommand, because `b3sum` is not installed here and shelling
  to it would have made an undeclared tool a dependency of the evidence lane.
- **Canary C13** attacked `fuzz_minutes`, which no longer exists. It now claims
  another family's fuzz target, which is the packet-level violation the derived
  budget makes possible.

### F-29 follow-up — both uncovered families now have fuzz targets

`fuzz/fuzz_targets/graph_interchange_codec.rs` and `.../ilrp_recovery.rs`, so
every family in the mapping names a target and none is claimed twice.

- **graph/interchange codec** — decode/encode round-trip stability over
  `Transaction`, not merely totality. Totality alone is satisfied by a decoder
  that rejects everything; a decoder that accepts a value it cannot faithfully
  re-emit is the interesting bug, because it silently rewrites a document on
  save. Encoding determinism is asserted too, or no digest over this codec
  means anything.
- **ilrp_recovery** — R4 §10 / v4 §7.8: an intent driven only through legal
  transitions must always be able to reach a terminal state, and a terminal
  state must never transition onward. Reachability is computed in the target by
  breadth-first search rather than asked of the implementation, per ADR-0020 §5.

Smoke-run at 20s each: 894,420 and 381,885 execs, zero artifacts. The scoring
campaign is part of the lane rerun.

## F-30 — Stage 3 tier 1: the verifier's own kill rate, and what its survivors say.

Scoped campaign over `crates/liminal-xtask/src/haq.rs` — the gate that decides
whether everything else passes. **306 mutants: 75 missed → 65, 226 caught, 15
unviable.**

Ten of the original survivors were in gate logic written THIS session, and they
shared one cause: **the canaries asserted that bad input is rejected and never
that good input is accepted.** Any mutant that widens a guard therefore
survived — a verifier that rejects everything satisfies a rejection-only suite.
That is the same defect this milestone has been finding in other people's code
all along, in the code written to find it.

Killed, with the accept case added:

| site | mutant | why it lived |
|---|---|---|
| evidence-path guard | `== ParentDir` → `!=` | nothing asserted an ordinary relative path is accepted |
| `verify_qualification_stage` | `1b if !claims_kills` guard → `true` | nothing asserted a 1b packet WITH kills passes |
| `collect_ignored` | `&&` → `\|\|`, `\|\|` → `&&` | only `#[ignore]` was tested, not `#[cfg(any())]` or a false positive |
| `verify_fuzz_budget` | `< 150` → `==`, `<=` | the floor was never checked from both sides |

**A test that looked like coverage and was not.** The first boundary test used
five targets at 1800s and claimed to exercise the total-minutes floor. It never
reached it: the per-target floor fires first, so line 766 was never executed and
three mutants there survived a test named for them. Reaching the total floor
needs FOUR targets; reaching the per-target floor with the total satisfied needs
six, one of them between 90s and 1800s so a mutant that moves the floor to
`30+60` or `30/60` is actually discriminated. Both cases are now explicit.

`verify_reviewer_independence`'s `< 2` → `> 2` is **equivalent** and documented
in place: with fewer than two records the loop's `distinct != records.len()` is
false either way, so both return `Ok`. The early return states the intent —
independence is a claim about a pair — and costs nothing.

### The repo-level gates had no test at all

Ten survivors were whole-function deletions — `verify_inventory_repo`,
`verify_qualified_repo`, `verify_provenance`, `verify_markdown_surface` and
every evidence binder survived being replaced with `Ok(())`.

They are exercised **only by justfile recipes**. `just ci` catches a break, and
`cargo mutants` runs `cargo test`, so the mutation lane could not see them:
the gates that decide qualification were invisible to the measurement that
judges the suite. Each is now driven directly, and the evidence binders are
driven with doctored packets, because the qualified gate's `qualification_state`
check fires first and they were otherwise never reached.

**One of those tests did not kill its mutant either.** The first version
asserted `verify_inventory_repo` ACCEPTS the committed tree — which `Ok(())`
also does. A gate is only pinned by a tree it must REFUSE, so the test now
builds one in a `ScratchDir`, verifies the unmodified copy still passes (or the
refusal proves nothing), then doctors the packet to declare itself ratified.
Same lesson as the boundary test above, from the opposite direction: asserting
the accept case is necessary and is not sufficient.

**75 → 55 survivors; whole-function deletions 10 → 2.** The two remaining are
`run_canaries_repo` and `run_generated_repo`, thin IO wrappers whose logic is
tested through `run_canary_suite` and `generate_evidence`.

The remaining 55 are recorded in `m17-5-verifier-survivors.txt` with the full
run in `m17-5-verifier-campaign.log`, committed rather than left in a session
scratchpad: the earlier 3-hour workspace campaign's only record was in a temp
directory and is gone, and an unreproducible measurement is not evidence.

### ADR-0020 §4's generator machinery

The 100,000-case evidence is only as good as the generator behind it. A constant
RNG, or a `word()` returning one string, yields 100,000 "cases" that are one
case repeated — and every count in the packet still looks satisfied.

Killed: the seeded stream is pinned as a fixed sequence from a fixed seed
(reproducibility under §1 IS "this seed yields this stream"); `below()` is
pinned in range and for actual variation; `word()` for content, length bounds
and alphabet; and `generate_evidence`'s shape — five distinct families, each
meeting its acceptance target with counts describing one run.

**`generated_evidence_is_byte_identical_across_runs` passed under every one of
the three whole-function replacements**, because it compares two runs to each
other and two EMPTY runs are identical. A determinism test is not a shape test.

`validate_case_count` was extracted from `generate_evidence` so its boundaries
are testable: `>` → `>=` at the safety limit can only be discriminated by a run
of exactly 10,000,000 cases, which no test can afford. Extracted, it is checked
from both sides in microseconds.

### A golden that covered nothing, and the discard that recorded nothing

`case_repair`'s eight survivors were all in the cyclic-dependency construction.
Two attempts failed before the real cause surfaced, and both are worth keeping:

1. **A 64-case golden never executed the branch.** `case_repair` closes a
   genuine cycle on roughly 1 case in 384, so a 64-case run never reached the
   code the golden was written to pin. Raised to 3,000 (~0.6s), which exercises
   every arm.
2. **At 3,000 cases the branch ran and the mutants still survived.** A cyclic
   plan is correctly refused by `topo_order` and returns `Case::Discarded` — and
   the runner recorded a discard as the single byte `b"D"`. So the digest
   absorbed the COUNT of discards and nothing about them. Every one of the six
   mutants kept the plan cyclic and only changed the cycle's SHAPE, which the
   evidence had no way to see.

ADR-0020 §4 says generator attempts and discards are **recorded**. Counting is
not recording. `Case::Discarded` now carries a witness the digest absorbs, the
same way `Accepted` has since pass-2 #16, and the runner rejects an empty one —
otherwise the evidence records that something was discarded and not what.
`case_repair`'s witness carries the refused edge set, so the shape of the cycle
reaches the digest.

All six died on the next run.

### Thresholds, and a test that proved whichever case it happened to take

Seven survivors sat on inventory bounds tested only from the failing side, so
any mutant that moved a bound by one survived. A bound is defined by the pair of
values that straddle it, and each now has both.

**One of those tests was conditional.** The operator-ceiling fixture used a
running counter and then BRANCHED on where it landed — so whichever side it hit
was the only side asserted, and the bound stayed unpinned either way. It now
places exactly 16 on the target operator and spreads the remainder round-robin
so no OTHER operator breaches the same ceiling and masks the case under test.

**Two bounds are unreachable and are documented as equivalent rather than
contorted into a test:**

- `share > MAX_KILL_SHARE` — `share` is `count / total` over 65 declared
  mutants, and 65/4 is not an integer, so `share == 0.25` exactly cannot occur.
- `discards * 100 > attempts` — exactly 1% needs `accepted == 99 * discards`,
  which is not expressible alongside the packet's ≥100,000 accepted floor and
  its own declared counts. The ceiling is checked from both sides at the nearest
  reachable pair (1,010 passes, 1,011 fails).

**Tier 1 total: 75 → 21 survivors, 216 → 271 caught (92.8% of viable).**

Remaining: `run_generated_repo` (5, mostly its progress arithmetic),
`verify_test_names_exist` (3), `mutate_canary` (3), `verify_qualified_repo` (2),
`verify_packet_shape` (2), and 6 others.

## F-31 — Stage 4: the five unverified blind-pass findings, adjudicated.

Each read at its cited line. Three real, one real-but-known, one already closed.

### P1-A01 — **CONFIRMED, and it is a VACUITY, not a bug.**

`ParagraphCompiler::incremental` (`crates/liminal-query/src/lib.rs:91`) is:

```rust
let edited = self.apply(source, edits);
self.full(&edited, basis)
```

with a comment saying it "deliberately shares the canonical parser with full
compilation until subtree reuse has its own benchmark and oracle evidence".

That is a defensible implementation choice. Its consequence is not: §112's
incremental law is `incremental(x, edits) == full(apply(x, edits))`, and this
implementation satisfies it **by construction**. The law cannot fail. So
`incremental_equals_full_compile_law_holds` — an M21 exit gate — proves nothing
about the implementation it names, and neither do the four `m21.rs` milestone
tests written this session, which assert the same identity.

What still has force is the canary: `ConstantCompiler` proves the LAW rejects an
input-ignoring compiler. That is a statement about the law, not about
`ParagraphCompiler`.

**Disposition:** record, do not "fix". Making `incremental` diverge from `full`
to give the law something to catch would be tuning the implementation to the
measure. The law becomes meaningful at M21, when subtree reuse arrives — and it
must be re-examined then, because a law that has been green throughout is the
easiest kind to assume is working. Added to the M21 exit criteria.

### P1-A08 — **NOT CONFIRMED as I first recorded it. Correcting my own verdict.**

I initially confirmed this on the reasoning that the packet declares
`before: true, after: true` per boundary while the artifact records one
occurrence count, so "nothing carries which SIDE was exercised". **That was
wrong, and I should have read the registry before writing it down.**

The before/after matrix is not two unrecorded sides of one boundary. It is
**eight separately registered boundaries** — `ilrp/before_intent_commit` and
`ilrp/after_intent_commit` are distinct names — and `crash_evidence` fails if
any registered boundary never fires (`never_fired.is_empty()`) or if any
unregistered boundary does. All eight fire. `ToyRun::crash_matrix` then crashes
at every `(point, occurrence)`, recovers twice, and compares terminal state and
world digest. The matrix is run and it is exhaustive.

This is the second time this session my own verification, not a reviewer's
claim, was the weak link — the first being the `canaries.json`/`generated.json`
dismissal that two blind passes then found independently. Both failures share a
shape: I reasoned from the shape of the data to a conclusion about the code
without reading the code that produces it.

**What IS real, and smaller:** the artifact carries a `scenarios` block —
per-scenario `faults_injected`, `boundaries`, `result` — and `verify_crash_rows`
never reads it. `CrashEvidence.scenarios` is typed `Vec<serde_json::Value>` and
goes nowhere. So "crash scenario evidence is ignored" is literally true of the
verifier, just not for the reason given. **Fixed below.**

Also real and smaller still: the packet's `before`/`after` booleans are constant
`true` on every row and are checked only against each other. Their meaning is
already carried by the boundary NAMES, so they are a declared field with no
evidence behind it — the same shape as the `fuzz_minutes` F-17 removed.

### P2-F04 — **CONFIRMED, same shape as A08.**

`double_recovery` is a `String` that reads `"pass"` — and it is a **literal in
the generator**, not a measurement written from one.

Stated precisely, because the distinction matters: `ToyRun::crash_matrix` DOES
recover twice and compare terminal state and world digest, and it errors if they
differ, so the `"pass"` is a consequence of the matrix having succeeded rather
than a fabrication. But the artifact cannot distinguish "double recovery was
verified" from "someone typed pass", which is exactly the property M17.5 has
spent its length removing everywhere else.

**Fix (scoped):** record the two recovery state digests and have the verifier
compare them, rather than accepting a constant. Lands with the crash-lane
rerun.

### P2-F05 — **CONFIRMED as stated, but half-closed since.**

C14 drops a crash boundary from the packet and expects
`crash-boundary inventory mismatch` — it exercises the INVENTORY layer only.
When F-27 added `verify_crash_evidence`, no canary was added for the binding
layer.

The unit test `every_evidence_binder_rejects_a_claim_its_artifact_contradicts`
now covers it. What is still missing is a packet-level **canary row**, which is
what a reader of the canary table would look for. **Fix (scoped):** a C17 row
that doctors the packet against the committed crash artifact.

### P2-F01 — **NOT REPRODUCED as stated.**

The claim is that `case_invalidation`'s irrelevant-input invariance goes vacuous
when an unrelated key collides with an expected one. The relation discards when
`expected.is_empty()`, and the collision path the reviewer describes would make
the case *stronger* (a collision means the unrelated write DID touch a watched
key, so the invariance assertion becomes non-trivial). Recorded as unreproduced
rather than dismissed: pass 2 declared 2 of its own findings false positives, so
its precision is not in doubt, and this may be a description of a real defect
that the summary did not capture. Re-examine if `case_invalidation` changes.

### Net

One of pass 1's nine (A01, recorded as DG21.1) and two of pass 2's four remain
open. A08 is downgraded to "the verifier ignores the scenarios block", which is
fixed here; the rest need the crash lane to record more than a verdict, which is
why they were not folded into Stage 3.

## Fresh blind rerun — 2026-08-14

Base `984fdc64a4ec666f5502ce884f1b1dd9f3bb2704`, tree
`d03e92444c8b81f5c442189e90556c217523039c`, clean. The runner recorded two
isolated passes, 24 attempts total, and made no qualification claim. Pass 1
(`codex:gpt-5.6-sol`) recorded ten independently reproduced verified findings;
Pass 2 (`mimo-direct:mimo-v2.5-pro`, Xiaomi Token Plan) recorded twelve
attempts, zero findings, and passed.

Mechanical closures in this follow-up:

- A01: runnable-test scanning now scopes `#[cfg]`/ignore exclusion to the
  immediately following test and resets state; regression test added.
- A04: false-positive attempts now require independent reproduction evidence;
  regression test added.
- A06: fuzz evidence now records seed count and a BLAKE3-bound manifest of
  sorted committed seed names plus SHA-256 input digests; all seven current
  corpora bind and exceed the 16-seed floor.
- Earlier F01: crash evidence now binds source commit, source tree, and
  lockfile digest to packet provenance or the current review base.

Remaining T1 decisions, not silently changed here: semantic mutation-failure
versus infrastructure-failure classification (A02); mandatory Pass-1 attack
class coverage (A03); resolution evidence binding to the cited commit and
changed hunk (A05); expanding the frozen 16-canary inventory to every
provenance/campaign/oracle/residual-risk and M17.3 conjunct (A07); generated
case category binding (A08); mutant `source` schema changing from requirement
ID to exact file:line plus a separate requirement link (A09); and the packet
schema/evidence contract for oracle independence (A10).

## Post-mechanical rerun — 2026-08-14

Base `c315c6f7305c7b1be555e47fa3fe9de038320970`, tree
`6704a5f01b62c93138b0d09f4107efdd8125f4e7`, clean. The runner recorded two
isolated passes, 24 attempts total, and made no qualification claim. Pass 1
(`codex:gpt-5.6-sol`) recorded six independently reproduced verified findings;
Pass 2 (`mimo-direct:mimo-v2.5-pro`, Xiaomi Token Plan) recorded twelve
attempts, zero findings, and passed.

Mechanical closures in this rerun's predecessor were exercised again:

- Compact emission now refuses terminal durable-marker literals and every
  compact heading/fence/quote/list projection; formatter regression coverage
  proves explicit fallback preserves HIR.
- Resolution coordinates must land on an added line in a zero-context diff;
  locked acceptance corpus components are rejected for targets, coordinates,
  and evidence paths.
- The 25% kill-concentration boundary has an accepting-side test.
- Blind review negative coverage now pins locked-target rejection, exact status
  tokens, and strict-descendant resolution ancestry.

Remaining T1 decisions from this rerun, not silently changed: restrict the
qualification child commit to packet-only metadata changes (P1-A01); replace
self-declared sanitizer labels with verifiable instrumentation/build/runtime
evidence (P1-A02); bind corpus manifests to raw tracer output, traced process,
and completeness (P1-A03); add independent duplicate-effect and mixed-Basis
crash evidence (P1-A04); represent and gate each generated metamorphic relation
and independent oracle (P1-A05); and close the canary registry over every M17
Level-3 conjunct with exact bidirectional mappings (P1-A06). Existing T1 items
from the prior rerun remain open: mutation infrastructure-failure semantics,
Pass-1 attack-class coverage, mutant concurrence identity binding, generated
category policy, mutant source-coordinate schema, and the oracle-independence
packet contract.

## F-32 — MAJOR. **RESOLVED.** The fuzz seed evidence was bound to one machine's scratch directory.

`.gitignore:15` excludes `fuzz/corpus/`, and sixteen seeds per target are tracked
anyway (force-added before the rule). `verify_fuzz_seed_manifests` then reads the
**filesystem** and calls the result *"the committed corpus"*:

```
fuzz evidence must bind every committed seed corpus:
cst_parse: evidence records 18233 seeds but committed corpus has 18611
```

It is not the committed corpus. It is sixteen tracked seeds plus every input
libFuzzer has ever written into that directory. On this machine `cst_parse` holds
**18,611** files against 16 tracked; `format_idempotent` holds 10,488.

So the recorded `seed_count` describes one machine's scratch state. Three
consequences:

- **A fresh clone cannot satisfy the check at all** — it has the sixteen, and the
  committed evidence claims 18,233.
- **Any fuzz run changes the answer.** Killing the 2026-08-16 campaign added 378
  inputs to `cst_parse` and turned a green tree red without a line of code
  changing.
- ADR-0020 §1 requires a reproducible campaign. This binds the evidence to a
  directory nobody else can reproduce, and §4 asks for "at least 16 predeclared
  seeds" — which is exactly what IS tracked.

### Fixed — bound to tracked state, and the misplaced assertion moved

`seed_manifest_digest` and the campaign script now both enumerate via
`git ls-files`, in the direction the error message already claimed. Sixteen
tracked seeds per target is exactly ADR-0020 §4's "at least 16 predeclared
seeds", and fuzzer output no longer perturbs the evidence. A corpus with nothing
tracked is refused outright, because it cannot reproduce a campaign anywhere.

### The structural question underneath, for Brian

This is not only a wrong `read_dir`. Unit tests now assert that **committed
evidence is coherent**, and evidence is produced at the fixed base but committed
in the metadata child. So at any fixed base, those tests see the PREVIOUS
campaign's artifacts. Today they pass because this machine's corpus happens to
match the last run — coincidence, not correctness.

That means the qualification lane and the CI suite disagree about when evidence
is supposed to be valid, and the disagreement is currently hidden by local state.
**Brian's ruling: do both (1) and (2).** Bind evidence to tracked state so it
is deterministic and reproducible, and move the misplaced assertion into the
lane.

The distinction that makes this tractable is that the evidence-reading tests are
two different kinds, and only one was misplaced:

- **Fixtures.** Most read a committed artifact, doctor a row, and assert the
  verifier rejects it. They assert nothing about whether the artifact is
  currently valid, so they hold at any commit and stay in CI. They are the
  reason a real artifact is worth committing at all.
- **Current-validity claims.** `committed_fuzz_rows_bind_the_declared_seed_sets`
  asserted the committed evidence satisfied the seed binding *right now*. That
  can only be true at a metadata child. It has been replaced by
  `the_seed_manifest_counts_tracked_seeds_only`, which asserts the property CI
  can actually own — the digest counts tracked seeds, deterministically — while
  `verify_fuzz_seed_manifests` keeps the binding inside `verify_qualified_repo`,
  where evidence is required to describe the tree.

So CI now asserts what is true of every commit, and the lane asserts what is
true only of a qualified one. The two no longer disagree, and neither depends on
a machine's scratch directory.

## F-33 — CRITICAL. **DEFERRED TO 1b, and disclosed.** More than half the mutation plan cannot be applied.

Found by the debiased blind pass 1 (A04, A06, A10) and confirmed by adding an
anchor check: **36 of the packet's 65 declared mutants are anchored to
DECLARATIONS** — `pub fn relations_from(`, `pub fn overlap_slots(`,
`fn slot_map(`, `Federated {` — lines that hold a name and no behaviour.

A function signature has no predicate to invert, no threshold to shift, no crash
point to disable and no ordering to perturb. Whatever the declared operator, the
mutation cannot be applied there. **A mutant that cannot be applied can never be
killed, while still counting toward §3's denominator.**

It is systematic rather than incidental — spread across every operator family
(4 `oracle-short-circuit`, 4 `disabled-crash-point`, 3 each of six others), which
is the signature of anchors pointing at enclosing signatures rather than the
mutable lines inside them.

### The arithmetic, which is the part that matters

ADR-0020 §3 requires **at least 64 semantic mutants**. 65 declared − 36
inapplicable = **29**. `verify_mutant_inventory` counts rows and passes, because
counting rows is not counting mutants.

So 1b is not "re-point some anchors". It is re-anchoring 36 AND authoring
roughly 35 more real ones. That is the same job as F-06's risk-weighted
selection and it belongs with it.

### Why deferred rather than fixed

Blocking 1a on this would contradict ADR-0021, which deferred §3's mutation
clauses to 1b precisely so 1a could complete. And Phase 1's real code does not
exist yet: mutants anchored to today's substrate are guesses about tomorrow's,
so re-anchoring now means authoring the plan twice.

The check therefore runs inside `verify_qualified_repo`'s `1b` branch. It
refuses there, so M24 cannot rediscover this after Phase 1 is built.

### Pinned

Three canaries: declarations are refused; real mutation sites are ACCEPTED; and
the committed plan's inapplicable count is asserted at exactly 36, so
re-anchoring the plan without updating this finding turns a test red.

**A correction to my own first attempt.** The check originally matched operators
to line tokens and falsely accused three legitimate anchors —
`NodeFlags(self.0 | other.0)` is a real inversion site, `if rev ==
inner.state.head` a real staleness comparison, `for entry in entries.flatten()`
a real ordering site. Guessing which token an operator needs produces false
accusations against a mutation plan; "is this a declaration" does not. The
narrower check is worth more than the broader one it replaced.

### Also fixed here, from the same blind pass

- **A08 (confirmed).** The markdown's `status` was checked with
  `contains("status: proposed")`, so the frontmatter could read
  `status: ratified` and pass as long as the literal survived anywhere in the
  file — one sentence of prose suffices. Now parsed as a header.
- **A09 (confirmed).** `fixed_review_base` appeared **only in docs**; no Rust
  file read it. The markdown declared which tree was reviewed and nothing bound
  that to provenance. Now bound, with `NOT_RUN` required while unqualified.
- **The flip's off-by-one.** `git()` stripped the whole porcelain output, eating
  the leading space of the FIRST line only and shifting it one character, so
  `conformance/...` became `onformance/...` and matched no metadata prefix. It
  refused a completed 4-hour lane. Third variant of one bug, each time from
  keeping a local copy; the local copy is gone.

### Remaining, unfixed

- **A07 (plausible)** — the locked-corpus check is substring matching on trace
  text. It catches direct access; a symlink or rename evades it.
- **A11 (plausible)**, **A05 (unverified)**.
- **P2-A02, P2-A11 (not actionable as stated)** — both ask the verifier to
  statically inspect an oracle's body for coupling. That is undecidable in
  general, and F-03 already records the limit.

## F-34 — the last three blind-pass findings: two fixed, one needs a ruling.

### A11 — **CONFIRMED, FIXED.** A label rename counted as skipping a durable transition.

`verify_mutant_operator_patch`'s `skipped-durable-transition` arm matched bare
WORDS: `["prepare", "ack", "finalize", "commit"].any(|t| before.contains(t) &&
!after.contains(t))`. The anchored site is

```rust
self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;
```

so renaming the label from `ack:` to anything else satisfied "a durable
transition was skipped" — while `commit_intent(...)` still ran and persistence
was intact. The operator validated a substring, not a behaviour.

The markers are now call-shaped (`commit_intent(`, `prepare(`, `persist(`, …)
and the call must be ABSENT from the after-text. Canaried both ways: a relabel
is refused, deleting the call is accepted.

### A07 — **CONFIRMED, HARDENED.** The locked-corpus filter was sound only by luck.

The corpus audit refuses a traced path containing `heldout` or
`conformance/corpora`. That is substring matching on what the process SAW, so a
symlink or rename gives the same bytes a name the filter does not recognise and
the audit reports clean.

Canonicalizing strace output after the fact is unreliable — the symlink need not
survive to verification time. So the aliasing is made impossible instead:
`verify_locked_corpus_has_no_aliases` walks the tree and refuses any symlink
resolving into `conformance/corpora/heldout`. With exactly one name, matching
that name is sufficient, and the existing filter becomes sound rather than
lucky.

(No alias exists today. That was the point: the check turns a coincidence into
an invariant.)

### A05 — **CONFIRMED, but my first survey was wrong in both directions. RESOLVED by ruling (2026-08-26): scope to ILRP, disclose in the packet, defer the rest to M17.9.**

The conclusion holds and hardens: ADR-0020 §5's "exhaustive registered crash
boundaries" is asserted and never checked, and read literally it is **false
today**. But the table I first published was wrong in every row, and the real gap
is somewhere else entirely. Both versions are kept here because the error is the
instructive part.

**What I first reported — wrong:**

| file | injection sites | registered |
|---|---|---|
| `liminal-jurisdiction/src/ilrp.rs` | 14 | 8 (`ilrp/*`) |
| `liminal-daemon/src/crash.rs` | 9 | 0 |
| `liminal-daemon/src/main.rs` | 2 | 0 |
| `liminal-cli/src/format.rs` | 4 | 0 |

**What is actually there — verified by reading each file:**

| file | what it is | unregistered durable transitions |
|---|---|---|
| `liminal-jurisdiction/src/ilrp.rs` | the 8 protocol boundaries, at 8 call sites | **0** — fully registered |
| `liminal-daemon/src/crash.rs` | the injector *implementation*; 5 of the 9 "sites" are calls inside one `#[cfg(test)]` unit test, all naming registered points | **0** |
| `liminal-daemon/src/main.rs` | a `Fire` debug subcommand that resolves its point from `CrashPoint::all()` and errors on unknown names — closed over the registry by construction | **0** |

My 14 for `ilrp.rs` counted the trait declaration, the no-op impl and the blanket
impl as injection sites. My 11 for the daemon — the finding's headline — was
**entirely phantom**: that code is the injector and its own test.

**The gap the survey missed.** `liminal-graph/src/store/mod.rs:473` documents
itself: *"Durable boundary: the record is fsynced before the commit returns."*
`store/log.rs:100` says the return of `append` **is** a durable boundary. So
every `txn.commit()` is one, and they are unregistered en masse:

| surface | durable transitions | registered |
|---|---|---|
| `liminal-jurisdiction/src/ilrp.rs` | 8 | **8** ✓ |
| `liminal-daemon/src/runner.rs` | 24 `txn.commit` | 0 |
| `liminal-daemon` executor/session/workspace/reactor | 6 `txn.commit` / `commit_if` | 0 |
| `liminal-source/src/file.rs` | atomic replace + dir fsync (122, 165) | 0 |
| `liminal-graph` store log/snapshot | the fsync boundaries themselves | 0 |
| `liminal-cli/src/format.rs:73` | 1 `commit_if`, M20, and its injection *unwinds* rather than aborting — strictly weaker than a crash boundary | 0 |

The registry covers **8 of roughly 40** durable transitions.

**Ruling (Brian, 2026-08-26): option 2 for 1a; option 1 to M17.9.** §5's
exhaustiveness is scoped to the ILRP protocol boundaries, recorded explicitly in
the packet rather than left implicit, with every other durable transition out of
scope for 1a. Registering the rest requires deciding which of ~40 transitions are
registrable boundaries — runtime design work, not qualification work, and it
would block 1a on Phase 1.

Note that option 1 as originally written ("register the daemon's") was mostly
phantom. What M17.9 actually inherits is the larger question: **which durable
transitions across the runtime are registrable crash boundaries.**

**How the disclosure is kept honest.** A scope declaration that is only prose
rots the moment someone adds a durable transition. `verify_crash_boundary_scope`
binds it: the declared in-scope subsystem must contain every `crash_if_armed`
call site in the tree, every such site must name a registered `CrashPoint`
variant, and the deferred surface is pinned to a measured count so a new
`txn.commit` turns the packet red until the disclosure is updated or the
boundary registered.

## F-35 — my own pre-launch pass: five defects, one of them a guaranteed lane-killer.

Brian asked for the next rerun to be hardened before launch, after two four-hour
lanes died on bugs a static pass would have caught. This is that pass, run
against this session's own failure taxonomy.

### 1. The lane would have failed at `haq-verify`, guaranteed. **FIXED.**

`verify_campaign_clock` reads `conformance/haqp/evidence/campaign.json`.
**That file does not exist**, and `haqp_qualify.sh` never invoked
`scripts/haqp_campaign_clock.sh`, which is the only thing that writes it. So the
next lane would have run four hours, flipped, and failed on a missing artifact.

The unit test passes because it builds its own fixtures in a `ScratchDir` — the
same fixture-versus-current-validity split as F-32. The logic was proven; the
artifact's existence never was.

Fixed structurally rather than by adding a call: the clock now exports
`HAQP_CAMPAIGN_CLOCK`, the lane **refuses to start without it**, and
`just haq-lane` is the one correct invocation. Forgetting it is no longer
possible.

### 2. The whole tail of `verify_qualified_repo` had never executed. **PARTLY FIXED.**

`just haq-verify` has always failed on `qualification_state: expected complete`,
which sits near the top — so every check after it has never run against a real
packet. Mutation confirms: `verify_recovery_proof_presence`, `verify_crash_replay`
and `verify_canary_evidence_replay` all survive replacement with `Ok(())`.

Two are now driven directly. Same shape as F-30's repo-level gates, one layer
deeper, and the lesson repeats: **a gate that has only ever been observed
refusing has not been observed working.**

### 3. A failed sanitizer build passed the "successful build" assertion. **FIXED.**

`build_log_text.contains("Finished") || contains("finished")`. cargo prints
`Finished` for the dependency graph and then `error: could not compile` for the
final binary, so a failed build satisfied it — demonstrated by appending one
error line to a real committed log. Hard-error markers are now refused.

### 4. The lane was not re-runnable. **FIXED.**

§1 makes every verified fix invalidate eligibility until the lane reruns, so
rerunning must be possible. But the blind lane refuses unless the packet is
`proposed`/`not-run`, so the SECOND run would have died at the reviews. The
orchestrator now resets the packet loudly instead of leaving a hand-edit as an
undocumented prerequisite.

### 5. `just haq-mutants || echo` swallowed every failure. **FIXED.**

Only the expected `not-ready` was meant to be tolerated; the `||` tolerated
everything, including a real mutant-lane failure. The F-19 family — masking an
exit code — for the third time this milestone. Now only `not-ready` passes.

## F-36 — 27 gates in the verifier that no test proves do anything

The completed verifier mutation campaign's most serious result. **27 functions
in `crates/liminal-xtask/src/haq.rs` survive being replaced wholesale with
`Ok(())`.** Measured against the test module:

| | count |
|---|---|
| survive `-> Ok(())` | 27 |
| of those, with **no** reject assertion anywhere | **26** |
| of those, never called by any test at all | **17** |

Among them: `verify_requirement_sources` (§2 traceability),
`verify_gate_unignore_only` (AM-17.6's guard on the metadata child),
`verify_locked_corpus_has_no_aliases`, `verify_crash_replay`,
`verify_crash_injection_bindings`, `verify_cross_pass_reproduction` (§6
independence), and the three `verify_corpus_scope_*` checks.

**The cause is structural, not 27 oversights.** `verify_qualified_repo` checks
`qualification_state == "complete"` and returns before anything else. The
committed packet is `not-run` — correctly, the gate is honestly closed — so
every downstream verifier is unreachable *through the gate*, and the only way to
exercise them is to call them directly. Almost nothing did. The codebase had
already met this once: `every_evidence_binder_rejects_a_claim_its_artifact_contradicts`
exists precisely because "the qualified gate's `qualification_state` check fires
first and they were never reached" — but it covers a handful of binders, not the
surface.

This is the highest-stakes instance of the session's recurring species. These
are the checks the lane relies on **after** the flip sets the state to
`complete`. Until now, nothing established that any of them does anything at
all, and a lane that passed would have proved nothing about them.

**Discipline for closing it:** a passing test is not evidence the mutant dies.
Each function is closed by writing reject cases *and* re-running
`cargo mutants --re <fn>` to watch the survivor turn caught. Killed so far, all
verified that way:

| function | mutants | result |
|---|---|---|
| `require_hex_digest` | 1 | caught |
| `require_git_object_id` | 1 | caught |
| `verify_requirement_sources` | 1 | caught |
| `verify_oracle_source_coordinate` | 1 | caught |
| `verify_review_attack_classes` + `verify_cross_pass_reproduction` | 17 | 12 caught, 5 timeout, **0 missed** |

The five timeouts are recorded as timeouts, not claimed as kills.

### Measured result

A second full campaign, run after the four batches, against the same file:

| | before | after |
|---|---|---|
| mutants | 1062 | 1087 |
| caught | 728 | **785** |
| **missed** | **310** | **280** |
| unviable | 21 | 21 |
| timeouts | 3 | 1 |
| kill rate | 69.9% | **73.6%** |

Of the 15 functions targeted, **13 came back with zero surviving mutants**, and
every one of the 15 wholesale-`Ok(())` mutants died. Functions surviving
wholesale replacement fell from 39 to 16.

**Two survivors remained, and both were my tests passing for the wrong reason —
the very species this finding is about.**

1. `verify_locked_corpus_has_no_aliases` kept a live `||`, and it was not the
   one I assumed. The condition that survived was
   `name == ".git" || name == "target"` — the walk's skip list. My fixture
   contained neither directory, so breaking the skip changed nothing
   observable. Fixed by giving the fixture a `.git/` and a `target/`, each
   holding a link into the corpus, and asserting both are still accepted. Now
   fully clean.

2. `verify_mutant_concurrence` kept a live `==`, and again not the one I
   assumed. My first fix asserted only `is_err()`; the packet's reviews are
   `planned`, so inverting `review.result == "pass"` still failed — further
   down and for an unrelated reason — and a reject test that does not name its
   reason accepts any refusal. Tightened to assert the message. That killed the
   comparison I had aimed at, and exposed a second one beneath it.

**Deferred to HAQP-1b (M24), with reason:** the residual survivor is
`attempt.id == *attempt_id` at `haq.rs:2730`, inside the concurrence walk. That
code runs only when a packet declares an `equivalent`/`duplicate` disposition
**and** carries passing committed review records. Dispositions belong to the
mutation stage, which ADR-0021 defers to M24, so the state that reaches this
line does not exist in a 1a packet. Killing it means authoring a full 1b
fixture, which is the same work as HAQP-1b itself.

The general lesson is sharper than "write reject cases". **Twice I predicted
which operator a test would kill and was wrong both times.** The campaign, not
the reasoning, found the live one. A reject case is a hypothesis about which
branch matters; only the mutation run tests the hypothesis.

## F-37 — the pre-flight: five lane-killers, all at the end of four hours

Brian cleared a pre-flight before launching the lane again. Method: a throwaway
`git worktree` at HEAD, flip the packet against the committed evidence, make the
metadata child, run the qualified gate. Minutes instead of four hours, and main
never touched.

It found five guaranteed failures. Every one of them fires only at the END of
the lane, after the 3.5-hour fuzz campaign.

**1. The flip crashed outright.** `{row["family"] for row in load("generated.json")}`
iterated a dict and got its KEYS, so `row["family"]` raised
`TypeError: string indices must be integers`. `generated.json` is
`{schema_version, artifact_blake3, rows}`. `crash.json` and `mutants.json` are
dicts too, but the flip indexes those directly, so they were never wrong.

**2. The flip never copied `negatives`.** It copies `seed`, `evidence_hash`,
`accepted`, `attempts` and `discards`, so the packet kept a stale value and
`verify_generated_rows` refused: *accepted 100000 + discards 0 + negatives 45569
!= attempts 145876*. Four of five families matched by luck.

**3. The flip never wrote the review markdown at all** — it only printed a
`git add` line naming it. The qualified gate cross-checks EIGHT things between
packet and surface (state marker, status tuple, `fixed_review_base`, `status`
header, packet digest, all 33 canary rows, the conclusion, and an explicit
refusal of leftover placeholders). The file carried **136 `NOT_RUN` cells**. The
flip's entire job is to make packet and surface agree, and it did the packet
only.

**4. The lane never verified its own result.** `haqp_qualify.sh` ended at the
flip. Every gate downstream of `qualification_state == "complete"` is
unreachable until that moment, so a dozen verifiers ran for the first time after
the lane had already reported success — surfacing later at the un-ignored gate
test, with the flipped packet committed.

**5. The canary section described a method that is not used.** It claimed
*"Execution occurs outside working tree and restores fixed review base after
each attempt"* and carried columns for an isolated command, a base-restored flag
and a per-canary raw hash. `run_canary_suite` mutates an in-memory CLONE of the
parsed packet. Nothing is ever written to the tree, so there is no base to
restore and no such evidence exists. The in-memory approach is *stronger* — a
clone cannot dirty the tree even on a crash — but three columns existed for
evidence nothing produces. Same species as F-28.

### What the renderer does, and the property that makes it safe

The flip now renders the surface from the artifacts: canary rows, generated and
fuzz rows, crash boundaries, both review passes, the falsification-attempt
table, residual risks, the raw-artifact manifest, and the campaign-conclusion
verdicts. The lane records a per-stage manifest (`evidence/lanes.json`:
command, exit code, elapsed, artifact digest) because ADR-0020 asks the report
for per-command exit statuses and artifact hashes that **no lane recorded** —
those cells could previously only have been filled by inventing values.

`assert_rendered` then refuses to hand back a document that still carries a
placeholder, naming every one. That is the load-bearing part: a cell with no
evidence behind it stops the flip in seconds instead of being filled with
something plausible. It fired repeatedly while the renderer was being written,
and each time it was pointing at a real gap.

Three more template/artifact mismatches surfaced that way, each the same
species as finding 5 — the report asking for something nobody records:

- the metamorphic table hard-coded ten relation columns
  (`Canonical reparse`, `Inverse/undo`, …) while the generator records a LIST of
  `{relation, oracle_id, result, artifact_blake3}` whose names differ per
  family. Every cell could only have been filled by mapping a recorded result
  onto a column it did not belong to. Header corrected to the artifact's shape.
- the Phase 1 test and coverage tables have no result to report at all: AM-17.2
  keeps those gates `#[ignore]`d. `NOT_RUN` reads as "the lane forgot"; they now
  render `QUARANTINED (AM-17.2)`, which is the honest third value.
- `packet.requirements` has no `stateful` or normative-hash key (serde defaults
  them), and `tests` use `name`, not `test`.

### What the pre-flight proved, and what it could not

**Proved:** the flip runs to completion; the packet and the full markdown
surface are rendered consistently and **accepted by the qualified gate**, which
previously refused at the very first markdown check.

**Could not prove:** the per-artifact provenance binders (sanitizer proof, fuzz
rows, corpus scope). The committed evidence was produced at older commits, and
each binder checks its artifact's own `source_commit`/`source_tree` against the
base. Restamping them further would only be testing my own simulation. Those
bind for real only when a lane regenerates evidence at the fixed base — which is
exactly what the next launch does.

One note on method: the digest the markdown must carry is computed by shelling
out to a new `haq packet-digest` subcommand rather than reimplemented in Python.
The gate hashes `serde_json::to_vec(packet)`, which serializes in STRUCT field
order, not the order the keys occupy in the file. A reimplementation would have
agreed until someone reordered a field, then disagreed silently.

## F-38 — a disclosed defect is still a defect: the blind reviewer refused F-33

The 2026-09-02 lane cleared every evidence stage and refused at the flip because
pass 1 (gpt-5.6-sol, 12 attempts) returned **6 verified defects**. All six were
one finding, and it was **F-33** — which this ledger had already recorded, which
ADR-0021 disclosed, and which `packet.residual_risks` carried as RISK-003.

The reviewer named six coordinates: `merge.rs:122` anchors
`pub fn overlap_slots(`, `store/mod.rs:370` anchors `pub fn relations(`,
`basis.rs:23` anchors a struct field. The packet declared "apply
predicate-inversion here" at lines where no operator can apply.

**Disclosure is not resolution.** A review finding clears only when a commit
changes the code at its coordinate — `verify_resolution_coordinate_changed`
requires the diff. There is no "resolved by an accepted residual risk" path, and
there should not be: one would let any finding be waved away by writing a risk
entry. So the disclosure that felt like diligence in August was, at the gate,
worth exactly nothing. §6 refused, correctly.

That produced a second F-28-shaped deadlock — 1a's §6 demanding a fix 1a had
deferred — and the way out was to notice that **re-anchoring is 1a work**.
ADR-0021 defers *evaluating* mutants, not *declaring them truthfully*. Fixing an
anchor makes a declaration true; it claims no kill.

**Ruling (Brian, 2026-09-03):** take the real fix, always. *"We don't accept tech
debt or improper bandaid fixes."* The alternative on the table — adding a
resolved-by-disclosure path to the review schema — was rejected as exactly the
bandaid that rule forbids.

### Result

| | before | after |
|---|---|---|
| mutants whose operator cannot apply at its anchor | 52 | **0** |
| operators assigned to a family whose code cannot exhibit them | 17 | **0** |
| duplicate anchors | — | **0** |
| heaviest operator share (§3 caps at 25%) | 25.0% | **12.3%** |

The 17 operator corrections are F-06 in the specific: the original 5×13 grid
assigned every operator to every family regardless of behaviour, so
`disabled-crash-point` landed on a family with no crash points and
`oracle-short-circuit` on one with no oracle comparison. Those could not be
re-anchored at all — no line in the family supports them — so the operator, not
the line, was the wrong declaration.

### Two false starts worth keeping

**The first re-anchor was itself the bug under repair.** It matched
`predicate-inversion` against `pub value: Option<serde_json::Value>,` — reading
a generic's angle brackets as comparison operators — and
`ordering-nondeterminism` against `pub nodes: BTreeMap<NodeId, Node>,` because
the TYPE NAME contains "BTree". It would have replaced "anchored to a function
declaration" with "anchored to a struct field that superficially matches a
string", which is the same defect wearing a different hat. Caught by spot-checking
the output rather than trusting the count.

**The second was looser than the verifier.** It accepted a bare `.iter()` for
`ordering-nondeterminism` and `matches!(` for `predicate-deletion` — but
`verify_mutant_operator_patch` requires the before-text to contain `sort` or
`BTree`, and a macro's `!` is part of its name, not a negation. Declaring those
anchors would have passed the packet gate and then been refused when a patch was
checked. The applicability test is now derived from the verifier's own contracts
rather than from an approximation of them.

The lesson is the session's, again: **the check must be the one that will
actually judge you.** Twice the shortcut was to write a plausible test of my own
instead of reading what the gate requires.

### What replaced the canary

`the_committed_mutation_plan_is_still_inapplicable` asserted exactly 36
declaration-anchored mutants — a canary that pinned a *defect* so it could not be
silently rediscovered. With the defect fixed the premise is gone, so it is now
`every_declared_mutant_anchors_a_line_its_operator_can_mutate`, which pins the
repaired property and the §3 concentration cap. A refactor that moves a line
turns a live anchor back into a signature, and nothing else would notice.

## F-39 — the corpus-scope subsystem: never run, and unrunnable as written

Fixing F-38's registry binding moved the 2026-09-03 lane's refusal one gate down
to `verify_corpus_access_audit`, where three things were waiting.

**1. It had never produced a row.** `scope_traces` in the committed audit was
empty for its whole life. `verify_corpus_scope_presence` requires exactly eight
scopes, so any lane that reached it would refuse — and none had. The producer
(`haq scope-probe`) existed but nothing in the lane or the justfile invoked it:
the same species as `concurrency.json` (F-37), one gate later.

**2. "Replay" meant re-executing the campaign inside the gate.**
`verify_corpus_scope_replays` created a worktree and ran each scope's full lane
command under strace — `just ci`, `haq generate --cases 100000`,
`cargo test --workspace`, `just haq-blind-review`, and
`scripts/haqp_fuzz_campaign.sh 1800`. Populating `scope_traces`, which (1)
requires, would have made every `haq-verify` a second complete campaign with a
second fuzz run and a second pair of paid reviews. It had never fired only
because the loop body had never had a row to iterate.

**3. Six of the eight scopes could not satisfy it.** `verify_trace_corpus_resolution`
collected only paths under `fuzz/corpus/` and then refused if it had collected
none. Only the `fuzz` scope reliably opens that corpus; `just ci`,
`haq-canaries`, `haq-generated`, `haq-crash`, `haq-mutants` and
`haq-blind-review` never do. A mandatory check that six of eight subjects cannot
pass is not a check, it is a guarantee of refusal.

**Rulings (Brian, 2026-09-04).** Capture during the lane and verify the recorded
bytes; no re-execution. Keep the forbidden-path refusal unconditional for all
eight scopes — that is the security property — and require corpus *resolution*
only of `fuzz`, the one scope for which an empty corpus set is a defect.

**What changed.** Every lane stage now runs under strace exactly once and
`scripts/haqp_scope_row.py` emits its row with digests asked of the xtask, so
the row is judged by the code that wrote it. `ci` and `replay` are lane stages
for the first time. `scope_lane_command` names the commands the lane actually
runs, verbatim, because the gate compares strings. Traces are stored `.zst`.
`CorpusAccess::{Required, IfPresent}` carries the ruling into the checker.

Four tests, built entirely in scratch: a well-formed row accepted and nine
single-field doctorings each refused for its stated reason; a held-out open
refused for a non-fuzz scope; resolution required of `fuzz` alone; presence
exactly eight. Before this the three functions survived `Ok(())` because no test
had ever constructed a row.

## F-40 — the crash fault matrix was not reproducible, so its replay gate could never pass

Closing `verify_crash_replay` (F-36) meant running it honestly: an independent
`crash-evidence` run must equal the committed record. It did not — and two
consecutive runs did not equal **each other**. Every boundary's
`first_effect_digest` and `first_recovery_digest` changed per run, while within
one run `first == second` held. §5's "second recovery identical" was true; §1's
"reproducible from the tree" was false; and the equality `verify_crash_replay`
enforces was unsatisfiable by construction. It had never been noticed because
the check had never run (F-36).

**Cause.** `recover()` hashed `crate::digest::world_digest` — the RAW world
digest, `intent:<uuid>:<json>` with `StepAck.at` wall-clock timestamps and
minted ids inside — and `effect_digest` is built from it. The codebase already
had `normalized_world_digest` (M04 Algorithm F: uuids → first-appearance
ordinals, clock fields dropped), written so that "two workspaces that differ
only by minted ids and clocks produce the SAME normalized digest". Recovery
simply did not use it. The authors had excluded `TransactionId` from the basis
digest for exactly this reason two lines above.

**Fix.** Recovery digests use the normalized world digest. Two runs now agree on
`registered`, `exercised`, `scenarios` and every boundary. `recover()` was the
raw digest's only caller, so nothing else moved. A first attempt normalized the
terminal intent states instead; measured, it changed nothing (the timestamps
enter through the intent aux, not the terminals) and was reverted rather than
kept as harmless.

`crash.json` is regenerated so the committed record matches what the fixed
binary produces; the new test replays it for real (~12 s) and also refuses a
packet declaring a boundary the replay never exercised.

**Correction (2026-09-05).** `146d8d4` claimed that regeneration but did not
ship it: the committed `crash.json` still carried the raw-digest rows and the
pre-zstd `lockfile_blake3` from `80f81e5`. It went unnoticed because every CI
run after `80f81e5` was killed or starved before the replay test completed;
the first run that finished refused with *"crash replay lockfile_blake3:
expected a73bf…, got fdae1…"*. The regeneration lands in the follow-up commit
(two consecutive runs are byte-identical), and the `haq::tests::crash_*` tests
join the serialized `crash` nextest group: they run the full fault matrix
through the binary and were writing one shared `target/haqp/crash-replay.json`
concurrently with each other and with the matrix group.

## F-41 — the 1a mutant stage's rows were refused by the 1a gate

`haq mutants` records every mutant as `not-ready` at stage 1a — "recorded, not
claimed", exactly as the lane comment says and as ADR-0021 requires.
`verify_mutant_evidence_row`, written for 1b's evaluated rows, refused the very
first one: *"predeclared mutant P1-M001 has evaluated evidence."* A not-ready
row is the absence of a claim, and the verifier had no notion of one. Never
noticed: it is one of the F-36 dead gates.

Now a not-ready row is accepted only for a predeclared mutant, may carry no
results, and counts toward nothing; every declared mutant must have a row; the
evaluated set must equal the non-predeclared dispositions. Four refusals pin it:
a predeclared mutant with an evaluated status, a not-ready row carrying exit
codes, a `killed` disposition backed by a not-ready row, and a duplicate.

Also found writing the wrapper's test: my first accept case *hedged* — it
accepted an `Err` so long as the message mentioned the lockfile, because the
committed evidence predates this `Cargo.lock`. That is the passes-for-the-
wrong-reason species in the very test meant to close it. The fixture now binds
its evidence to its own scratch lockfile so the accept is strict, and lockfile
drift is a separately-asserted refusal.

## F-42 — the sanitizer-build replay could never match, because cargo hashes the build path

`verify_sanitizer_build_replay` rebuilds each fuzz target in a scratch worktree
and requires the digest to equal the committed binary's. It had never run
(F-36). Run honestly, it cannot pass: the same commit built at two paths yields
two binaries.

**What differs, measured rather than assumed.** Not source paths — with
`--remap-path-prefix`, `-Zremap-cwd-prefix` and `trim-paths = "all"` the two
binaries still differed in 5.9 MB of `.debug_*`/`.strtab`, and after stripping
those, in exactly 112 bytes: 111 in `.rodata` and one in `.text`. Those bytes
are the codegen-unit name, `cst_parse.db8c568e48c1f5ae-cgu.0` versus
`cst_parse.6be4151ba0cf3c2-cgu.0`. cargo derives `-C metadata` from the package
id, which for a path dependency includes the manifest's **absolute path**; ASan
embeds the resulting CGU name in `.rodata`; the `.text` byte is its length.
Remapping cannot reach it because it is not a path string, it is a hash of one.

**Proven fix.** Two clean worktree builds at one fixed path,
`/var/tmp/liminal-haqp-build/<commit>`, are byte-identical
(`424194da…` twice). The campaign now builds every sanitizer binary in a
worktree at that canonical, commit-keyed path — persistent disk, older commits
pruned — and runs the evidence copy directly; the replay rebuilds at the same
path with the same flags. The path and flags are a closed registry in the
verifier (`sanitizer_build_canon`); the proof records what the campaign used and
the replay refuses any that differ, so the two sides cannot drift apart.

**Cost of the honest answer.** The first replay after a fresh clone performs a
cold ASan build per target (~30 s each here); later replays hit the cached
worktree. The committed proofs must be regenerated by the next campaign — the
current ones were built in-tree and will not match, which is the correct
verdict on them.

**A second defect inside the same verifier.** Once the digest matched, the
runtime probe refused: it ran `-help=1` and required the text to contain
`-fsanitize=address`. libFuzzer's help is a usage banner and never names the
sanitizer — the campaign's own committed probe logs contain that string **zero**
times — so the probe could not pass for any binary that has ever existed. It
now asks the runtime to identify itself: `ASAN_OPTIONS=help=1 <bin> -runs=0
-seed=1` makes the linked AddressSanitizer print its flag table, which cannot
happen without the instrumentation. The campaign records the same probe, so the
proof and the replay describe one witness. The map from sanitizer to runtime
name is closed; an unknown sanitizer is refused rather than guessed.

Rejected on the way: comparing stripped binaries with the CGU name masked
(guessing which bytes are non-semantic is exactly how a check stops meaning what
it says), and dropping the digest equality in favour of the runtime probe alone
(which would have turned "this binary is reproducible from this commit" into "a
binary with a sanitizer exists").

### A fixture that names a count must read it

Adding canary C33 turned `markdown_surface_rejects_a_stale_qualification_state`
red, and the reason is more interesting than the fix. The test doctored its
fixture with a hardcoded `markdown.replace("Target: exactly **32 predeclared
canaries**", ...)`. With 33 canaries the replace matched nothing, so the
"doctored" markdown was byte-identical to the real one and the test was no longer
testing the canary-target check at all.

It only went red because an *earlier* assertion in the same test happened to
fail. Had that assertion not been there, the test would have gone on passing
while exercising nothing — a green test whose name still described the check it
had stopped performing. That is the session's recurring species (F-11, F-31,
P2-F05) reached from a new direction: not a check that never worked, but a
working check quietly detached from its fixture by an unrelated edit.

Fixed by deriving the count from `packet.canaries.len()` and asserting the
doctored text actually differs from the original, so a no-op mutation fails
loudly instead of passing silently.

### Also noted, not fixed

- `fuzz/fuzz_targets` is enumerated from the FILESYSTEM, so an untracked scratch
  `.rs` there would demand coverage and break the lane. F-32's class, milder
  because the directory is tracked.
- Review-resolution evidence is matched with `contains` on three strings, so any
  file mentioning all three satisfies it — the findings ledger would.

### A correction to my own reporting

I reported this audit three times before getting it right, and the first two
figures are both in the git history:

| reported | actually | why it was wrong |
|---|---|---|
| "10 survivors, 3 in `verify_qualified_repo`" | — | an unfinished run, read through `head` |
| "74 survivors across 1062 mutants" | — | a *different* unfinished run, whose log I then deleted with `rm -f` during commit cleanup while it was still being written |
| **310 survivors across 1062 mutants** | **the completed run** | 57m, `-j 6`, recorded at `docs/execution/m17-5-verifier-campaign.log` |

The completed campaign: **728 caught, 310 missed, 21 unviable, 3 timeouts** — a
**69.9% kill rate** on the file that judges every other piece of qualification
evidence. 131 of the 310 sit in mutation machinery deferred to 1b
(`verify_mutant_operator_patch` alone has 57); **179 are in 1a-critical paths**,
led by `ordered_block_contents` (13), `verify_recovery_pairs` (11),
`generated_category_class` (10), `integer_delta` (9),
`verify_locked_corpus_has_no_aliases` (8).

The 74 figure was low by 4×, and I had committed it. The lesson is not "read the
whole file" — I knew that after the first miss. It is that **a measurement
written only to an untracked path is not a measurement**, which is the same
finding as F-29 and F-32, arrived at a third time by destroying the evidence
myself during cleanup of the very finding that says so. The campaign log is now
a tracked artifact so the number has a source that outlives the session.

And one test I wrote here was wrong: I asserted `verify_recovery_proof_presence`
should reject a boundary with zero recovery pairs. It should not — it iterates
pairs and checks their digests, and `verify_recovery_pairs` owns the count.
Corrected to pin both functions against their own contracts rather than
"fixing" one to cover the other.

## F-43 — the lane's scope tracer and the sanitizer runtime cannot share a process

**Found.** 2026-09-05, first lane at `a42fc8f`, the first one whose stages ran
under the F-39 scope tracer. The `ci` stage refused with
*"replayed sanitizer runtime probe failed for cst_parse"*: the verifier's own
ASan replay built at the canonical path, then its probe exited 1. The same probe
exits 0 by hand, and the stage's strace shows the probe process exec, spawn one
helper, and exit 1 with no signal.

**Cause.** LeakSanitizer stops the world with `ptrace` at exit, and a process
can have one tracer. Under `strace -f` the runtime says so itself:
`LeakSanitizer does not work under ptrace (strace, gdb, etc)` — reproduced with
the probe (exit 1; exit 0 with `detect_leaks=0`) and with a real three-input
fuzz run, which finishes its inputs and then dies at the exit-time leak check.
The campaign already rules on exactly this for its own traced runs
(`haqp_fuzz_campaign.sh`: "Keep ASan memory checks enabled while disabling
only leak detection for traced runs") and records `ASAN_OPTIONS=detect_leaks=0`
in the declared command; ADR-0020 §4 lists memory errors, not leaks, as failing
results, and a leak is not unsoundness in Rust. The probe — written when only
the campaign's inner strace existed, which never wrapped it — did not inherit
that policy, and F-39 put every stage under a tracer.

**Fix (probe).** The probe runs under `<SAN>_OPTIONS=help=1:detect_leaks=0`
on both sides (`probe_sanitizer_runtime` and the campaign's probe); it runs
zero inputs, so leak detection had nothing to observe there. The refusal now
carries the runtime's exit status and last stderr line: the first lane said
only "probe failed" and hid the sentence above.

**Fix (fuzz stage).** The same one-tracer rule broke the fuzz stage itself:
the stage wrapper's `strace -f` and the campaign's per-target `strace -f`
cannot both attach — nested strace fails outright with
`PTRACE_TRACEME: Operation not permitted` — which would have left every
per-target corpus audit empty. The stage tracer is now the only tracer. Under
the lane (`HAQP_STAGE_TRACE` set by the stage wrapper) the campaign runs each
binary untraced by itself and, once the binary's exit line is present, carves
its lines out of the stage trace by pid into the target's own trace
(`carve_stage_trace`); libFuzzer runs single-threaded here and LeakSanitizer's
tracer thread is off, so a binary's pid is its whole subtree. The row script
removes those pids' lines from the stage trace and stores the remainder; the
fuzz scope row names the carved traces as `parts`, bound to the campaign's
per-target rows exactly (none dropped, none twice, no other scope carves), and
the gate judges the UNION: observed paths, corpus resolution and the
locked-corpus refusal are computed over remainder plus parts. Every digest is
set-based, so the partition changes nothing the gate judges, and the scan
streams each file line by line through zstd — a `just ci` trace is a gigabyte
raw and the fuzz stage's is larger. Proved on a 5-second campaign under a real
stage tracer: all seven targets complete, and each target's carved trace
equals its pid's lines in the stage trace line for line.

## F-44 — whole-stage traces show the suite reading the locked corpus by design

**Found.** 2026-09-05, the first union digest over a real fuzz-stage trace and
the first complete `ci` stage trace, both refused by the locked-corpus rule.

**What the traces show.** The refusal fires on any trace line carrying the
fragments `heldout` or `conformance/corpora`, unconditionally for all eight
scopes (ruling 2026-09-04, reaffirmed 2026-09-05). Under whole-stage tracing
(F-39) that fragment appears in lines no oracle produced:

| stage | who | what | lines |
|---|---|---|---|
| `ci` | the SLO graduation test (`external_file_profile_graduates_on_heldout_corpus`) | `openat(O_RDONLY)` of `heldout/v1/MANIFEST.b3` and held-out traces — the profile is *measured* on the held-out corpus, which is the point of holding it out | 137 |
| `ci` | `git` (index refresh) and the corpus-alias test walking the directory | `statx`/`newfstatat`/`openat(O_DIRECTORY)` | ~1,300 |
| `fuzz` (remainder) | `git worktree add` at the canonical build root, and the prune of the previous commit's tree | `openat(O_CREAT)`/`unlink`/`rmdir` of a *copy* under `/var/tmp/liminal-haqp-build/<commit>/conformance/corpora` | 696 |
| `mutation` | `cargo-mutants` (`copy_vcs = true`) | copies the whole tree, held-out files included, into its scratch build | not yet traced |

The carved fuzz binaries' traces contain none of these: the rule as applied to
the campaign's own processes holds exactly as before.

**Why this is a ruling.** The rule was written when a scope trace meant the
campaign's tools; applied to whole stages it refuses the suite's own design
(`ci` cannot pass while the graduation test exists) and ordinary tooling
(`mutation` cannot pass while cargo-mutants copies the tree). Narrowing it is a
change to qualification semantics — Protocol §3 — and reverses an explicit
instruction. The candidate rulings, for the record:

1. **Repo-tree writes are the unconditional rule; the fuzz binaries keep the
   full rule.** No stage may modify the repository's locked corpus (write-class
   syscalls on paths resolving under `<root>/conformance/corpora`: the "never
   hash-update" half of the lock), and the carved fuzz traces may not carry the
   fragment at all (the "never read or tune against" half, for the one process
   family that produces corpus evidence). Reads by the graduation test are by
   design and documented; copies under build roots are outside the repository.
2. Keep the literal rule and change the suite: move the graduation test out of
   `just ci`, sparse-checkout `conformance/corpora` out of canonical worktrees,
   run cargo-mutants in place. Ordinary `git` index refreshes still stat the
   paths, so this alone does not make `ci` pass.
3. Keep the literal rule as written: `ci`, `fuzz` and `mutation` can never
   qualify. Recorded so the option is visibly rejected, not overlooked.

Sparse-checkout of `conformance/corpora` in canonical worktrees is worth doing
under any ruling: the sanitizer build does not reference it, and a build root
that never materializes the locked corpus has nothing to prune.

**Ruling (Brian, 2026-09-05): option 1.** Writes are the unconditional rule
for every stage; the carved fuzz binaries keep the full rule; canonical
worktrees are sparse.

**Fix.** The trace scan classifies every file syscall as read- or write-class
(`scope_trace_line_accesses`: creat/truncate/unlink/rename/mkdir/rmdir/link/
symlink/chmod/chown/mknod and the open family with a writing flag; the target
of a two-path syscall counts as written, because a link INTO the locked corpus
is the second-name attack). For a stage's trace, write-class paths that name
the corpus are resolved against the repository — through aliases, relative
paths judged root-relative — and any that lands under `conformance/corpora`
refuses (*"wrote to the locked corpus"*); reads and stats are permitted.
Corpus resolution over a stage's union runs with the `WritesOnly` policy; the
per-target rows and the carved parts keep `AnyTouch`, exactly as before.
Canonical worktrees (campaign and verifier replay alike) are created with
`--no-checkout` and a sparse-checkout that excludes `conformance/corpora`, and
refuse if it materializes anyway: the sanitizer build does not reference the
corpus, so the build root now has nothing of the locked data to write, prune or
alias. Tests: a stage's read and stat of a held-out file are accepted, as is a
write to a copy under a build root; an open for writing, an unlink, a rename
into the corpus, a relative write and a write through a symlink alias each
refuse for the stated reason; a carved part that opens a held-out file still
refuses on any touch (union test). The relative-path guard follows the same
split: a fuzz binary's relative corpus path is still the campaign's defect
(`AnyTouch` refuses), while a stage helper's relative seed read — git's index
refresh, the campaign's own seed manifest — is unresolvable without a cwd
receipt and contributes nothing (`WritesOnly` skips it); the campaign's own
corpus reads are absolute now regardless. Proved on a traced 5-second campaign:
the fuzz scope row is produced with all seven parts, a computed resolved
digest, and a 1.6 MB remainder carrying no fuzz-binary line.

**Second lane (c437aa3).** `ci` through `fuzz` passed with scope rows — the
first lane to produce any — and the seven sanitizer targets ran their full 30
minutes each under the stage tracer with zero artifacts (12.8 M executions for
`cst_parse`, 70.5 M for `graph_interchange_codec`). `reviews` refused:
*"traced repository path escapes repository root"*. The reviewer tooling
(codex CLI, node, GPU probes) opens paths relative to a directory fd —
`../nvidia0` under `/dev`, `../lib/node_modules/...` — and the resolver read
every relative path as repository-relative, so `..` climbed out of the root.
Under the stage policy a relative path is relative to a cwd or dirfd the trace
does not receipt and can add no corpus entry, so it is skipped rather than
refused; a fuzz binary's trace keeps the strict reading (test: a dirfd-relative
open is accepted for a stage and still refused for a binary).

## F-45 — the first complete lane: eight verified defects from blind pass 1

**Found.** 2026-09-05, lane at `0d8c32a`: every stage produced its scope row,
the seven sanitizer targets ran 30 minutes each with zero artifacts, both
blinded reviews ran, and the flip refused — *"pass1-codex-gpt-5.6-sol.json
reports 8 unresolved verified findings; ADR-0020 §6 requires zero"*. Pass 2
(mimo-v2.5-pro) reported none; §6 lets one pass block alone (F-33). The
records are preserved under `docs/execution/reviews/2026-09-05-lane-0d8c32a/`.
Each finding was verified at its line before anything moved; all eight held.

| attempt | class | what was true | fix |
|---|---|---|---|
| A01 | vacuity | C26 stripped provenance from a packet that had none, then "caught" the baseline's own refusal | C26 builds a scratch repository (fixed base, then a metadata child with a bound block), requires `verify_provenance` to ACCEPT it, then strips it |
| A02 | shared-oracle coupling | `Phase1Document`'s `PartialEq` compared holder and derived graph only; HIR content drift on reparse passed the round-trip oracle | equality also compares the HIR with source positions removed (`range`, `source_map`, `basis`), since reformatting moves every offset. Diagnostics stay out, on evidence: regression fixture `f09-070ba65d68c9.bin` carries UnknownForm/MalformedSyntax on the original parse and none on the reparse, with identical HIR and graph — canonical emission drops what it diagnosed, by design |
| A03 | missing negatives | the compact-paragraph predicate tested `first.starts_with("```")` on the untrimmed line while every other check used the trimmed one; an indented fence bypassed the refusal | trimmed line; negative test |
| A04 | weak mutants | ADR-0020 §3 names *broadened allow-list*; the inventory had none (F-38 re-anchored the two that were) | P1-M013 re-anchored to `is_compact_id`'s byte allow-list (`matches!(byte, b'_' \| b'-')` gains `\| _`), killed by P1-T17/P1-T01; the operator arm admits an added wildcard alternative |
| A05 | fault omissions | `run_concurrency_repo` trusted `concurrent_code = not_applicable` from the packet | the generator scans non-test implementation sources: concurrent execution (spawned threads, async runtimes) refuses the label; synchronization primitives are recorded and the gate recomputes them (two `Mutex<` sites today) |
| A08 | exception broadening | `Packet` and its rows accepted undeclared fields, which typed reserialization dropped from the digest | `deny_unknown_fields` on the packet and every row type; test |
| A10 | weak mutants | the skipped-durable-transition arm knew `.commit(` but not `.append(`, so P1-M022 was unevaluable | call markers |
| A12 | weak mutants | same for `.commit_if(` and P1-M035 | call markers |

Rejected by the reviewer's own verification and left alone: A06 (a random
transaction changes Basis identity but never emitted bytes), A07, A09, A11
(caught by the gate as designed).

**Consequences.** The packet digest and the canary evidence rebound; the
committed concurrency record now carries the primitives it must match. Every
fix is source or gate code, so the lane reruns from a new fixed base.

## F-46 — the second complete lane: six verified defects, and a reviewer answer that killed the lane

**Found.** 2026-09-06, lane at `612cbcc`. Every stage passed with scope rows
and seven clean campaigns again. Pass 1 (codex gpt-5.6-sol) reported six
verified findings; pass 2 (mimo-v2.5-pro) answered off-schema — one attempt's
`observed_result` did not quote its target string — and the lane died there,
two hours in, with no record. Records preserved under
`docs/execution/reviews/2026-09-06-lane-612cbcc/`. All six verified at their
lines; all six held.

| attempt | class | what was true | fix |
|---|---|---|---|
| A02 | shared-oracle coupling | a pure move kept only the marker SET: swapping two moved markers' contents passed the identity oracle | a move keeps every marker's content (`ordered_block_contents` compared per marker); test |
| A03 | missing negatives | transform negatives `invalid-span`, `truncated`, `hostile` were ordinary label strings | each category constructs the shape it names (unterminated and spaced markers; a block cut mid-marker; NUL, bidi override, nested and over-long markers); the oracle is totality plus token survival, and a non-disjoint outcome is recorded as the refusal it is |
| A05 | fault omissions | the durable-transition census knew `.commit(` and `.commit_if(`, not the store's `inner.log.append(` | `.log.append(` is a marker; the store's single checksummed append is disclosed as deferred under AM-17.8 |
| A06 | nondeterminism | the concurrency scan (F-45) stopped at the first `#[cfg(test)]` line, so production code after an earlier test module escaped | the attributed item is skipped as a brace-delimited block and scanning resumes; test |
| A07 | corpus leakage | every accepted interchange category built one `Node` with the category name in its payload — and a `Node` is not the codec's unit; the fuzz target decodes a `Transaction` | each category builds the transaction it names (`single-edge` two nodes one relation, `dag` a diamond, `wide` 1+8, `deep` a chain of 12, `large-payload` 64 KiB) and an independent JSON count certifies the shape; goldens re-recorded |
| A10 | missing negatives | the seed manifest counted sixteen and hashed them; `graph_interchange_codec` and `ilrp_recovery` carried sixteen hash-named inputs of no declared class | every corpus must declare, by seed name through a closed token table, at least one valid, boundary, truncated, malformed and hostile seed; both corpora replaced by classed sets written by `haq seed-corpus` from the same constructors |

**Lane defect.** A reviewer that answers off-schema is retried with the
identical prompt in a fresh isolated session, at most three times; every
off-schema answer is kept (redacted) beside the record and the record carries
`schema_retries`. Provider failures are not retried: an outage is not a
malformed review. Self-tested.

**Consequences.** Packet (scope disclosure), digests, canaries and the two
corpora changed; the campaign reruns from a new fixed base.

## F-47 — the third complete lane: nine verified defects

**Found.** 2026-09-06, lane at `f360e90`. Every stage passed; pass 2 (mimo)
answered off-schema twice and the F-46 retry recovered it (`schema_retries: 2`,
result pass); pass 1 (codex gpt-5.6-sol) reported nine verified findings.
Records under `docs/execution/reviews/2026-09-06-lane-f360e90/`. All nine
verified at their lines; all nine held.

| attempt | class | what was true | fix |
|---|---|---|---|
| A01 | vacuity | every repair seed category built `WriteFile` steps with the category's name in their contents | each category builds what it names: `graph-step` graph operations, `two-step` two steps and one edge, `duplicate-ack` a dependency acknowledged twice, `poststate` a content-hash predicate, `hostile` NUL/bidi/64 KiB contents on a traversal path, `boundary` six steps; `missing-step`, `truncated-intent` and `cycle` are refused by `topo_order` itself (`UnknownStep`, `Cycle`) and recorded as negatives, and a refusal for the wrong reason is a generator error |
| A02 | shared-oracle coupling | the interchange oracle counted two operation kinds; a renamed published field round-trips through the same serde on both sides | the encoded JSON's key sets for the transaction, node and relation are pinned to the published schema |
| A03 | missing negatives | the ordering oracle accepted equal lengths; a duplicated step stood in for a missing one | the order is exactly the declared steps, once each |
| A04 | weak mutants | P1-M033 was anchored on a `pub use` re-export; the mutability scan excluded `use` but not `pub use` | re-exports are declarations; P1-M033 re-anchored to `view.rs:38`, the content-hash staleness check |
| A05 | fault omissions | the durable census knew call markers only; a direct `fs::rename` or `sync_all` escaped | `.sync_all(`, `.sync_data(` and `fs::rename(` are markers; the census grew by `file.rs` 3→7, `store/log.rs` 4, `store/snapshot.rs` 2, `daemon/crash.rs` 1, each disclosed as deferred with its AM-17.8 reason |
| A06 | nondeterminism | `use std::thread as th; th::spawn(..)` named no listed token | spawn and scope are matched by call shape (`::spawn(`, `.spawn(`, `::scope(`); the product crates spawn no processes, so the over-match is a named refusal |
| A07 | corpus leakage | `openat(AT_FDCWD, ".../heldout", O_DIRECTORY) = 7` then `openat(7, "x.md", O_WRONLY)` carried no fragment on the write | directory fds are remembered per pid from their open's return value (including the `<unfinished ...>`/`resumed` split) and a path relative to one is judged under that directory |
| A08 | exception broadening | the review record's attempt, reviewer, fixed-base and blindness objects accepted undeclared fields | `deny_unknown_fields` on all of them; `schema_retries` declared |
| A09 | evidence/report drift | `raw_response_sha256` was only hex-shaped; nothing retained was hashed | the reviewer's raw answer is retained beside its record as `<record>.raw.txt` and its SHA-256 must be the record's |

**Consequences.** Packet (scope disclosure, P1-M033), digests, canaries and
the repair-family golden rebound; the campaign reruns from a new fixed base.

## F-48 — the fourth complete lane: six verified defects and the first two rulings

**Found.** 2026-09-06, lane at `78c8f9b`. Every stage passed; both passes
completed in one answer each; pass 1 (codex gpt-5.6-sol) reported eight
verified findings. Records under `docs/execution/reviews/2026-09-06-lane-78c8f9b/`
(with the retained raw answers, the first lane to keep them). Six held; two
contest standing decisions and go to the signed-ruling path the grilling
provided for (decision 6, 2026-09-03), built here because it was first needed.

| attempt | class | what was true | fix |
|---|---|---|---|
| A02 | shared-oracle coupling | the incremental oracle's comment said it does not call `paragraph::parse`; nothing enforced it | a closed tripwire (`INDEPENDENT_ORACLES`): the oracle's body may not name the production symbols it judges; run at inventory time; test |
| A04 | weak mutants | P1-M010 (oracle short-circuit) was anchored on `return Err(error)`, which has no predicate | P1-M010 is `success-error-substitution` at the same line: a failed stage treated as success |
| A09 | evidence/report drift | the markdown verifier checked headers, tuple and digest, never the cells the flip renders from artifacts | each pass block's eleven rows are re-derived from the committed record and compared; an unqualified packet's rows must be NOT_RUN |
| A10 | weak mutants | P1-M002 (predicate inversion) was anchored on a match arm | re-anchored to `parser.rs:221`, the nesting-overflow predicate |
| A11 | evidence/report drift | the integrity binding covered the verdict and raw digest, not the attempts and findings | binding v2 adds the SHA-256 of the persisted attempts, findings and reproduced ids in canonical JSON (sorted keys, compact, UTF-8 unescaped), re-derived by the gate from the record file |
| A12 | fault omissions | a relative write from a subdirectory, `../conformance/corpora/heldout/x`, joined onto the root climbed out and passed | a relative write naming the locked corpus refuses on its name, before resolution |

**Rulings.** A07 (corpus leakage: stage reads of the locked corpus are
accepted) restates F-44's rejected option and is ruled by **R-001**; A01
(vacuity: no canonical-reparse relation is declared) is false against the
committed packet — P1-T02 and P1-T04 assert `parse(emit(parse(x))) == parse(x)`
through the equality F-45 strengthened — and is ruled by **R-002**. A ruling
lives in `docs/execution/rulings/`, names an attack class and a target file,
and is in force only while its status is `ruled` and the last commit touching
it verifies against the pinned signers in `conformance/haqp/ruling-signers`
(SSH signature; Brian's key). The gate's own count of a record's unresolved
findings after rulings (`haq review-unresolved`) is what the flip consults;
the reviewer's number is recorded but does not decide. Standing rulings are
part of the specification every reviewer is given. Both rulings are drafts
until Brian signs them.

## F-49 — provider receipts, session transcripts, and the principle for how much rigour

**Found.** 2026-09-06, lane at `aa00d41`, blind pass 1 A09: "every independence
field and binding is locally recomputable; no external provider receipt
authenticates distinct reviewers or sessions." True as written. The runner
computed the identity hash, the session hash and the blindness proof itself,
discarded MiMo's response envelope, and passed `--ephemeral` to Codex, which
means "run without persisting session files to disk" — destroying the one
externally-originated artifact showing the session existed.

**Ruling (Brian, 2026-09-06).** Turn session transcripts on. Capture provider
envelopes and bind them. AI independent reviews are inherently hostile to
perfect idempotency, so a receipt attests that an exchange OCCURRED; it is
never a claim that the same review would recur.

**Fix.** `--ephemeral` is gone and the Codex rollout transcript is attributed
to its call by set difference over the sessions directory, not by newest
mtime, then copied beside the record as `<record>.session.jsonl`. The MiMo call
keeps the provider's envelope: its response id, the model it says answered, its
clock and the tokens it will bill. Both land in the record as
`provider_receipt`, typed and closed to unknown fields, and the integrity
binding becomes v3 by adding the receipt's canonical-JSON digest, so a receipt
cannot be swapped for another session's. The gate re-derives all of it: the
backend must match the reviewer's, a Codex transcript must be retained and hash
to its claimed digest and length, a MiMo receipt must carry a response id, a
model, a clock and non-zero billed usage, and a backend that produces no
receipt at all cannot qualify.

**What this does not claim.** A receipt is not unforgeable by a hostile runner.
It is externally originated and checkable against the vendor's dashboard, which
is the evidence a non-hostile qualification can actually produce. The residual
is stated rather than closed — see AM-17.9, which also settles how much rigour
this campaign spends and where.

## F-50 — the fifth complete lane: eight more, and the gate's own reach

**Found.** 2026-09-06, lane at `aa00d41`. Every stage passed; pass 2 clean;
pass 1 (codex gpt-5.6-sol) reported ten verified findings. Two were answered by
the rulings signed that day and are recorded under F-48/F-49 (A07's ruling
mismatch became F-48's narrowing; A09 became F-49's receipts). The other eight
held, and eight of the ten targeted `haq.rs` — the gate, not the suite, which
is what prompted AM-17.9.

| attempt | class | what was true | fix |
|---|---|---|---|
| A01 | vacuity | a formatter error became a `Negative` case for ANY category, so a formatter that lost nested or deep documents reclassified them and the run still reached its accepted target from the categories that still worked | only the five categories that declare malformed input may be refused; a refusal elsewhere is lost support and fails the generator |
| A02 | shared-oracle coupling | the independence scan matched literal paths, so `use liminal_source::paragraph::parse as p; p(input)` named none of them | a `use` renaming a watched symbol makes its alias watched in that file |
| A03 | missing negatives | the invalidation relation re-checked only the keys that already invalidated, so a `record` that did nothing passed monotonicity | the newly recorded read must now invalidate, the old ones must not have been lost, and the unread key must still not invalidate |
| A04 | weak mutants | P1-M046 was anchored on `pub const REPAIR_STALE_BASIS: &str = "JUR053";` — deleting a diagnostic code's name changes compilation, not staleness | constants and statics are declarations, EXCEPT one binding a number: `pub const MAX_NESTING: u16 = 256;` is the canonical threshold site and stays mutable. P1-M046 re-anchored to `checker.rs:288`, the prestate/chain check; P1-M042 to `checker.rs:567`, a real single-evidence threshold |
| A05 | fault omissions | the durable census matched `fs::rename(`, so `use std::fs::rename as mv; mv(a, b)` renamed a durable transition out of the surface | durable symbols are matched by name and an aliasing `use` adds its alias for that file; the census grew again and the packet discloses `store/log.rs` 7, `store/mod.rs` 5, `file.rs` 8, `crash.rs` 2 and `store/asof.rs` 1 |
| A06 | nondeterminism | the concurrency tokens ended in `(`, so `let f = thread::spawn;` named none of them while spawning all the same | the symbol's mention is the signal; calling it is not required |
| A07 | corpus leakage | a `chdir` into the locked corpus made every later relative write nameless, and resolution against the repository root put it somewhere harmless | each pid's announced cwd is remembered and its relative paths resolve there; a failed `chdir` moves nothing |
| A10 | weak mutants | mutation runs and the `#[ignore]` check both filter by a test's LEAF name, so two tests sharing a leaf let a namesake certify a declared kill | a leaf a mutant relies on must name exactly one runnable test in the tree |

**Consequences.** Packet (two mutant anchors, five disclosure counts), digests
and canaries rebound. The campaign reruns from a new fixed base.

## F-51 — the sixth lane: sixteen findings, none of them about the suite

**Found.** 2026-09-06, lane at `5fb1b57`. Every stage passed; the campaign ran
clean again (91 M executions on the interchange codec, zero artifacts); both
passes carried provider receipts for the first time — a Codex rollout of 4.4 MB
and a MiMo envelope billing 219,180 tokens. Then BOTH passes reported defects:
pass 1 eleven, pass 2 five. Every one of the sixteen targets
`crates/liminal-xtask/src/haq.rs`. None targets the Phase 1 suite.

This is the shape AM-17.9 was written for, now with evidence rather than
suspicion. Six of pass 1's eleven are gaps in fixes landed the same day:
the alias scan adds `prod(` but the body calls `prod::parse(` (A02); a
turbofish `rename::<T>(` lacks `rename(` (A05); `pthread_create` is in no
token list (A06); `fchdir` was excluded on the previous round's advice and
now leaves a stale cwd (A07); `renameat`'s destination is not anchored
(A11); a dirfd whose open was not `O_DIRECTORY` is dropped (A10). Pass 2's
five are all one observation: the verifier does not verify itself — it is not
run under the corpus tracer, its own oracles are outside the independence
registry, and a change to the gate is not caught by the gate.

**Fixed here (class B under AM-17.9 — the record could assert something
untrue).** A09: `verify_markdown_review_blocks` and `verify_oracle_independence`
ran on the INVENTORY path only, so the path that judges a flipped packet
accepted review cells disagreeing with their committed records, and an oracle
coupled to the production path it judges. Both now run on the qualified path.

**Standing.** The remaining fifteen are gate-hardening against an author who
controls the gate. Under AM-17.9 they are class C: recorded, ruled, not
blocking — but AM-17.9 is unratified, so they block, and the loop cannot
terminate while every fix is new gate surface for the next pass. The decision
is Brian's and is recorded here as open.

**A08 is the one class-C finding that also weakens a ruling.** `claim_requires`
matches by substring, so a phrase can be found inside a sentence that negates
it, and a signed ruling could suppress a defect it does not answer. Whatever
the termination decision, ruling scope needs a stronger predicate than
substring containment.

## F-53 — lane 5fb1b57 closed: twelve fixed, two ruled, two rejected

**Disposition of all sixteen.** Under AM-17.9, ratified 2026-09-07.

| finding | class | disposition |
|---|---|---|
| P1-A01 vacuity | A | fixed (F-52): the CST oracle asserted lossless emit and idempotence, both satisfied by a formatter returning a constant; it now requires the document's words to survive |
| P1-A02, P2-A10 shared-oracle coupling | A | fixed (F-52): module aliases watched, and the five oracles in `haq.rs` added to the independence registry |
| P1-A04 weak mutants | A | fixed (F-52): every operator ADR-0020 §3 names must appear in the plan |
| P1-A05, P1-A11 fault omissions | A | fixed (F-52): durable symbols matched on a word boundary; each path anchored to its own dirfd |
| P1-A06 nondeterminism | A | fixed (F-52): `pthread_create`, `libc::clone`, `clone3` |
| P1-A07, P1-A10 corpus leakage | A | fixed (F-52): `fchdir` moves the pid; every successful open is remembered |
| P1-A08 exception broadening | B | fixed (`e67eda2`): rulings gained `claim_excludes` |
| P1-A09, P1-A12 evidence/report drift | B | fixed (F-51, F-52): the qualified path runs the record-agreement and oracle checks; an anchor no longer matches the registry's own string literal |
| **P2-A07 corpus leakage** | **C** | **ruled: R-003** — the verifier is not traced by itself |
| **P2-A12 exception broadening** | **C** | **ruled: R-004** — a change to the gate is not caught by the gate |
| **P2-A06 nondeterminism** | — | **ruled incorrect: R-006** — a per-file census has no order to drift |
| **P2-A09 vacuity** | — | **ruled incorrect: R-005** — false on both halves |

**The two rejections, verified against `5fb1b576` — the exact tree the reviewer
read, not today's.**

P2-A09 claimed that "the committed packet is not-run so `verify_qualified_repo`
bails before reaching `verify_packet_shape`, and the inventory gate test only
asserts acceptance of the committed tree, not rejection of a doctored one."
Both halves are false in that tree. `verify_qualified_repo` calls
`verify_packet_shape` as its first check after `read_packet`, which itself has
no early return; and `the_inventory_gate_refuses_a_doctored_tree` is present,
doctoring a copied tree and requiring the refusal.

P2-A06 claimed the durable-surface measurement "compares counts not order",
leaving ordering drift undetected. The census is a map from file path to the
number of durable transitions in that file. Order is not a property it has:
reordering two `commit_if` calls inside one file changes neither which file
performs durable transitions nor how many, so there is nothing for the check to
miss. The cited coordinate, `haq.rs:2847`, is a doc comment about mutant kill
attribution and does not carry the claim either.

Both are cleared by ruling, which is the mechanism grilling decision 6
(2026-09-03) provides for a finding ruled INCORRECT — the same device R-002
used, and distinct from R-003 and R-004, which set aside findings that are
true. The reviewer's ~20-30% false-positive rate is why every finding is opened
at its line before anything moves.

**Standing residual after this lane.** Two class-C findings, both saying the
same thing in different words: the gate cannot be its own adversary. R-003 and
R-004 state the residual rather than closing it. What narrows it is external —
reviewed commits, signed rulings, a fixed base bound into every record, and the
reviewers' sight of `haq.rs` itself.

## F-54 — the seventh lane: two findings, and the reviews-first payoff

**Found.** 2026-09-09, lane at `e09ae5e`, the first with the blind reviews
running FIRST. Both passes answered in about six minutes: pass 2 clean, pass 1
two findings — down from sixteen at `5fb1b57` and ten at `aa00d41`. The lane
was stopped there rather than spending the remaining ~50 minutes on stages
whose verdict could not change the flip. That is the whole point of the
reordering, and it paid on its first use.

Both receipts landed: a 2.1 MB Codex rollout and a MiMo envelope billing
222,881 tokens.

| attempt | class | what was true | fix |
|---|---|---|---|
| A07 | corpus leakage | the cwd and directory-fd maps are keyed per pid and were never inherited, so a parent that `chdir`'d into the locked corpus and then forked left the CHILD unanchored: its relative write carried no forbidden text and resolved against nothing | a successful `clone`/`clone3`/`fork`/`vfork` copies the parent's cwd and its open descriptors to the child pid, which is what the kernel does; a failed clone creates no child |
| A09 | evidence/report drift | the gate checked headers, the packet digest, the status tuple, the canary table and the review blocks — and nothing else. Twelve further tables are rendered from artifacts, so a human-facing cell could claim a result no artifact supports while every checked thing stayed valid | while the packet is unqualified, a cell in those twelve tables may carry only what the PACKET declares (family names, predeclared counts, requirement and test identifiers, the ADR's 30-minute budget, a closed activation vocabulary), and no cell in ANY table may BE a verdict word. Column headers are exempt, since `Executed` there names a column rather than claiming one |

A09 is class B under AM-17.9 — the record could assert something untrue — and
it is the third time this species has appeared: F-45 caught the review cells
unverified, F-51 caught the check running on the inventory path but not the
qualified one, and this catches the tables neither reached. The pattern is
worth naming: every artifact the flip WRITES needs a check that re-derives it,
and adding a renderer without adding its check is how the record drifts.

**Note on the qualified path.** These twelve tables are re-derived only in the
unqualified state, which is the state every reviewer sees, since the reviews
run before the flip. A qualified packet's tables are bound by the packet digest
the markdown carries and by the canary and review-block checks. Closing the
remainder — re-deriving each rendered table from its evidence artifact after
the flip — is recorded here as the next piece of this species, not as done.

## F-55 — the eighth lane: the reviewer finally attacked the suite

**Found.** 2026-09-09, lane at `9eca4f0`. Pass 2 clean; pass 1 nine findings.
The count went up from two, and the reason matters: for the first time the
findings are mostly about the SUITE — the mutation plan and the campaign's
coverage — rather than the gate's own scanners. Five of the nine say a declared
mutant cannot be applied or cannot be killed by the tests that name it.

| attempt | class | what was true | fix |
|---|---|---|---|
| A01 | vacuity | the generated campaign called `liminal_cst::parse` only, so `coarse_parse` — the other public entry point, the one that must never reject — could return empty block metadata for every input and no case would notice | every source case now runs `coarse_parse` too, judged against the SOURCE and not against the fine parse: a document with content has at least one block, each block's range lies inside the source and holds content, and blocks do not overlap |
| A04 | weak mutants | P1-M041 declared predicate-inversion on `let mut indegree: BTreeMap<..> =`, which has no predicate | re-anchored to `repair.rs:253`, `if order.len() == plan.steps.len() {` |
| A05 | fault omissions | P1-M049 declared disabled-crash-point on `impl<C: CrashInjector> CrashInjector for &C {`, an impl header. The mutability screen listed `"impl "` with a space, so a GENERIC impl read as behaviour | `impl<` joins the declaration list; P1-M049 re-anchored to `ilrp.rs:273`, a real `crash_if_armed` call |
| A06 | nondeterminism | P1-M020's ordering mutant on the store log's sort named P1-T29 and P1-T04 as killers — rendered-output basis and malformed-source round-trip, neither of which exercises log ordering | killers re-pointed to P1-T34 and P1-T08, replay determinism and snapshot migration |
| A07 | corpus leakage | descriptor tracking followed opens but not duplication, so `dup`ing a locked directory fd and writing through the duplicate anchored against nothing | `dup`, `dup2`, `dup3` and `fcntl(F_DUPFD)` give the duplicate what its source named |
| A09 | evidence/report drift | table cell VALUES were checked but not how many cells a row had. The metamorphic-relation table in the committed markdown had a four-column header over ten-cell rows | a row must have exactly its header's width, and the committed table was corrected |
| A10 | evidence/report drift | a resolution receipt naming any command with a self-authored `verification_exit_code: 0` was accepted without the command ever running — the resolution asserted its own success | the command must come from a closed registry and the gate RUNS it. Closed rather than executed as written: running arbitrary text out of an evidence file would hand whoever wrote the file this process |
| A11 | weak mutants | P1-M064 declared stale-basis-acceptance on `) -> Result<WorkspaceBasis, PerspectiveError> {`, a multi-line signature's continuation, which begins with none of the screened keywords | a line that closes a parameter list and opens a body is a declaration; P1-M064 re-anchored to `inputs.rs:103` |
| A12 | missing negatives | P1-M053 was anchored on `assert!(!deps.invalidated_by(&b));` inside `mod tests`. Killing it would measure the ORACLE, not the product — the shared-oracle defect this campaign exists to catch, declared as a mutant | anchors inside `#[cfg(test)]` items are refused |

**What the new screen then found on its own.** A12 named one mutant in test
code. Running the strengthened check over the whole plan found **eight** bad
anchors, not the three the reviewer cited: six mutants sat inside `mod tests`
(P1-M035, P1-M036, P1-M039, P1-M053, P1-M055, P1-M061) and two on declarations
(P1-M049, P1-M064). All were re-anchored to production sites, packet and closed
registry in lockstep.

P1-M055 could not simply move: it declared `threshold-plus-one`, and the
Basis/revision/query invalidation family has no numeric threshold anywhere in
its production code. An operator had been assigned to a family with no site for
it. It is now `missing-enum-dispatch` at `durability.rs:38`, a real dispatch
arm; `threshold-plus-one` still appears five times elsewhere, so §3's operator
list stays satisfied.

**Why this lane is the encouraging one.** Seven lanes of findings were almost
entirely about the gate. This one reached past the gate into the plan the gate
judges, and what it found there was real: a quarter of one family's mutants
measuring test assertions instead of the product. That is the failure mode
ADR-0020 §3 exists to prevent, and it survived seven lanes because nothing
checked whether an anchor was production code.

## F-56 — the screen that reads test modules could not read Rust

**Found.** 2026-09-09, in the external review of `e490671` and the pass over
its own follow-up. Not a lane finding: two defects in the anchor screen F-55
had just added, caught before the ninth lane started.

| what was true | fix |
|---|---|
| `continues_signature` matched `trimmed.starts_with(')') && trimmed.ends_with(';')`, which is a bare `);` — the terminator of every multi-line call in the codebase. Behaviour anchors would have been refused as declarations | a signature continuation contains `->` or ends `{`; `);` is behaviour |
| `line_is_inside_test_code` counted every `{` and `}`, including those inside line comments and string literals. One `// weird }` closes the test module early and a test-code anchor reads as production | only structural braces count, via a new `structural_braces` |
| `structural_braces` then opened a character literal at every `'`. A lifetime never closes one, so `Formatter<'_>) -> Result {` lost its brace. Eighteen lines in the anchor-bearing files pair an odd `'` count with a brace | a `'` opens a literal only when it closes within one escape sequence; raw and byte strings are recognised with their hash counts |
| in the first draft of the escape handling, `'\''`'s quote at `at + 2` is the escaped character, not the terminator, so the scan resumed mid-literal | the search starts past the escaped character; `'\u{7d}'` is covered |

**Why it is recorded.** Every one of these undercounts brace depth, and brace
depth is the whole signal deciding whether an anchor is production code. The
screen that caught six mutants measuring test assertions was itself unable to
parse the language it screened. It was added and reviewed in the same day, so
nothing downstream ever depended on the wrong answer — but a screen this load-
bearing earning a pass on a scanner that mistakes `Formatter<'_>` for an
unterminated literal is the species of defect this campaign exists to find,
whether a lane or a review is what surfaces it.

**Nits not taken.** `#[cfg( test )]` with inner spaces is not syntax rustfmt
emits. The block-comment case spans lines a per-line scanner never sees. The
resolution registry's commands run with the gate's environment and no timeout
because they are the project's own gates: a hang there is a hang in CI.
