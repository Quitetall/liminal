# 0020. Require high-assurance Phase 1 suite qualification

- **Status:** accepted
- **Date:** 2026-07-20
- **Deciders:** Brian
- **Related:** M17; `docs/execution/phase1-suite-review.md`; v4 Part XXII Phase 1;
  R4 §§2, 8, 10, 11.6; ADR-0005; ADR-0013

## Context

M17 already requires a proposed Phase 1 suite, coverage matrices, mutation and
fuzz evidence, and two independent adversarial passes. Those nouns do not set a
qualification bar. A suite can claim each artifact while using trivial mutants,
short fuzz runs, shared oracles, vacuous predicates, or reviews that contaminate
one another.

Phase 1 will freeze foundations used by later parsers, graph projections,
recovery, revision tracking, and external adapters. Test quantity is therefore
not enough. Ratification needs evidence that the suite detects seeded defects,
survives malformed state, reproduces failures, and has no unexamined critical
requirement. The campaign must remain finite so it can be repeated after fixes.

## Decision

Phase 1 suite ratification SHALL use the **HAQP-1 high-assurance qualification
profile** below. Passing HAQP-1 does not ratify the suite; it only makes the
proposed packet eligible for Brian's separate T1 ratification decision.

### 1. Two execution lanes

- The **continuous lane** runs deterministic unit, integration, regression,
  malformed-input, and gate-canary tests on ordinary changes. It includes
  `just ci` and must not depend on elapsed-time fuzz success.
- The **qualification lane** runs from one fixed clean commit and source tree.
  It contains every HAQP-1 item and is rerun in full after the last verified
  defect is fixed. Partial reruns are diagnostic evidence only.
- Every command, exact toolchain and dependency version, lockfile hash, target,
  seed, declared non-locked regression/fuzz corpus hash, environment
  classification, exit status, and raw artifact hash is recorded. Missing
  evidence fails closed.

#### Qualification provenance amendment (2026-08-12)

The qualification packet is committed after its fixed-base campaign. Its
provenance records the campaign commit and that commit's tree hash. The
qualification commit MUST have exactly that campaign commit as its first
parent; the verifier checks parent and tree hashes, requires a clean tree, and
rejects any other provenance shape. This parent-binding design avoids a
self-referential packet commit hash while keeping evidence tied to one clean
source tree. No external manifest or signing key is required.

### 2. Bidirectional requirements traceability

- Every Phase 1 normative law, `MUST`, threat boundary, abuse case, fault class,
  work-order exit criterion, and ratification gate has a stable requirement ID.
- Every ID maps to at least one test, and every test maps back to at least one
  ID. Machine verification rejects duplicates, orphans, unknown IDs, and missing
  critical rows.
- Every applicable law has positive, negative, malformed/adversarial,
  basis/provenance, and deterministic-replay evidence. Stateful laws also have
  injected-fault and idempotent-recovery evidence.
- Uncovered critical rows cannot be waived inside the packet. Any exception is
  an exact coordinate accepted through a separate T1 amendment; wildcards,
  whole-file exclusions, and substring allowances are forbidden.

### 3. Tests of the tests

- Qualification predeclares at least **64 semantic mutants** across critical
  surfaces. Operators include predicate deletion/inversion, threshold ±1,
  missing enum or kind dispatch, success/error substitution, stale-Basis
  acceptance, wrong Holder/profile selection, skipped durable transition,
  disabled crash point, ordering nondeterminism, oracle short-circuit, and
  broadened allow-list.
- Every critical surface family receives at least eight mutants, and no single
  operator supplies more than one quarter of the denominator. Mutation selection
  cannot cluster around easy-to-kill utility paths.
- Mutation score is **100% of applicable, non-duplicate, non-equivalent
  mutants**. One survivor blocks eligibility.
- An equivalent or duplicate disposition names the mutant, shows the proof, and
  receives concurring verification in both adversarial-pass records. Disposed
  mutants remain visible in the numerator/denominator report.
- Every ratification gate and every M17 Level-3 measurement conjunct gets at
  least one disposable gate canary: a deliberate violation must make the exact
  expected gate fail. Canary execution occurs outside the working tree and
  restores the fixed review base afterward.

### 4. Generated, metamorphic, and fuzz evidence

- Five critical surface families each execute at least **100,000 accepted
  deterministic generated cases** from recorded seeds: source/CST/formatting,
  graph/interchange codecs, transforms/projections, repair/ILRP/recovery, and
  Basis/revision/query invalidation.
- Generator attempts and discards are recorded. Discard rate above one percent,
  post-failure filtering, or shrinking the declared domain fails qualification.
- Each family defines independent metamorphic relations where applicable:
  canonical reparse, idempotent replay, inverse/undo, irrelevant-input
  invariance, commuting independent transactions, incremental/full equivalence,
  and deterministic permutation. A relation is not proved by calling the
  implementation's own equality or normalization path on both sides.
- Each family also receives one **30-minute sanitizer-enabled fuzz campaign**
  with at least 16 predeclared seeds spanning valid, boundary, truncated,
  malformed, and hostile input. Total fuzz budget is at least 150 target-minutes.
- Every crash, panic, timeout, memory error, divergence, or minimized failure is
  a failing result. Minimized artifacts become named regression fixtures before
  the qualification lane is rerun.
- Concurrent code receives deterministic schedule exploration. If Phase 1 adds
  no concurrent implementation, the packet records the condition as not
  applicable rather than fabricating coverage.

### 5. Oracle and fault independence

- Incremental and full-reparse oracles may share raw bytes and published data
  schemas only. They may not share parser events, lowering code, canonicalizers,
  anchor recovery, comparison shortcuts, or expected-result construction.
- Expected values come from fixed literals, worked spec examples, or a separately
  implemented oracle. Self-snapshots generated by the system under test are not
  authority without independent verification.
- Every registered durable-transition crash boundary is injected immediately
  before and after the transition. Recovery runs twice and must reach the same
  terminal state without hidden staged data, duplicate effects, or mixed Basis.
- Fault matrix is exhaustive over registered boundaries. Runtime discovery and
  declared inventory must match exactly; either-side drift fails.

### 6. Independent adversarial review

- Pass 1 attacks implementation and packet for vacuity, shared-oracle coupling,
  missing negatives, weak mutants, fault omissions, nondeterminism, corpus
  leakage, exception broadening, and evidence/report drift.
- Pass 2 starts from the original spec and fixed review base. It cannot read Pass
  1 prompts, findings, dispositions, or fixes until it records its own findings.
- Passes use distinct reviewer identities and isolated conversation/session
  state. If both reviewers are automated, they use different model families;
  otherwise at least one reviewer is human. Sanitized prompt and reviewer
  identity hashes are recorded without credentials or secret material.
- Each pass records at least 12 concrete falsification attempts, including
  attempted violations that the suite correctly catches. Findings cite exact
  evidence and are independently reproduced before any fix.
- Zero verified findings may remain unresolved. False positives remain recorded
  with reproduction evidence; they are not silently deleted.

### 7. Bounded campaign and residual risk

- HAQP-1 qualification is a review-candidate campaign, not a per-edit tax. Its
  intended wall-clock ceiling is eight hours on the recorded reference machine.
  Crossing that ceiling twice blocks ratification and triggers a superseding ADR
  or harness optimization; thresholds cannot be silently reduced.
- Packet lists every known limitation with owner, severity, trigger, affected
  requirement, evidence, and planned resolution phase. "Future work" without
  those coordinates is invalid.
- Locked acceptance corpora remain black-box inputs touched only by existing
  approved verifiers. HAQP-1 does not authorize inspecting, listing, copying,
  hashing, or deriving new fixtures from locked corpus contents.

**What would falsify or reverse this:** two clean qualification attempts exceed
the eight-hour ceiling, required tooling cannot produce reproducible evidence,
or HAQP-1 encourages equivalent-mutant or low-value-case inflation without
finding real seeded defects. Reversal requires a superseding ADR with measured
campaign data and cannot weaken already-recorded failures or erase artifacts.

## Consequences

- **Positive:** ratification evidence demonstrates sensitivity, independence,
  replayability, traceability, and fault coverage instead of test count alone.
- **Negative:** final qualification consumes up to eight hours, requires at
  least 64 reviewed mutants and 150 target-minutes of fuzzing, and makes every
  post-campaign defect fix trigger another full run.
- **Follow-ups:** AM-17.1 binds M17's packet gate and review records to HAQP-1.
  Phase 1 work orders must budget both execution lanes without ratifying the
  suite automatically.

## Alternatives considered

- **Coverage percentages plus ordinary CI** — rejected because high line or
  branch coverage does not show that assertions detect semantic defects.
- **Unbounded fuzzing and exhaustive model checking** — rejected because results
  cannot be repeated on a review schedule and termination becomes subjective.
- **Broad source-wide mutation testing** — rejected because utility-code mutants
  inflate cost and score while diluting critical contract coverage.
