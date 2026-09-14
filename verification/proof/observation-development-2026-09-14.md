# Verifier observation reader development record

This record covers the development-only `parse_verus_output(raw: bytes)` seam.
It parses complete pinned-Verus stdout and returns ordered observations; it does
not authenticate source, tools, commands or metadata, assign proof PASS, qualify
a phase, or discharge an obligation. The reader is not yet integrated into the
formal commands, and the broader work in `../PLAN.md` remains open.

## Contract exercised

Input must be exact `bytes`, at most 2 MiB, with no more than 64 ordered events
and at least one JSON report. The reader never truncates an over-limit stream.
It accepts canonical u64 summary and JSON counters. Whole summaries retain the
original summary shape; only the exact emitter suffix
`` (partial verification with `--verify-*`)`` adds `partial: true`. Near-miss
suffixes and glued events refuse.

Each report is an object with the exact `func-details`,
`verification-results`, and `verus` sections. Conditional result shapes retain
ordinary failures, VIR failures, partial verification, entire-crate results,
and zero-check reports without interpreting them as qualification. The default
no-time metadata/report shape is closed and typed. Metadata strings are
self-reported observations, not admission pins.

Function entries have exactly `obligation_proof_notes` and
`failed_proof_notes`. Each is an array of Unicode scalar strings corresponding
to the emitter's per-field string set. Input order is preserved because emitter
set order is not guaranteed; duplicate decoded strings within one field refuse,
while the same text may occur once in each distinct field. Malformed UTF-8,
JSON, duplicate members, lone escaped surrogates and non-finite or out-of-range
numbers normalize to `ObservationFailure`. Every returned result has
`status: output-parsed` and `qualification: false`.

The schema choices were checked against pinned Verus source at commit
`b432e82fed7e05090fd53b5e5fc39020f725aabe`: JSON assembly in
`source/rust_verify/src/main.rs:519-554`; result/note definitions in
`source/rust_verify/src/verifier.rs:293-294` and `:350-369`; summary emission in
`source/rust_verify/src/verifier.rs:3348-3362`; and serialization support in
`source/rust_verify/src/util.rs:218-232`. These anchors explain the parser
contract but do not prove that a retained binary came from that source.

## TDD and support evidence

Fifteen public-interface controls were developed as serial RED/GREEN slices.
The durable logs are under `/mnt/4tb/liminal-formal-evidence/reviews/`, including
the `observation-*-red.log` records and the final
`observation-fifteen-support-green.log`. The detailed progression, including
hashes and corrected interpretations, is retained in
`observation-continuation-2026-09-14.md` in that directory.

Two test-fixture corrections were preserved rather than hidden. VIR-positive
fixtures initially included `is-verifying-entire-crate`, which the pinned emitter
omits on VIR errors; the parser was not weakened. The error-overflow and
error-float cases were then isolated from the separate `success` consistency
rule so their refusal specifically exercises numeric validation. Earlier setup
and timing mistakes remain identified in the continuation record.

After the fifteenth slice, full proof-support discovery ran 121 tests in 5.550
seconds and exited 0. Its log SHA-256 is
`da172df1c5751b0296bb57ca13227ad033cd9b8a7f8adb3565f33f657c6d41d6`.
The parser and test hashes were identical before and after that run:

- `observation.py`: `03aea2dbed70a042dd8bc09a5ab8906b21838940bc23287565b5d30d23df2e1d`
- `test_observation.py`: `5a1fb109d69ab95147c5edb7fdf8066d53c923c38c8a15657003f405cf731458`

This was the support suite, not full CI or qualification. Retained real-output
replay v2 used an earlier parser hash and predated the final Unicode-scalar and
note-set fixes. Its preparation also included a preserved script syntax error
caused by an accidentally pasted shell hash line; that operator setup failure
occurred before parsing and was corrected without changing the retained inputs.

Replay v3 then parsed the same four retained stdout artifacts against the final
reader without running Verus. It exited 0, retained `qualification: false`, and
held the parser and test hashes above unchanged before and after. It preserved
the loose 1/0 report, strict VIR failure, witness 4/0 plus empty 0/0 report, and
mutant 3/1 failure. The artifacts are:

- `/mnt/4tb/liminal-formal-evidence/probes/observation-real-replay-v3/replay.py`,
  SHA-256 `239889d44a87dd903c95f913d3fcae7c684613535e594db8530bf42fd9fd866d`;
- `/mnt/4tb/liminal-formal-evidence/reviews/observation-real-replay-v3.json`,
  SHA-256 `f7088fa5a29013e879669c19e45ceb75f733be22435caa66853397fb98c72eee`;
- `/mnt/4tb/liminal-formal-evidence/reviews/observation-real-replay-v3.log`,
  SHA-256 `b48147f0715ba515fce91151f31db9f6b45a412d9020ab42a59a3f799271a295`.

This establishes compatibility with those retained bytes only. It does not
authenticate their source or tool, rerun a verifier, constitute full CI, or
qualify any proof or phase.

## Integration checks and review state

The first bounded full-CI attempt stopped at the `typos` gate, child exit 2. The
only reported spellings were the abbreviated reverse-order labels for the
obligation and failed-note subtests. That CI attempt had not reached Python or
Rust tests. The labels alone were expanded to `obligation-reverse-order` and
`failed-reverse-order`, with fixtures and assertions unchanged. The resulting
current test-file SHA-256 is
`bff4e945d8a203603c7b074c28395691da8abdc9500793d5c235b50b2c4e9fea`.
Historical support and replay hashes above intentionally continue to bind the
earlier test bytes.

The second bounded full-CI run then completed with child exit 0 and equal source
before and after. It observed 620 Rust tests, 47 skipped, 22 bootstrap Python
tests plus 121 proof-support tests, and all 33 canaries. Runtime was 148.571
seconds; peak memory was 3,108,917,248 bytes under a 6 GiB maximum with zero
swap, and cleanup observed the transient unit absent. Evidence under
`/mnt/4tb/liminal-formal-evidence/reviews/observation-ci-v2/run/` has these
hashes:

- run log: `6eba9ac2be201a8499c364f77958a8ae236baa56b819354fafe3d1f73c25da48`;
- result: `8610e4cebc281c37543d47d4bed7eaa04fa7088c809c184bc70f865f3447ca85`;
- JUnit: `003a47a4a046a00fcadf4327ad5ab6c008a73a070e6297d6c6a5c6202a1434e0`.

This CI evidence does not change the reader's authority or qualification
boundary. Pre-commit LAMU review returned a primary `PASS WITH NITS`; its critic
response was truncated, so mandatory review of the eventual actual commit is
still pending. No commit-level verdict is claimed here.
