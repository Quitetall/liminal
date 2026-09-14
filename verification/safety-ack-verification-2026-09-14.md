# Executable acknowledgement slice: verification record

Source commit: `076e0e78dcaca233765e4b2eac923154d4591026`.
Source tree: `165f114c2fcf861cdcc38d164269e3cdce032984`.
Approval record: `c9471980` (DG17.5), externally reviewed PASS before integration.

**Full CI passed for this source slice. Whole-core proof and HAQP qualification
remain incomplete.** The sixteen formal obligations remain not established;
the acknowledgement membership theorem alone does not discharge `ilrp-ack`.

## Observed results

- `just ci`: actual `CI_EXIT=0`; 620 tests passed, 47 skipped. Threaded replay,
  doctests, rustdoc, dependency checks, inventory and all 33 canaries completed.
- Runtime acknowledgement integration: nine admission plus seven ILRP tests
  passed before full CI; final full CI includes the unchanged production tests.
- Dependency guard: 13 controls passed, including real Cargo resolution and
  an allowed-name alias at the scan entrypoint. Skipping workspace admission
  made that entrypoint canary fail after successful compilation, exit 101.
- Producer-derived crash evidence: eight registered boundaries exercised;
  eight single-step and ten DAG fault injections passed. Regeneration changed
  only lockfile/source bindings, not the recorded fault rows or outcomes.
- Producer-derived concurrency evidence: same scan result/synchronization list;
  only source commit/tree bindings changed. No new not-applicable exemption.
- Targeted Clippy passed with warnings denied. Source-bound Verus development
  evidence remains four leaf checks, zero errors; deliberately false bodies
  fail both runtime and proof controls as recorded in `safety-ack-slice.md`.

Leaf source SHA-256:
`a96cf145ef072b23916d7bc92f6c90cde82102a8d0451c2b361f591f3795c5ba`.
These observed proofs are not yet the planned pin-enforcing cold-replay runner.

## Execution envelope and artifacts

Full CI ran in the isolated checkout with source fixed throughout. Only
producer-written HAQP evidence differed from the committed source. The wrapper
checked source stability before/after, used the supported workspace target,
and recorded actual exit. Limits: two Cargo build jobs, four test threads,
four CPU quota, MemoryHigh 20 GiB, MemoryMax 24 GiB, no swap. Observed peak during
the run was about 7.5 GiB; this is a sampled observation, not the final maximum.

Raw files live under `/mnt/4tb/liminal-formal-evidence/reviews/`:

- `safety-ack-076e0e78-ci.log`, SHA-256
  `6a887fb2c07bb914917adb798a1ef94cf0ab9d799eb68497f9ecc07e27f32763`.
- `run-safety-076e0e78-ci.sh` records the fixed-source checks and resource inputs.
- `safety-ack-076e0e78-{crash-producer-r2,concurrency-producer,canary-producer}.log`
  retain actual producer output. The first crash invocation used an invalid
  xtask subcommand and exited 2; it is retained, not counted as a producer run.
- `safety-ack-076e0e78-commit-review.jsonl` contains a complete PASS main review
  and an incomplete critic response. The separate `-commit-review-fast.jsonl`
  contains a complete PASS WITH NITS review. No complete ensemble is claimed.

The reviewer suggestion to narrow vstd detection to its local alias would
reopen the tested alias bypass and was rejected. The claim that enabling std
makes the compile-fail doctest silently disappear is false: the explicit
inverse control makes the doctest fail because it compiles. Repeated identity
checks preserve error precedence. Ordinary offline dependency-cache availability
remains an explicit prerequisite; a missing Cargo resolution refuses admission.

The earlier baseline campaign at `29f9f159` ran all stages but refused its final
flip. See `haqp-29f9f159-result.md`; successful CI here does not clear those five
findings, sign their dispositions, adopt RFC-0002/formal obligations, or authorize
Phase 1.
