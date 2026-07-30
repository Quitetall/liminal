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

## F-18 — MAJOR. The packet's fuzz budget exceeds what ADR-0020 requires, and the packet was populated to fit the verifier rather than the campaign.

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

**NOT fixed — the disposition is Brian's**, because the two branches differ in
cost by 150 minutes of machine time:

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

Option 1 is the recommendation: the ADR is canonical, and a threshold invented
by a verifier should not silently redefine the protocol it is checking.

**What this commit DID do:** added the per-target floor the ADR does state
(30 minutes), so one long target can no longer carry the 150-minute total while
another runs for seconds. It follows the ADR, not the other verifier, and says so
at the call site.

---

## F-12 UPDATE — the scratch leak is now a HARD BLOCKER on the mutation measurement, and it is broader than first recorded.

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

`typos` rejects `ba` inside the git SHA `` `c4ba072` `` in this very document,
introduced by `feda854`. Every `just ci` run since has failed at `fmt-check`,
the first recipe.

**Why it went unnoticed:** I invoked CI as `just ci 2>&1 | tail -20`. In a
pipeline the shell reports the exit status of the LAST command, so what I read
as "exit code 0" was `tail`'s success, never the recipe's. Three separate
"CI green" claims this session rested on that measurement. A verification
harness that reports the status of the wrong process is precisely the failure
this campaign keeps finding, and it is worth recording that the reviewer of the
suite made it too.

**Fix:** `extend-ignore-re = ["`[0-9a-f]{7,40}`"]` in `typos.toml`. Every
findings document cites commits in backticks, so ignoring the construct is the
durable fix rather than rewording one SHA and waiting for the next hex collision
— `ba`, `fo`, `ot` and friends recur constantly in SHAs.

**Standing correction to method:** never read a gate's result through a pipe.
Redirect to a file and test `$?` directly.
