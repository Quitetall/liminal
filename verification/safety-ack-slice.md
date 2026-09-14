# Candidate executable acknowledgement proof slice

Development slice based on `29f9f159`. Not qualified, adopted, or a complete
discharge of `ilrp-ack`. The completed baseline HAQP campaign remains bound to
its unchanged source in a separate checkout and refused the final packet flip.

The `liminal-safety` dependency leaf implements exact repair-step identity and
dependency-acknowledgement membership over lossless 128-bit identity values.
Its Verus postcondition characterizes both acceptance and refusal for all input
slices; loops carry bounds, membership invariants and decreasing variants.
There is no `assume`, `admit`, `external_body` or separately implemented verified
twin in this crate. It borrows slices without allocation and is `no_std`.

`liminal-jurisdiction::validate_ack` calls that executable body. The first call
retains the original identity-error precedence; the existing structural
poststate comparison remains next, followed by the dependency check. Mapping
UUIDs to all 128 bits and selecting the matching dependency/key entries are
ordinary Rust adapters, not newly proved translations. The checker, intent
history, poststate truth, graph finalization and syscalls remain outside this
narrow theorem. Each dependency uses the existing ordered-map lookup and a
stack singleton/optional slice; no collection allocation is introduced. The
initial vector-based draft was rejected during Spec review for changing
allocation-failure behavior. Its review and historical test logs are retained,
not represented as reviews of this corrected binding.

## Observed controls

- Intentional false stub: positive literal test failed, exit 101.
- Correct body: ordinary runtime controls passed (two tests, an eight-row
  positive/negative/boundary table plus the original positive); the no-thread import compile-fail
  doctest passed.
- Pinned Verus full leaf verification: 1862 vstd checks and four leaf checks
  verified, zero errors, with vstd default features disabled. The earlier
  default-feature proof had 2045 vstd checks and the same four leaf checks.
- Disabling dependency validation in an isolated body made a runtime test fail
  and Verus reject a loop invariant (three verified, one error; exit 101).
- Corrected stack binding: nine admission and seven ILRP production tests
  passed, exit 0 (`safety-ack-stack-production-tests.log`); targeted Clippy also
  passed. The existing identity-refusal test killed the stack-binding mutant,
  exit 101 (`safety-ack-stack-binding-mutant.log`).
- Disabling identity validation in an isolated copy of the production binding
  made the unchanged `ilrp_rejects_mismatched_ack_before_persistence` test fail
  at its existing refusal assertion (exit 101). The final no_std copy was
  checked for source drift before/after the run.
- Ordinary and verified release witness builds used the same leaf source and
  example source. Both executables returned `acknowledgement-witness:5` and
  exit 0. The ordinary example harness is not a proved function: its zero-check
  Verus report is not counted as proof. The library is the four-check proof.
- Enabling vstd's default `std` feature in an isolated inverse control made
  the no-thread compile-fail doctest fail because the import compiled. This
  confirms the control observes the feature restriction, not a misspelled name.

Raw development logs and exact mutated copies are retained beneath
`/mnt/4tb/liminal-formal-evidence/reviews/` and `probes/`. These manually observed
commands are not a pin-enforcing production qualification runner or cold replay
receipt. Existing `formal-proof`, `formal-model`, `formal-adapters` and
`formal-gate` must continue to refuse incomplete evidence.

## Constrained dependency admission

The unchanged concurrency registry rejects the new exact `vstd` dependency.
Its default feature really does export `vstd::thread::spawn`; the candidate
therefore disables default features and its thread-import control detects
accidental re-enablement. The source guard is at the pinned vstd `vstd.rs:85-86`
and the wrapper at `thread.rs:107-121`.

Brian approved constrained admission in DG17.5 on 2026-09-14. The candidate now
checks locked, offline, all-feature Cargo metadata before scanning any real
workspace's lexical dependency keys. Exactly one pinned crates.io vstd package,
an empty resolved feature set and exact/default-disabled/featureless dependency
declarations are required. Renaming vstd to an already allowed dependency name
cannot evade the metadata check. All other unknown-name refusals remain.

Thirteen focused controls passed: twelve metadata/live-Cargo tests and a
scan-hook alias canary. Each live inverse first requires successful Cargo
resolution, so a broken Cargo invocation cannot masquerade as a caught drift.
The initial key-triggered draft had an alias bypass; independent review found
it, and re-review confirmed the unconditional workspace trigger closes it.
The frozen scan is changed only under this recorded dependency-admission
exception; thresholds, corpora and excluded-crate lists remain unchanged.

The ordinary macro expansion of this exact leaf was inspected in all 47 lines;
it emits the expected two loops and no thread/process calls. This diagnostic
is not a universal proof about all macro invocations or a qualified build.
Detailed review dispositions and expansion hash are retained in the external
`reviews/safety-ack-review-disposition.md` evidence record.

Broader verifier development run: 245 passed, one crash-replay failure. A
focused reproduction found the frozen harness searches workspace target paths
despite a custom `CARGO_TARGET_DIR`; switching to the supported workspace target
let the crash matrix run and exposed the expected old/new lockfile digest
mismatch. No harness predicate or recorded digest was weakened. Fresh producer
evidence at a committed source and a full fixed-source rerun remain required.

Still needed: complete source/tool/executable binding and cold replay, proof
evidence runner, further protocol/store/Basis/authority obligations, finite
models and qualified adapters, independent reviews and human adoption.
