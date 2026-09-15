# DG17.6 host-buffer construction evidence

Development baseline: `9b34d10599ecd1ba26fb334115838f64e5631384`.
This slice implements only fallible, lossless input storage. It establishes no
formal obligation and does not yet alter admission or ILRP execution.

`OrderProofInput::try_new` checks each layout and aggregate size arithmetic,
reserves four buffers fallibly, and fills them without further growth. The
zero-sized `ResourceExhaustion` carries no allocated message. Map/declared IDs,
operation classification, dependency duplicates, and both caller schedules are
preserved, including invalid facts. Construction grants no authority.

## Controls and review

Durable raw artifacts are under
`/mnt/4tb/liminal-formal-evidence/reviews/`:

- `dg176-buffer-red.log`: launcher exit 127 from a stale Cargo path, not RED.
- `dg176-buffer-red-v2.log`: compiler exit 101 for the missing module before
  implementation; the first literal translation control's RED.
- `dg176-buffer-green.log`: exit 0, initial translation control passed.
- `dg176-buffer-coverage.log`: exit 0, three public controls passed: empty
  inputs, invalid/duplicate/full-width identities, and graph/file classification
  with independent literal orderings. Later coverage controls were not RED.
- `dg176-buffer-clippy.log`: exit 101 for a new test style diagnostic;
  `dg176-buffer-clippy-v2.log` and `dg176-buffer-coverage-clippy.log`: exit 0
  after that exact correction.
- `dg176-buffer-regression.log`: exit 0, nine admission and seven ILRP driver
  controls passed. They do not test new resource-error propagation.
- `dg176-allocation-control.log`: exit 0. An external standalone System
  allocator harness refused reservations 1 through 4 separately. Each call
  returned `ResourceExhaustion`; the successful control observed four
  allocations and exact translated contents. No huge allocation or production
  fault switch was used. Two unused-variable warnings remain in the raw log.
- `dg176-diff-review.json`: fresh LAMU review completed with process exit 0,
  primary PASS and critic PASS WITH NITS. Verified disposition is in
  `dg176-diff-review-disposition.md` beside it.
- `dg176-final-tests.*`: launcher exit 101, systemd working directory omitted;
  no tests ran. `dg176-final-tests-corrected.*`: exit 0 with the explicit
  isolated working directory, all 19 focused tests passed (9 admission,
  7 ILRP driver, 3 buffer). `dg176-final-rustfmt.*`: exit 0.

Reviewer suggestion to cap the sum at `isize::MAX` was rejected: the four
vectors are separate allocations, each with its own checked layout. The sum
uses checked `usize` arithmetic, not one combined pointer-offset domain.
Reservation guarantees sufficient capacity, not exact allocator capacity.
A panic test would not establish the required typed-refusal behavior.

Compute limits: CPU quota 200%, 4 GiB memory, no swap, 256 tasks, 600 seconds.
These observations are development evidence, not qualification receipts.

## Remaining boundary

Complete executable ordering proof, admission/Prepare/recovery propagation,
precedence and no-effect fault controls, and full verification remain pending.
The proof-source inventory predates this new module/test and changed `lib.rs`.
Reconstruct and bind a new committed source projection before any new proof;
do not refresh historical cold-replay artifacts or claim they cover this diff.
No whole-core proof, full CI, HAQP, suite ratification, or phase GO is claimed.
