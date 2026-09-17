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
- [x] S5: `assurance amend propose|check|apply`: parser-backed unchanged exact
  targets, independent review, externally pinned SSH policy/tool trust, isolated
  apply and producer-derived refresh. Disposable-key controls cover signatures,
  namespace, scope, revision, source/patch drift, ambiguity, protected fields and
  interruption. No human key or trust activation needed for tests.
- [ ] S6: Complete merge checks, inventory, canaries and resource controls; named
  test deltas and independent diff/commit findings verified at cited lines. Human
  enrollment and hosted execution remain explicit external boundaries.

Checklist interpretation: S5 is complete as local implementation and test
evidence. External trust enrollment is deliberately not part of that local
closure. S4 remains open for observed hosted parity. S6 remains open for its
final whole-system review and any hosted/external evidence; its registered local
merge checks are green in the receipts below.

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

S5 remains open: registry/candidate binding is only a read-only partial seam;
independent review receipt verification, unchanged mutation behavior, isolated
apply and producer refresh are not implemented by this slice. Hosted S4 evidence
and S6 remain open. No Phase 1 authorization, M17 closure, or HAQP qualification
follows from these local checks.

### Post-review concurrency correction

LAMU's review of `5a12a636` identified a real pipe deadlock in synchronous
OpenSSH verification: a payload over the child stdin pipe could block the writer
before `wait` allowed the child to drain it. `5dc7e374` moves the exact payload
write to a joined writer thread and adds a bounded disposable-key regression with
a 131-KiB signed batch. The targeted test and all 31 assurance integration tests
pass. LAMU reviewed `5dc7e374` and returned PASS WITH NITS; the only remaining
notes are non-actionable allocation/polling style suggestions. The prior review's
serde indexing concern was verified as safe and recorded as a skipped false
positive. No authority or apply behavior changed.

### Registry and exact candidate binding slice

The authorization-only check now accepts an optional complete candidate group:
candidate commit, source path, old line and exact anchor. Partial groups refuse.
The executable carries the 65-entry packet-derived closed registry snapshot,
bound by its externally pinned binary bytes. A bound request verifies the base
source coordinate, the proposal's unchanged enclosing function and nonblank
source, candidate packet target movement, exact packet one-line source replacement,
and a two-file modification set. File metadata changes, extra packet edits and
unrelated tree edits refuse. Output remains `apply_authorized: false`.

Disposable fixture controls cover one valid coordinate move, extra packet content,
and unrelated tree content. The positive fixture uses a whole-line expression
anchor (`P1-M008`) and a packet source line with a trailing comma, exercising the
real JSON formatting shape. No production packet, source, frozen qualifier,
held-out corpus, or accepted SAS bytes were touched.

LAMU reviewed commit `944532f6fbd247eaa862a938b59b994a2525ffb9` with MiMo V2.5
Pro max and returned PASS WITH NITS. The source-diff question is intentional:
the proposal already rejects changed nonblank source and changed enclosing code;
full mutation-patch behavior remains a later S5 seam. Other notes concern JSON
output readability, the fixed coordinate-line comma convention, and a fixture
line constant. None changes security or correctness; no finding was applied.

Final committed verification at `29c5480794bc39e44b517d93b8ff13df8b4c1534`:
`assurance-s5-reg2` merge profile passed all 14 commands. Nextest reported
665 passed / 47 skipped; source was clean (`source_dirty: false`), qualification
remained false. Runtime was 4m09.192s with a 4-GiB peak and zero swap under the
bounded systemd service. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.KVPBqtRg/merge/receipt.json`.
`assurance-s5-reg2-gates` exited 0 with 665 active and 47 deferred. These are
development checks on the maintenance branch, not hosted, HAQP or Phase 1
qualification evidence.

### Independent review receipt and isolated apply slices

Receipt binding is now implemented in `edef62c3`. `amend check` accepts an
optional external review receipt only with the complete candidate/source/line/
anchor group. It parses a strict schema, constrains reviewer/backend labels,
binds exact base/candidate/target scope, recomputes the fixed two-path raw Git
patch digest, requires a pass with zero unresolved verified findings, and checks
finding identity/classification/resolution. Output changes to
`independent_review: receipt-verified` only after these checks; it remains
`apply_authorized: false`. The positive control and substituted, unresolved,
unreproduced, outside-root and incomplete-group refusals all pass.

LAMU reviewed `edef62c3` with MiMo V2.5 Pro and returned PASS WITH NITS. The
review's three style notes were addressed or recorded: coordinate/review
coupling is intentional and documented; the fixed two-path scope is deliberate;
the candidate is already validated by the registry proposal before receipt
diffing; enum boxing is private CLI layout; broad printable identity labels are
safe in this evidence-only output and remain a nonblocking style note. Receipt
review artifact:
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-edef62c3-review-r2.stdout`
(SHA-256 `42bf6ec0bb478d27c1c9de11e37e8cd297e4d640576f197cf5e7ff8905bf1377`).
Follow-up `92383b17` added the missing incomplete-group control; LAMU returned
PASS with no findings (`assurance-92383b17-review.stdout`, SHA-256
`a9e61e5da98f3ca9c2510cb76e5340c6dfef8917110a2d40178c4d1368f0c311`).

Isolated apply is implemented in `0eda47b8`. It requires the signed batch and a
verified review receipt, creates a fresh external detached worktree at the
authorized base, applies only the exact source/packet patch, compares the
staged tree byte-for-byte with the candidate commit, rejects untracked output,
and derives packet digest through the existing producer. It leaves the source
checkout untouched and explicitly reports no commit, push or signature. Output
paths must be new, absolute and outside every repository worktree. `4b68a342`
hardens non-NotFound destination errors, drains bounded stderr without pipe
deadlock, scopes the real packet fixture to the apply test, and adds panic-safe
test cleanup. `f13a1c4f` documents wait-before-join ordering and covers output
inside repository scope. LAMU returned PASS WITH NITS for `0eda47b8`
(`assurance-0eda47b8-review.stdout`, SHA-256
`ce36652c9392750d06d0205f4b3ff6e79dd092b4edc7d7d140627800517f2d75`), PASS WITH
NITS for `4b68a342` (`assurance-4b68a342-review.stdout`, SHA-256
`3ede2eb0e290b3f344b788e27790d1676c066ded055b7b6896ee24b623b6be26`), and PASS
with no findings for `f13a1c4f` (`assurance-f13a1c4f-review.stdout`, SHA-256
`d4acc5d72a60131008cc859914a3e925f1a40a27bd2822c682f5846d445fe82f`).

The critic's `0eda47b8` concern that `git apply` might use the source checkout's
index was checked against the actual call (`output` is passed as the apply root)
and the positive control's clean source index; it is a false positive. The
`4b68a342` concern that the 4-KiB stderr cap could deadlock was checked against
the reader loop, which continues draining after truncating captured bytes; it is
also a false positive. Both dispositions are retained in follow-up commit
messages. The `--authorization-only` flag remains explicit on apply as a
fail-closed caller acknowledgment required by the existing authorization seam.

Current assurance integration controls: 42 pass in the complete targeted binary;
the known parallel fixture race can emit `ExecutableFileBusy` once, and its
named test passes on isolated rerun. Full bounded merge verification and final
producer-derived refresh remain next. These slices still do not establish
mutation equivalence, HAQP qualification, hosted parity, Phase 0 closure or
Phase 1 authority.

### Full verification after isolated apply

`assurance-s5-apply-final-r2` ran the complete Linux merge profile under an
explicit systemd user unit with 2 CPU, 4-GiB memory, zero swap and 256-task
limits. Unit exit was 0; service runtime 2m56.484s, CPU time 4m02.622s, memory
peak 4 GiB and swap 0. All 14 registered commands passed, including style,
lint, bootstrap, proof-support, nextest, threaded tests, doctest/rustdoc,
documentation, dependency policy, inventory, canaries, bounded resource run,
resource lint and the final assurance check. Nextest JUnit reports 672 passed,
zero failures and 47 skipped. Receipt is
`/home/brianklam/.local/state/liminal/assurance/run.YA8zDD5R/merge/receipt.json`
(SHA-256 `c71f8c23230ba2ee6272118e82cf77a34c5a2496d0f628dabe64c5c5c6950646`);
it binds clean source commit `8b08c22e` and `qualification_established: false`.
Durable service logs (the merge unit intentionally emits status on stderr, so
stdout is empty):
`assurance-s5-apply-final-r2.stdout` (SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) and
`assurance-s5-apply-final-r2.stderr` (SHA-256
`da3e98a24a03ae06863459112da0b90aed2732e1404431c4aecfc8005909de43`).

`assurance-s5-apply-final-gates` exited 0 with 672 active and 47 deferred;
stdout SHA-256 is
`4491b660fdbe1390b7acd7ce1ba8950100fc6ab0b8150122243c730c24f52a11` and
stderr SHA-256 is
`0a3290d4e86c01c9764b0551f6af86740c2d81aacd582f5bfb3268a6ce75ef73`.
The one earlier parallel fixture race was isolated and passed on rerun; no
source change or assertion weakening followed it. This is local development
evidence only: no hosted parity, mutation equivalence, HAQP qualification,
Phase 0 closeout or Phase 1 authority follows.

### Current-HEAD bounded rerun

After the log-capture clarification, `liminal-assurance-s5-current-r3` reran
the same complete merge profile at clean source commit
`359f0f7f4600a89809b048c281ac232a47eb7938`. Unit exit was 0; runtime was
3m06.485s, CPU time 3m46.072s, memory peak 3.4 GiB and swap 0 B under the
same 2-CPU/4-GiB/zero-swap/256-task limits. All 14 registered commands passed.
Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.7kF7CxdR/merge/receipt.json`
(SHA-256 `6a77c2ea2c4620208c76216cecd0d8dc3900db5e0c8275964c74e70b7cbf9ce5`);
it records `source_dirty: false` and `qualification_established: false`.
Nextest JUnit reports 672 passed, zero failures and 47 skipped. The bounded
`just gates` rerun exited 0 with 672 active and 47 deferred; stdout SHA-256 is
`4491b660fdbe1390b7acd7ce1ba8950100fc6ab0b8150122243c730c24f52a11` and
stderr SHA-256 is
`d660316325a055dbaabca1abb2246b644c120e48694abcaa0c39f034751a52d1`.
This closes current-HEAD development verification only; hosted parity,
mutation equivalence, HAQP qualification, Phase 0 closure and Phase 1
authority remain unestablished.

### Accumulated implementation review

The accumulated delta from baseline `54b6450c15f161978ad63cc0f999d5090bc577f4`
through `6daecfc2844439d0112c8ff31c91b432a8c3e5f0` received a durable MiMo
V2.5 Pro LAMU `review_diff` result: `PASS WITH NITS`. Artifact
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-accumulated-review-r2.stdout`
has SHA-256
`960809a01eed038d5d3a82da0c721a3a77c35c15ac18b345e0cbe3c1cfc3f830`;
transport stderr has SHA-256
`048a62198fe06cb1214de58e3d4b1592731bf461212c464b9c264f82e4134284`.
Primary review notes are nonblocking style observations. Critic findings were
checked at source: path containment uses component-aware `Utf8Path::starts_with`,
`.lines()` removes diff framing newlines, source extraction retains the final
non-newline line, and the trusted-host worktree model is documented rather than
an omitted same-user isolation guarantee. No security or correctness finding
was confirmed; no source change followed this review.

### Final current-HEAD merge receipt

After the accumulated review record, `liminal-assurance-final-r4` reran the
complete merge profile at clean source commit
`799706c918a5aa203fa1f2fa5540da12e5fe194c`. Unit exit was 0; runtime was
2m52.281s, CPU time 3m45.132s, memory peak 3.1 GiB and swap 0 B under the
2-CPU/4-GiB/zero-swap/256-task envelope. All 14 registered commands passed.
Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.0qTR4ANK/merge/receipt.json`
(SHA-256 `d4b15886904866fff4130ac15bfd99c908b02d506864407d23443d05f264db2c`);
it records `source_dirty: false` and `qualification_established: false`.
Nextest JUnit reports 672 passed, zero failures and 47 skipped. The bounded
`just gates` rerun exited 0 with 672 active and 47 deferred; stdout SHA-256 is
`4491b660fdbe1390b7acd7ce1ba8950100fc6ab0b8150122243c730c24f52a11` and
stderr SHA-256 is
`aa6c5cb269f8e3dfe3eec74e42545d0c7c5e07daa8fee20836a540eba91f4e91`.
This is the final local development observation for the current implementation;
hosted parity, mutation equivalence, HAQP qualification, Phase 0 closure and
Phase 1 authority remain unestablished.

### Post-control full rerun and environment correction

`liminal-assurance-final-r5` is retained as an infrastructure-unavailable
attempt at source commit `3db6b39f80b7d055b95249e92217418908779d77`: style,
lint, bootstrap and proof-support passed, then nextest stopped with four Pandoc
tests reporting exit 127 because the systemd-launched test environment did not
resolve the installed pinned binary. Its receipt is
`/home/brianklam/.local/state/liminal/assurance/run.NCL1apeH/merge/receipt.json`
(SHA-256 `1e416b2ec21bf0bd41e9f593abf072e532cee0d80c49024ab83c3f0a253e13b2`).
The same Pandoc test passed in a direct probe and `/usr/bin/pandoc` reported
`pandoc 3.10.2`; no source, golden or assertion change followed.

`liminal-assurance-final-r7` repeated the complete merge profile at the same
clean source commit with the inherited process `PATH` explicitly passed into
the bounded systemd unit. Unit exit was 0; runtime was 2m29.650s, CPU time
3m44.248s, memory peak 2.1 GiB and swap 0 B under the 2-CPU/4-GiB/zero-swap/
256-task envelope. All 14 registered commands passed. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.RA2TPR6T/merge/receipt.json`
(SHA-256 `4db6b5bc251f5cce1e24b1e55eba89b3c3443a637e004a27679f01442be35f6f`);
it records `source_dirty: false` and `qualification_established: false`.
Nextest JUnit reports 673 passed, zero failures and 47 skipped. The bounded
`just gates` rerun exited 0 with 673 active and 47 deferred; stdout SHA-256 is
`fa12b0c533e4b10fdf60336672fe15bcba4419badbfaa929f840aca57e38e96b` and
stderr SHA-256 is
`433c0fc81706bd1d34fc864dcb4795bb91870fe1ec3a95a502e7175b86ab43c9`.
This remains local development evidence only; no hosted, mutation, HAQP,
Phase 0 or Phase 1 qualification follows.

### Hosted parity attempt

Draft PR `#7` on `assurance-maintenance` triggered CI run `35176289912` for
head `80073e1e825aff245eff7bb0a4e8599d1a3a89c7`. All seven declared jobs
completed as failures within seven seconds, with zero runner steps and no
runner assigned. GitHub's check annotation records that the job was not
started because recent account payments failed or the spending limit needs to
be increased. This is unavailable hosted infrastructure, not a source or test
failure; no hosted command executed. Hosted parity remains unobserved and the
external billing/account action is outside this worktree's authority.

### Final accumulated review after boundary control

The accumulated delta through `49808acca2a172e7106beab9fcabc53b91e8a8a2`
received a second durable MiMo V2.5 Pro LAMU `review_diff` result:
`PASS WITH NITS`. Artifact
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-accumulated-review-r3.stdout`
has SHA-256
`9f394658d3ca7fc8d809ae9cd715180f04b1dbef89acbcb895dd0260c62e6f9e`;
transport stderr has SHA-256
`67fd26d7c2cd5c8a0877825a758c2d191665f2b9c2e94ed9b3f76ee902282e95`.
Primary notes are nonblocking style/documentation observations. Critic review
confirmed no security defect: path containment is component-aware, Git diff
framing is line-safe, and the trusted-host model's same-user limits are
documented. No source change followed this review.

### Current-head verification after token binding

At clean source commit `3c363482223b296f358c52ce7f03c810bb49903a`, the bounded
Linux merge profile completed successfully. All 14 registered commands passed;
nextest reported 675 passed and 47 skipped, with no failures. The outer
systemd service used the standing 2-CPU, 4-GiB, zero-swap and 256-task envelope;
runtime was 14m43.139s under host contention, CPU time 14m46.528s, and the
service-reported memory peak reached the 4-GiB cap. This is one cache-unknown
development observation, not a budget or performance guarantee. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.vDvK9IdD/merge/receipt.json`
(SHA-256 `b3b769fae7636b06f8f2ea2f052a0c96a6b676bd25b6bef9f1201568212c8883`).
The receipt records `source_dirty: false` and
`qualification_established: false`.

An overlapping manually launched nextest probe was not evidence: it shared the
checkout target directory with the bounded merge unit and produced one
`maintenance executable pin mismatch` in an assurance fixture while its other
674 tests passed. The failure is the expected executable-replacement race from
concurrent Cargo jobs, not a source result; no source or assertion change
followed it. Do not run overlapping jobs against one target directory.

### Final current-HEAD verification after parser-stability hardening

At clean source commit `6bc9ce1350554f6fed5c1a4de7fcca1846f6e76e`, the bounded
Linux merge profile completed successfully. All 14 registered commands passed;
nextest reported 675 passed and 47 skipped, with no failures. The outer
systemd service used the standing 2-CPU, 4-GiB, zero-swap and 256-task envelope;
runtime was 3m31.055s, CPU time 4m19.774s, and memory peak 1.8 GiB. This is a
single cache-unknown local development observation, not a performance budget
or qualification claim. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.Z3WKD1Pr/merge/receipt.json`
(SHA-256 `12031aeb568993e1f7737a1d2e32659148e31518882502f07eec4c223e6942e0`).
It records `source_dirty: false` and `qualification_established: false`.
The bounded `just gates` rerun exited 0 with 675 active and 47 deferred.

At commit `c56c0e6b846c0f4fa7ef1e96519fc40e66cf4fde`,
`just assurance-workflows` exited 0 and regenerated no changes; the generated
workflow tree remained byte-identical. A same-commit `just gates` rerun exited 0
with 675 active and 47 deferred. These checks add local reproducibility
evidence only; hosted execution and external trust activation remain open.

The accumulated delta through this commit received a durable MiMo V2.5 Pro
LAMU `review_diff` result: `PASS WITH NITS`. Artifact
`/mnt/4tb/liminal-formal-evidence/reviews/assurance-accumulated-review-final.stdout`
has SHA-256
`c6b1c86674b046e458e5e2f08053da72dfe9a0c9a154febbcbf6d1df7a818ea5`; transport
stderr has SHA-256
`69fd061eaeb34aa3cb186761e4c017b71c55621214da30613ffcfbb8af4bb10f`.
The visible findings are nonblocking style notes; no security or correctness
defect was identified in the primary result. The critic response ended before
completion, so separate critic clearance is not claimed. No source change
followed this review.

The bounded full merge profile was then rerun at clean source commit
`fa39e2e7fc98887aea06297f388ffcbe72101842`. All 14 registered commands passed;
nextest reported 675 passed, zero failures and 47 skipped. The 2-CPU, 4-GiB,
zero-swap, 256-task systemd envelope observed 3m35.833s runtime, 4m23.899s
CPU time and 1.8 GiB peak memory. Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.o32WJ94n/merge/receipt.json`
(SHA-256 `1f18e6764797b34cf6565c6d0346b83a00e121e5b9dfd983e54eab5dd569f9e0`).
It records `source_dirty: false` and `qualification_established: false`.
The bounded `just gates` rerun exited 0 with 675 active and 47 deferred.

### M17.5 packet correction and clean-head verification

At `3aa74f07160283057d87f83122e570fabb559791`, the M17.5 packet metadata
for P1-M001 was corrected to its actual M19 requirement; `haq derive-killers`
then confirmed its declared killer-test set (`P1-T03`, `P1-T13`). The full
fixed-base HAQP campaign remains documented as F-74 in
`docs/execution/m17-5-adversarial-findings.md` at that commit; its final exit
was fail-closed on five pass-1 findings, so it does not establish
qualification.
Canary evidence was regenerated against packet digest
`10d5fe46f196fc368c277aaf3a8c85de594bfff8666cd8513a447e0850ecbb15` and
committed at `7e5bb881433b14840e7267a49cd6b147b567d2c3`; all 33 canaries
caught their declared mutations.

The first clean detached merge attempt failed only in `threaded` (exit 101)
while nextest passed. A direct `just test-threaded` rerun at the same commit
passed all workspace targets, including 246 xtask tests and 45 assurance tests.
A subsequent clean `just ci` completed successfully with all 14 registered
commands passed, 675 passed (active) and 47 skipped (deferred), zero failures.
Receipt:
`/home/brianklam/.local/state/liminal/assurance/run.5FLLnQEb/merge/receipt.json`.
Receipt SHA-256: `92db157e03f97db9797ef9488b405591f4e26f0e255a8daca603cf090f25f642`.
It records `source_dirty: false`, `qualification_established: false`, and
resource peaks of 622,149,632 bytes for `resource-run` and 155,758,592 bytes
for `resource-lint`. This is local development evidence only; hosted parity,
external trust activation, M17.6/M17.8 governance, and HAQP qualification
remain open.

### Closure boundary

As of assurance-maintenance `9299768a` (`docs: clarify M17.5 verification
evidence`), local maintenance slices exercised by the clean receipt above are
implemented and complete. These slices are authorized under ADR-0022/AM-17.14;
the receipt proves the registered local merge profile only. It does not alter
the frozen HAQP instrument or establish qualification. The assurance branch
still preserves failed-campaign artifacts in its working tree, while the clean
detached run proved source-clean behavior.

Further progress requires decisions or external state, not more local retries:

- Frozen HAQP findings A01, A03, A11 and A12 need explicit scope rulings before
  any qualifier or authoritative registry edits.
- M17.6 Phase 0 GO/NO-GO, M17.8 meter reconciliation, and M17.9 suite review
  remain human gates.
- Hosted parity is unavailable until the GitHub billing/runner block is cleared.
- Automatic coordinate maintenance remains inactive until a human enrolls the
  external trust root, policy, batch and pinned tool. This branch adds no
  private-key material and maintenance runs do not read user key stores;
  disposable signing keys are generated only by test fixtures in OS temporary
  directories.
