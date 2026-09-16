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

## Pending T1 boundary for S5

Do not implement acceptance of independent-review claims until Brian selects how
they are authenticated. Candidate-supplied `reviewed` metadata is insufficient.
Recommendation sent: the externally pinned trusted maintenance tool obtains
LAMU review itself and binds the receipt to the exact proposal; unavailable or
inconclusive review refuses apply. Alternative: separate human-signed review
attestation per proposal. Human standing-policy signing and external enrollment
remain required under either choice. Ordinary tests do not depend on this choice.
