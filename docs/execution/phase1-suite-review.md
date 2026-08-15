# Phase 1 suite review packet

suite_version: phase1-haqp1-proposed-v2-seven-fuzz
fixed_review_base: NOT_RUN
status: proposed

This file is the proposed packet schema and predeclared inventory, not
qualification evidence. Planned rows below remain pre-run until the HAQP campaign
records raw artifacts. Nothing here claims HAQP-1 eligibility, Phase 1
authorization, suite ratification, or access to locked acceptance inputs.

## Packet authority and bounds

| Field | Value |
|---|---|
| profile | HAQP-1 (ADR-0020) |
| packet authoring state | predeclared inventory committed |
| qualification state | NOT_RUN |
| concurrent implementation | `not_applicable` — M21 explicitly reserves sync implementation for Phase 6; no Phase 1 concurrent implementation exists |
| ratification decision | unratified — Brian T1 decision required |
| Phase 1 execution authorization | NOT_RUN |
| locked acceptance corpora touched | NOT_RUN — prohibited |
| credentials or secret material recorded | NOT_RUN — prohibited |
| held-out contents inspected, listed, copied, hashed, or derived | NOT_RUN — prohibited |
| intended campaign ceiling | 8 hours |
| complete post-fix qualification rerun | NOT_RUN |
| machine inventory | `conformance/haqp/packet.json` (`cargo run -p liminal-xtask -- haq verify-inventory`) |
| machine inventory BLAKE3 | `aec2dea85361b1f9d87a8e8c4abe8fbf98cd0262d5baac189403e211de979975` |
| packet digest | `b06dc5b3d891a5bf81b970f31b63c59b6c3cdb876cba2a31143962dcc9dde3f5` |
| machine status tuple | qualification_state=not-run; qualification_stage=1a; requirements=55; tests=38; mutants=65; canaries=32; generated=5; crash_boundaries=8; reviews=2 |

## Execution lanes

### Continuous lane

| Field | Value |
|---|---|
| command inventory | NOT_RUN (`just ci` plus deterministic unit, integration, regression, malformed-input, and gate-canary commands) |
| elapsed-time fuzz dependency | NOT_RUN — must be absent |
| latest exit status | NOT_RUN |
| raw log hash | NOT_RUN |

### Qualification lane

| Field | Value |
|---|---|
| fixed commit (40 lowercase hex) | NOT_RUN |
| fixed source tree (40 lowercase hex) | NOT_RUN |
| clean-tree proof | NOT_RUN |
| exact commands, in order | NOT_RUN |
| toolchain and dependency versions | NOT_RUN |
| lockfile SHA-256 | NOT_RUN |
| target triple | NOT_RUN |
| environment classification and reference-machine identity | NOT_RUN |
| seeds | NOT_RUN |
| declared unlocked regression/fuzz corpus SHA-256 | NOT_RUN |
| per-command exit statuses | NOT_RUN |
| per-command raw artifact hashes | NOT_RUN |
| wall-clock elapsed | NOT_RUN |
| last verified defect and fix coordinate | NOT_RUN |
| full rerun after last verified fix | NOT_RUN |
| missing-evidence fail-closed check | NOT_RUN |
| unlocked-corpus path audit | NOT_RUN — traced open-path manifests required; held-out paths are forbidden |
| campaign clock artifact | NOT_RUN — `scripts/haqp_campaign_clock.sh`; two clean >8-hour runs block ratification |

Partial reruns are diagnostic only. Any verified fix invalidates eligibility until
this lane runs again in full from one fixed clean tree.

## Stable requirement inventory

Populate one row for every Phase 1 normative law, `MUST`, threat boundary, abuse
case, fault class, work-order exit criterion, and ratification gate. IDs are
immutable after review-base freeze.

| Requirement ID | Kind | Exact source coordinate | Normative text/hash | Critical | Stateful | Owner | Inventory state |
|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Machine checks must reject duplicate IDs, orphan tests, unknown IDs, and missing
critical rows. Exceptions require exact T1-amendment coordinates; wildcards,
whole-file exclusions, and substring allowances are forbidden.

## Declared future gate tests

| Test ID | Exact test | Activation | Requirement IDs | Positive | Negative | Malformed/adversarial | Basis/provenance | Deterministic replay | Fault injection | Idempotent recovery | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P1-T01 | `laws::formatter_idempotence_law_holds` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T02 | `laws::canonical_round_trip_law_holds` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T03 | `laws::incremental_equals_full_compile_law_holds` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T04 | `classes::malformed_source::malformed_source_never_panics_and_round_trips` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T05 | `classes::malformed_source::fuzz_regressions_stay_fixed` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T06 | `classes::golden_render::full_document_html_matches_golden` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T07 | `classes::golden_render::incremental_patch_equals_full_render` | Phase 1 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| P1-T08 | `classes::migration::old_snapshots_open_after_format_evolution` | Conditional: first persisted-format ADR | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Every test must map back to at least one requirement ID. Every applicable law
requires positive, negative, malformed/adversarial, basis/provenance, and replay
evidence; stateful laws also require fault and idempotent-recovery evidence.

## Coverage matrices

### Normative-law coverage

| Requirement ID | Positive tests | Negative tests | Malformed/adversarial tests | Basis/provenance tests | Replay tests | Coverage state |
|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

### Class and threat-boundary coverage

| Class/boundary ID | Source coordinate | Abuse cases | Tests | Oracle | Coverage state |
|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

### Abuse-case coverage

| Abuse ID | Trigger/input | Expected refusal or preservation | Tests | Evidence hash | Coverage state |
|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

### Fault and recovery coverage

| Fault ID | Registered boundary | Before injection | After injection | Terminal state | Second recovery identical | Hidden staged data absent | Duplicate effects absent | Mixed Basis absent | Coverage state |
|---|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Runtime-discovered and declared durable-transition boundary inventories must match
exactly. Any drift fails qualification.

## Semantic mutation campaign

Target: exactly **65 predeclared semantic mutants**, 13 in each HAQP family below.
Applicable, non-duplicate, non-equivalent score target: **100% killed**; one
survivor blocks eligibility. No operator may supply more than 16 mutants. Required
operators include predicate deletion/inversion, threshold ±1, missing enum/kind
dispatch, success/error substitution, stale-Basis acceptance, wrong
Holder/profile selection, skipped durable transition, disabled crash point,
ordering nondeterminism, oracle short-circuit, and broadened allow-list.

| Family | Planned | Executed | Applicable | Killed | Survived | Equivalent | Duplicate | Raw report hash | Result |
|---|---:|---|---|---|---|---|---|---|---|
| source/CST/formatting | 13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| graph/interchange codecs | 13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| transforms/projections | 13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| repair/ILRP/recovery | 13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Basis/revision/query invalidation | 13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

### Mutant record schema

| Mutant ID | Family | Operator | Exact source coordinate | Semantic defect | Expected killing tests | Actual killing tests | Applicable | Disposition | Proof hash | Pass-1 concurrence | Pass-2 concurrence | Result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Equivalent and duplicate mutants remain in numerator/denominator reporting and
require proof plus concurrence in both independent review records.

## Disposable gate-canary campaign

Target: exactly **32 predeclared canaries** covering every ratification gate and
every M17 Level-3 measurement conjunct. Execution occurs outside working tree and
restores fixed review base after each attempt.

| Canary ID | Gate/conjunct | Deliberate violation | Exact expected failure | Isolated command | Failure observed | Base restored | Raw hash | Result |
|---|---|---|---|---|---|---|---|---|
| C01 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C02 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C03 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C04 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C05 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C06 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C07 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C08 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C09 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C10 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C11 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C12 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C13 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C14 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C15 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C16 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C17 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C18 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C19 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C20 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C21 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C22 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C23 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C24 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C25 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C26 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C27 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C28 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C29 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C30 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C31 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| C32 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

## Generated, metamorphic, and fuzz evidence

Each family target is at least **100,000 accepted deterministic generated cases**
from recorded seeds and one **30-minute sanitizer-enabled fuzz campaign** with at
least 16 predeclared valid, boundary, truncated, malformed, and hostile seeds.
Planned total fuzz budget: 150 target-minutes. Discard rate must be at most 1%;
post-failure filtering or domain shrinking fails qualification.

| HAQP family | Accepted target | Accepted | Attempts | Discards | Discard rate | Seed-set hash | Generator/domain hash | Shrinks | Sanitizer | Fuzz target | Fuzz duration | Corpus hash | Crash/panic/timeout/memory/divergence count | Minimized fixtures | Raw hashes | Result |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| source/CST/formatting | 100000 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 30 minutes | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| graph/interchange codecs | 100000 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 30 minutes | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| transforms/projections | 100000 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 30 minutes | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| repair/ILRP/recovery | 100000 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 30 minutes | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Basis/revision/query invalidation | 100000 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 30 minutes | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

### Metamorphic relation records

| Family | Canonical reparse | Idempotent replay | Inverse/undo | Irrelevant-input invariance | Commuting independent transactions | Incremental/full equivalence | Deterministic permutation | Independent oracle proof | Result |
|---|---|---|---|---|---|---|---|---|---|
| source/CST/formatting | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| graph/interchange codecs | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| transforms/projections | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| repair/ILRP/recovery | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Basis/revision/query invalidation | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Any crash, panic, timeout, memory error, divergence, or minimized failure fails
the campaign. Minimized artifacts become named regression fixtures before full
qualification rerun. Concurrent code additionally requires deterministic schedule
exploration; otherwise record exact `NOT_APPLICABLE` proof, never fake coverage.

## Oracle and expected-value independence

| Surface | Primary implementation | Independent oracle | Shared material limited to raw bytes/published schemas | Forbidden sharing absent | Expected-value source | Self-snapshot independently verified | Evidence hash | Result |
|---|---|---|---|---|---|---|---|---|
| source/CST/formatting | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| graph/interchange codecs | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| transforms/projections | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| repair/ILRP/recovery | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Basis/revision/query invalidation | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Incremental/full oracles may share only raw bytes and published schemas—not parser
events, lowering code, canonicalizers, anchor recovery, comparison shortcuts, or
expected-result construction. Expected values must come from literals, worked spec
examples, or separately implemented oracles.

## Registered crash-boundary inventory

| Boundary ID | Registration coordinate | Durable transition | Declared-before injection | Declared-after injection | Runtime discovered | Inventory match | Recovery-twice evidence | Raw hash | Result |
|---|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

## Independent adversarial review records

Pass 2 starts from original spec and fixed base. Pass 2 cannot read Pass 1 prompts,
findings, dispositions, or fixes until its own findings are recorded. Reviewers
use distinct identities and isolated sessions; two automated reviewers use
different model families. Record hashes only—never credentials or secret material.

### Pass 1 — implementation and packet attack

| Field | Value |
|---|---|
| reviewer identity hash | NOT_RUN |
| reviewer kind/model family | NOT_RUN |
| isolated session hash | NOT_RUN |
| sanitized prompt hash | NOT_RUN |
| fixed-base hash | NOT_RUN |
| blindness proof | NOT_RUN |
| concrete falsification attempts (minimum 12) | NOT_RUN |
| attempted caught violations | NOT_RUN |
| findings artifact hash | NOT_RUN |
| unresolved verified findings | NOT_RUN |
| result | NOT_RUN |

### Pass 2 — original-spec falsification

| Field | Value |
|---|---|
| reviewer identity hash | NOT_RUN |
| reviewer kind/model family | NOT_RUN |
| isolated session hash | NOT_RUN |
| sanitized prompt hash | NOT_RUN |
| fixed-base hash | NOT_RUN |
| blindness proof | NOT_RUN |
| concrete falsification attempts (minimum 12) | NOT_RUN |
| attempted caught violations | NOT_RUN |
| findings artifact hash | NOT_RUN |
| unresolved verified findings | NOT_RUN |
| result | NOT_RUN |

### Finding and falsification-attempt records

| Pass | Record ID | Attack class | Exact target/evidence | Attempt | Observed result | Independently reproduced | Verified defect/false positive/caught violation | Fix coordinate | Disposition evidence hash | Resolved | Result |
|---|---|---|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Attack inventory includes vacuity, shared-oracle coupling, missing negatives, weak
mutants, fault omissions, nondeterminism, corpus leakage, exception broadening,
and evidence/report drift. Findings are reproduced before fixes. False positives
remain with reproduction evidence. Eligibility requires zero unresolved verified
findings.

## Approved pre-qualification hardening contracts

The next qualification run must produce, not self-declare, these records:

- qualification metadata child changes only packet/evidence/review documentation;
  source and gate code remain at fixed evidence commit;
- each sanitizer target retains build command, instrumented binary digest,
  compiler sanitizer flag, and successful runtime probe;
- each corpus audit retains raw recursive tracer output, trace digest, PID,
  tracer exit, completeness marker, and command/process binding;
- each crash recovery pair retains terminal-set, Basis, world, and duplicate-effect
  digests, equal across both recovery passes;
- each generated family retains closed relation rows and an independent oracle ID,
  source, result, and relation-matrix digest;
- mutant rows use exact `file:line` plus separate requirement ID; compilation,
  timeout, and infrastructure errors are not semantic kills;
- equivalent/duplicate dispositions bind concurrence to pass-1/pass-2 record and
  finding IDs;
- Pass 1 covers all nine closed attack classes at least once; exact class names only.

## Residual-risk coordinates

Every known limitation needs all coordinates below. Bare “future work” is invalid.

| Risk ID | Owner | Severity | Trigger | Affected requirement ID | Exact evidence coordinate/hash | Planned resolution phase | Mitigation | Acceptance authority | Risk state |
|---|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

## Raw artifact hash manifest

Hashes cover only authorized generated evidence and declared unlocked
development/regression inputs. Locked acceptance content is never enumerated or
hashed by this packet.

| Artifact ID | Kind | Producer command ID | Path or opaque authorized identifier | SHA-256 | Byte length | Created UTC | Fixed-base binding | Verification state |
|---|---|---|---|---|---|---|---|---|
| NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

## Campaign conclusion

| Gate | Required condition | Evidence | Result |
|---|---|---|---|
| fixed clean review base | one commit/tree, full provenance | NOT_RUN | NOT_RUN |
| bidirectional traceability | no duplicates, orphans, unknown IDs, or missing critical rows | NOT_RUN | NOT_RUN |
| semantic mutation | 65 declared; 100% applicable kill; no survivor | NOT_RUN | NOT_RUN |
| disposable canaries | 32 declared; exact expected gates fail | NOT_RUN | NOT_RUN |
| deterministic generation | five families × at least 100,000 accepted cases | NOT_RUN | NOT_RUN |
| sanitizer fuzzing | five families × 30 minutes; at least 150 target-minutes | NOT_RUN | NOT_RUN |
| oracle independence | no forbidden shared implementation paths | NOT_RUN | NOT_RUN |
| crash/fault matrix | declared and discovered boundaries match; before/after; recovery twice | NOT_RUN | NOT_RUN |
| independent reviews | two blinded records; at least 12 attempts each; no unresolved verified findings | NOT_RUN | NOT_RUN |
| campaign ceiling | no repeated clean-run breach of eight hours | NOT_RUN | NOT_RUN |
| residual risks | every limitation has exact coordinates | NOT_RUN | NOT_RUN |
| HAQP-1 eligibility | all preceding gates pass after final full rerun | NOT_RUN | NOT_RUN |
| Brian T1 ratification | separate explicit decision | NOT_RUN | NOT_RUN |

Current conclusion: `NOT_RUN`. Packet remains proposed and unqualified.
