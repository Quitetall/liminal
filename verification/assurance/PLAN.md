# Assurance maintenance implementation

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md),
AM-17.14. Baseline: `54b6450c15f161978ad63cc0f999d5090bc577f4`.

## Ordered delivery

- [x] S1: Record contract and maintenance links; independently review commit.
- [x] S2: Family catalog and `assurance check`: reject missing mandatory gates,
  unknown/duplicate IDs, uncovered targets, stale references and workflow drift.
  Disposable fixtures never open locked corpora.
- [x] S3: `assurance run <profile>` and `assurance report`: fixed command registry,
  durable incomplete/failure receipts, environment/cost observations and advisory
  impact. Selection never bypasses merge requirements.
- [ ] S4: Generated workflow wiring; Linux parity, portability/job names,
  nextest/threaded diversity, weekly advisories, informational beta. Integrate
  bounded resource harness run/lint, refusing missing limits. Hosted execution
  evidence is separate from local generation checks.
- [ ] S5: `assurance amend propose|check|apply`: parser-backed unchanged exact
  targets, independent review, externally pinned SSH policy/tool trust, isolated
  apply and producer-derived refresh. Disposable-key controls cover signatures,
  namespace, scope, revision, source/patch drift, ambiguity, protected fields and
  interruption. No human key or trust activation needed for tests.
- [ ] S6: Complete merge checks, inventory, canaries and resource controls; named
  test deltas and independent diff/commit findings verified at cited lines. Human
  enrollment and hosted execution remain explicit external boundaries.

## Public test seams

Approved CLI seams: `assurance check`, `run`, `report`, and
`amend propose|check|apply`. Test observable exits, receipts and filesystem effects;
no mock-only subprocess-isolation or signature claims. Red before green per slice.

## Evidence and limits

Record exact revision, commands, exits and durable logs per slice. Missing evidence
remains unexecuted. No local pass proves hosted operation, HAQP qualification,
formal obligations or Phase 1 authority. Historical receipts are never rewritten.

Implementation started in isolated `assurance-maintenance` worktree. This plan
does not establish code, execution or signing capability.

S1 commit `5809e3763837af03b337cd57bcfadfb588a2dd9a`: LAMU MiMo V2.5 Pro
returned PASS WITH NITS. Verified documentation-only scope and references. Kept
existing conversation-attribution convention and protocol note placement; this
follow-up closes the S1 checkbox. Receipt:
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-5809e376-review.stdout`.

S2 classification slice: 13 registered families; nine new active CLI tests, zero
ignore flips. Targeted tests pass (9/9); full suite not yet rerun. Evidence root
`/mnt/4tb/liminal-formal-evidence/reviews/`: `assurance-red-2` proves missing CLI,
`assurance-red-3` proves missing Python discovery, `assurance-red-4` proves
duplicate-key acceptance, and `assurance-green-4` passes all nine controls.
Each has `.stdout` and `.stderr`; bounded systemd runs retained actual exits.
`assurance-red-1` and `assurance-green-1` were infrastructure launch failures,
not red/green test evidence. Workflow drift checking remains S4 work; S2 stays
open until that dependent validation exists.

S2 review of `260bcde007601dd4a1810e440cb225cff9277197`: primary PASS WITH NITS;
critic response was truncated, so no critic clearance is claimed. Symlink omission
verified at `discover_python`, reproduced in `assurance-red-6`, then fixed with
explicit refusal rather than following links. Source-file versus manifest-area
check is intentional; no manifest containment rule was promised. Failed scratch
fixtures are retained for diagnosis; passing fixtures clean up. Critic's edition
concern does not reproduce with the pinned Rust toolchain. Exact review receipt:
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-260bcde0-review.stdout`.

S3 runner slice adds fixed command dispatch, external exclusive receipt paths,
incomplete/unexecuted states, fail-fast mandatory commands, advisory impact and
cost observations. Resource adapter reuses the existing bounded producer. Real
resource run and lint both exited zero (`assurance-resource-run-1` and
`assurance-resource-lint-1` under the evidence root above); no qualification is
claimed. Named test delta now +13 active / zero ignore flips; targeted controls
pass in `assurance-green-8`. `assurance-red-7` reproduced hidden `test=false`
targets; discovery now includes all Cargo targets, including existing example
and benchmark targets. Registration does not claim those are qualification tests.
Full profile execution, hosted parity and signed amendment implementation remain
pending; independent S3 review must close before this slice is marked done.

S3 review of `bc44b04dddf4fb2c163fb5e4d69bc46e8d73d71d`: primary PASS WITH NITS;
critic output again truncated. Verified that existing output reuse is refused,
so stale pending-file cleanup is neither needed nor permitted. Unknown resource
measurements stay null; unknown impact conservatively selects all families.
Producer import failures now report infrastructure unavailable. The executable
replacement note became a confirmed defect during full verification: Cargo
unlinked the running tool, and late `current_exe` returned a deleted path. The
`assurance-red-10` disposable-binary control reproduces this; commands now bind
before any child build. `assurance-green-10` passes all 16 CLI controls.

S4 generated wiring preserves job names/platforms and restores mandatory Linux
commands including advisories and bounded resource controls. Actionlint and YAML
syntax checks pass locally; hosted execution remains unobserved. New public
mechanical seam: `assurance generate-workflows` (explicit generation required by
the approved plan), with read-only drift checking through `assurance check`.

Full verification attempts remain separately recorded under the evidence root:

- `assurance-full-ci-1`: failed. Global target override broke existing crash and
  sanitizer binary lookup. New generic `run` names also entered the frozen
  name-based oracle closure through an unrelated local variable. Scoped
  maintenance names fix that collision without editing the checker or oracle;
  `assurance-oracle-scope-green` records the unchanged inventory command passing.
- `assurance-full-ci-2`: stopped at Clippy on the first environment guard.
- `assurance-full-ci-3`: all payload commands passed, including 645 nextest tests
  / 47 skipped, threaded tests, docs, advisories, inventory, canaries and resource
  controls. Final self-check could not launch the replaced executable, so overall
  verdict is infrastructure-unavailable, not pass.
- `assurance-full-ci-4`: complete merge PASS, exit 0. 646 nextest tests passed /
  47 skipped (+16 active, no ignore flips), threaded tests, both proof-support
  self-test suites, doctests/rustdoc, dependency/advisory checks, inventory,
  canaries, resource run/lint and final maintenance check all passed. Bounded
  outer service observed 2m07.126s, peak 3.6 GiB and zero swap. This is one
  development rerun with reused build artifacts; receipt cache classification
  remains unknown. It is not a budget or qualification claim. Full receipt:
  `/home/brianklam/.local/state/liminal/assurance/run.Dostemqr/merge/receipt.json`.

The infrastructure failure's minimized empty-operation proptest seed replayed
successfully after environment correction. It is retained outside the source
tree as `assurance-infrastructure-proptest-regressions` under the evidence root,
not promoted as a production defect or silently discarded. No assertions,
goldens, frozen HAQP logic, crash harness code, accepted SAS or v4 bytes changed.
S4 local implementation is verified; its checkbox remains open for hosted
execution evidence. S6 remains open for the unimplemented S5 controls and final
whole-system independent review. Nothing here closes M17 or authorizes Phase 1.

## Review dispositions and approved T1 boundary for S5

Latest local code commit: `2845197317d11e15d8258f8299d676f487549277`.
LAMU primary review returned PASS WITH NITS; critic output was incomplete and
does not supply separate clearance. Exact receipt under the evidence root:
`assurance-28451973-review.stdout`. Verified dispositions:

- Missing-profile panic claim: false. `load_catalog` checks the profile set and
  each required key before rendering. Disposable CLI probe
  `assurance-missing-profile-probe` exited 1 with `profile set drift`, not panic.
- Concurrent directory creation: a fail-closed error under the standing
  single-writer rule, not missing coverage. Retain component confinement checks;
  do not replace them with traversal through unchecked symlink parents.
- Stale UUID temporary-name collision and absent-destination rename claims:
  unsupported by the implementation. Each attempt has a fresh UUID; rename does
  not require a pre-existing destination. Existing outputs are not resumed.
- Literal fixed-argument quoting, the unused resource-output sentinel in the
  non-resource render path, missing-file diagnostic context, retained receipts
  and template comments are nonblocking maintenance nits. No automatic receipt
  deletion is authorized. Closed unknown-command refusal is intentional, not a
  missing future-profile feature. Import refusal prints an error and fails closed.

`assurance-gates-1` exited zero: 646 active / 47 deferred, including the unchanged
two deferred M17 gates. No main-branch merge, push, hosted result, signing or
Phase 1 authorization is implied by these local development results.

Brian approved the hybrid boundary on 2026-09-16: humans authorize critical
changes and batch scope; the externally pinned trusted maintenance tool handles
coordinate-only updates within that scope. Each batch binds base revision,
permitted targets, change class, and tool/policy versions. The tool cannot expand
its authority. Semantic changes, ambiguity, or out-of-scope changes require human
review rather than automatic apply.

The trusted tool obtains independent LAMU review directly, binding reviewer
identity, patch digest, verdict, and verified finding dispositions to the exact
proposal. Candidate-supplied `reviewed` metadata is insufficient. Unavailable,
inconclusive, or mismatched review refuses apply. Human standing-policy signing,
batch authorization, and external enrollment remain required; no per-proposal
human review signature is required for eligible changes within an approved batch.
This approval resolves the policy boundary, not S5 implementation or activation.

S5 CLI controls must demonstrate refusal of unauthorized batches, changed base
revisions, targets outside scope, mismatched tool/policy versions, semantic changes,
ambiguous targets, and substituted review receipts. Positive controls must use
disposable signing keys and an explicitly authorized fixture batch. Existing
signature, source/patch drift, isolation, and interruption controls remain required.

## S5 first slice: read-only proposals (not complete)

Added `amend propose` at the approved CLI seam. It reads exact committed blobs,
requires one whole-line expression inside an unchanged parsed top-level function,
and binds both source revisions/hashes plus the enclosing source hash. It grants
no authority and establishes no registry membership, mutation equivalence, review,
or apply eligibility. Parser versions were already locked: syn 2.0.119 and
proc-macro2 1.0.106. Only xtask dependency edges/features were added.

Evidence under `/mnt/4tb/liminal-formal-evidence/reviews/`:

- `assurance-s5-red1` selected zero tests due to an abbreviated exact filter;
  it is not red evidence. Corrected `assurance-s5-red2` ran the named test and
  exited 101 on the missing `amend` command. `assurance-s5-green1` passed it.
- `assurance-s5-red3` reproduced inherited `GIT_DIR` redirecting source reads
  into a foreign fixture repository. Git environment overrides are now stripped;
  no environment values are logged. `assurance-s5-green2` passed 18 controls.
- `assurance-s5-green3` passed all 19 CLI controls, including five refusal cases
  in one test. Named delta is +3 active since the previous 646-test baseline,
  zero ignore flips. Disposable fixture commits are not project commits.
- MiMo draft `assurance-s5-test-draft.stdout` was not adopted verbatim: wrong
  anchors, changed-body acceptance, invented eligibility metadata, and missing
  fixture root markers were rejected. Main agent verified the adapted controls.
- `assurance-s5-review1` is the actual LAMU diff review, PASS WITH NITS. Attached
  MCP transport was closed; fresh stdio worked. Runtime logged a model fallback,
  so the review header alone is not evidence of the final provider identity.
  Two factual nits were checked and rejected: filtering blank lines permits, not
  refuses, blank-line additions outside the function; pinned proc-macro2's
  `src/location.rs:12` explicitly defines columns in UTF-8 characters, not bytes.
  Naming/single-element-vector suggestions are nonblocking. No critic clearance
  is claimed. Review predates the equivalent Clippy formatting correction and
  final explanatory documentation/comments.
- `assurance-s5-ci1` stopped at Clippy's inline-format-argument lint. Failure is
  retained; the correction changes no behavior. `assurance-s5-ci2` failed with
  648 passed / 1 failed / 47 skipped. The crash-replay test rejected historical
  crash evidence's old lockfile hash after the approved parser dependency edges
  changed Cargo.lock. `assurance-s5-crash-probe1` reproduced this exact failure
  (exit 101). Both replays exercised 8/8 boundaries; all boundary and scenario
  rows match the historical artifact. Only source commit/tree and lockfile hash
  differ. Producer source `conformance/src/bin/crash_evidence.rs:29` reads HEAD
  and the live lockfile, so promotion of dirty-tree replay output would misbind
  provenance. Commit source first, then rerun the existing producer from that
  committed source; never copy hash fields or change the frozen checker. Full
  verification remains failed until that producer refresh and a full rerun pass.

Remaining S5: closed-registry and patch binding, authenticated human batch and
external tool/policy admission, direct independently bound review, disposable-key
negative/positive controls, and isolated apply with producer refresh. No human
signing, key reads, enrollment, main merge, push, or qualification occurred.

### Committed source, producer refresh, and full local verification

Source commit `16661be5eb3ef856136487d7f67e7935f118c76c` received actual LAMU
`review_commit`: PASS WITH NITS (`assurance-16661be5-review.stdout`). Verified
that the alleged unused `spec` binding is consumed by `cat-file` at
`amendment.rs:153`; no removal is appropriate. The CLI's `usize` parser does not
itself reject zero; the existing function guard does. Nested subexpressions have
their own narrower spans, so their mere presence does not imply multiple exact
matches. Diagnostic wording, fixture branch name and substring checks are
nonblocking nits; none calls for changed behavior or weaker tests.

`assurance-s5-crash-refresh1` ran the existing crash producer from the clean source
commit above with an external output destination, exit 0. The preserved output
`assurance-s5-crash-16661be5.json` was promoted byte-for-byte (`cmp` exit 0).
Only producer-derived source commit, source tree and lockfile hash changed;
all boundary and scenario rows are identical. Historical bytes remain in Git
history. No qualifier algorithm, assertion, golden expectation, or provenance
field was hand-edited to obtain a pass.

`assurance-s5-ci3`: full local merge PASS, exit 0. Nextest: 649 passed / 47
skipped. Threaded tests, bootstrap/proof support, doctests/rustdoc, dependency and
advisory checks, inventory, canaries, resource run/lint, and final assurance check
all passed. `assurance-s5-gates1` exited 0, confirming +3 active and no ignore
flips. Service runtime was 4m02.288s, peak 4 GiB under the 4-GiB cap, zero swap;
this single cache-unknown observation is not a performance budget or guarantee.
Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.a8pswrrm/merge/receipt.json`.
It correctly binds source commit `16661be5` with a dirty flag for the refreshed
evidence file; it is local development evidence, not qualification. This final
documentation update follows that run. S5's remaining implementation and hosted
execution remain open.

## Resolved gap: cryptographic batch lifecycle (protocol §3)

The approved human/tool boundary defines who authorizes a batch and what scope
it binds. It does not yet specify whether that authorization is part of the
standing policy's exact signed bytes or a separately signed batch manifest.
These are different activation/revocation contracts, not interchangeable JSON
encodings. Implementation paused for Brian's choice; he subsequently approved
option 1 explicitly ("approved 1."). Separate batch signatures and external
active-batch pins are now authorized for implementation, not activated.

Options:

1. Separate human-signed batch manifest (selected and approved). Keep standing policy's
   approved `liminal.assurance.policy.v1` namespace. Batch namespace:
   `liminal.assurance.batch.v1`. Manifest binds standing-policy digest, base
   revision, exact permitted targets/change class, and trusted tool revision and
   executable digest. External human trust configuration selects the active
   batch digest; missing, mismatched, or revoked activation refuses admission.
   Adding a batch does not change or re-sign the standing policy. This namespace
   and activation contract are approved for implementation, not enrolled.
2. Unselected alternative: include batches in the standing policy's signed bytes. Every batch addition
   changes the policy digest, requiring human re-signing and external re-pinning
   of that entire policy. No separate batch-signature namespace is introduced.

No signed-batch acceptance code or tests were added while this choice was open.
A bounded MiMo fixture draft was requested with a provisional embedded-batch
shape; that external draft is not a contract or implementation and must not be
adopted until reconciled with the decision. Existing proposal code and its
verification remain unchanged. No real keys, signing or trust activation occurred.

Next slice uses the already approved `amend check` seam with an explicit
`--authorization-only` mode. It authenticates exact policy and separate batch
bytes, signer, repository scope, active digests, tool pins, base and target.
Success reports batch authentication only, `apply_authorized: false` and review
not established. Full check/apply must remain unavailable until registry/patch
and independent-review checks exist. CLI fixtures use disposable SSH keys only.

### Authorization-only implementation evidence

Implemented the approved separate-signature contract. No real enrollment,
signing, activation, full admission or apply capability was created. There are
30 CLI controls, including 11 new active controls on Unix; no ignore flips.

Durable logs are under `/mnt/4tb/liminal-formal-evidence/reviews/`.
`assurance-auth-red1` exited 101 before the CLI existed; green1 through green3
passed the growing control set. Lint1 failed on function size and a stack buffer;
private helper extraction and an 8-KiB streaming buffer resolved both, without
waivers (lint2 exit 0). Full ci1 passed 659 tests with 47 skipped.

A real separate-Git-directory probe then exposed pathname whitespace loss.
`assurance-auth-red2` reproduced it; Git output framing now removes only its
newline, preserving the path's trailing space. `assurance-auth-green4` passed
30 controls. Full ci2 failed on two missing test-closure semicolons; these were
fixed without changing assertions. Failed-run logs remain preserved.

Final `assurance-auth-ci3` full merge profile exited 0: 660 passed / 47 skipped,
all 14 commands passed, 3m34.732s runtime, 3.5-GiB peak, zero swap. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.Bj7IUnWQ/merge/receipt.json`.
It records dirty source atop `98210d76` and qualification not established.
`assurance-auth-gates` exited 0: 660 active / 47 deferred. This documentation
update follows verification. Timing is one cache-unknown observation, not a
performance guarantee.

LAMU diff review returned primary PASS WITH NITS; critic output was truncated,
so no critic clearance is claimed. Source verification rejected claims about
cwd-based scratch (uses OS temp), PID-only collisions (counter and time also
participate), the intentional one-byte overflow probe, incomplete GIT_ prefix
removal, sibling-directory containment, and standalone-repository rejection.
The real positive fixture demonstrates standalone acceptance. Diagnostic style
nits are nonblocking. Final commit still requires its own review.

S5 remains open: closed registry and candidate/patch binding, independent review
receipt verification, isolated apply and producer refresh are not implemented
by this slice. Hosted S4 evidence and S6 remain open. No Phase 1 authorization,
M17 closure, or HAQP qualification follows from these local checks.
