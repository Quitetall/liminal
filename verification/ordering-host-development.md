# DG17.6 ordering host integration — development evidence

Baseline: `6331ef7be0a300e9d7ddce54ca8167caf9876e80` in the isolated formal
runner worktree. This work is not qualification, formal-companion adoption or a
phase authorization. This records the admission/Prepare/recovery development
slice prepared for integration into main; completed-history proof binding is
still pending.

## Admission slice

Human and automatic admission now prepare fallible ordering input and run the
allocation-free leaf after their existing refusal checks. Automatic admission
retains its checker evaluation and second head check before the new preparation.
Human acceptance retains its distinct caller-supplied actor route. Exhaustion
has a transparent typed `AdmissionError::ResourceExhaustion` variant, not an
allocated review reason. The existing topological order is retained rather than
recomputed. The stable execution partition is copied into the fallibly reserved
proof buffer without an intermediate infallible vector.

The private partition constructor is not validation or authority. Unknown IDs
remain present for leaf rejection; they are not removed or made into valid map
members. The public lossless constructor still preserves supplied orderings.
No workspace unsafe lint, production allocator, fixed size cap, UUID layout or
`vstd` feature is changed.

## Observed controls

Receipts below are in `/mnt/4tb/liminal-formal-evidence/reviews/`; each prefix has
separate `.stdout` and `.stderr` files with the actual terminal result. All jobs
used two CPU equivalents, a 4 GiB memory cap, zero permitted swap and a 600-second
runtime limit. A test allocator refuses a single targeted allocation, not host
memory exhaustion. The one-step proof-fact tuple's 48-byte size and 16-byte
alignment are asserted before arming; this is platform-bounded development
evidence, not a portable allocator identity theorem.

- `order-admission-red-r1`: exit 101 before integration, at the assertion that
  human acceptance must refuse. The old code instead returned acceptance.
- `order-admission-green-r1`: exit 0 after human integration; exactly one
  targeted allocation refused, exact resource-error display, unchanged head and
  no persisted intent. Peak memory 204.1 MiB, swap zero.
- `order-automatic-red-r1`: exit 101 before automatic integration. The unarmed
  positive succeeded; the armed route wrongly accepted.
- `order-automatic-green-r1`: exit 0 after automatic integration, including
  typed error matching, exact display, unchanged head and no intent. Peak memory
  171.7 MiB, swap zero.
- `order-admission-precedence-r1`: exit 0. Both routes first accept valid
  unarmed controls, then preserve the exact legacy key/identity mismatch reason
  with the allocation fault armed. Zero matching allocations occur on either
  legacy refusal. Head, original node payload and intent namespace are unchanged.
- `order-admission-regression-r1`: exit 0, nine admission and three buffer
  projection tests passed after the human slice.
- `order-admission-tests-r2`: exit 0 after both routes and formatting; 12 unit,
  nine admission, seven ILRP driver and three projection tests passed, plus one
  compile-fail doctest. This is the jurisdiction package, not full workspace CI.
- `order-admission-lint-r1`: scoped all-target Clippy with warnings denied,
  exit 0.
- `order-buffer-controls-r2`: setup refusal, exit 101. The historical scratch
  lock pinned `blake3` 1.8.7 while the pinned vendor contains 1.8.5. This was not
  an allocation-control failure. The historical lock was preserved.
- `order-buffer-controls-r3`: new scratch root, unchanged historical harness
  source and manifest, initial lock copied from the authoritative workspace;
  offline run exit 0. Four individual reservation refusals and the positive
  four-allocation control pass after the constructor refactor.

The human harness source SHA-256 is
`c83fe6bfd8c299a8313acea6e76463e2edcc5b3015173f8f10e2a9a0aa939e15`;
the automatic harness source SHA-256 is
`9525df763388bf88e9cd09772f1af581abf91e141f394dfdca2ff2ed727a2eba`.
They live in the correspondingly named probe directories under
`/mnt/4tb/liminal-formal-evidence/probes/`. Human probe lock resolution was
independently compared with the authoritative workspace: all 75 shared registry
package versions, sources and checksums matched.

## Diff review disposition

Actual LAMU `review_diff` receipt:
`order-admission-review-r1.result.json`. Source hashes were unchanged during
review. Primary verdict was PASS WITH NITS. The critic recommended a change
based on an incorrect reading of the partition iterator; it is not counted as
an independent pass. Each finding was checked against the supplied code:

- The first filter selects external/unknown entries; the chained second filter
  selects graph entries from the same immutable schedule and plan. Together
  they emit exactly `schedule.len()` entries, including duplicates. The critic
  omitted the second filter. No length mismatch exists.
- The defensive capacity guard returns an error before an extra push; it does
  not silently truncate a successful result. Current internal iterators have
  exact construction-derived lengths. No extra allocation is permitted.
- Unknown schedule entries remain invalid inputs to the full leaf. Replacing
  this with a panic would make the translation less robust, not more correct.
- Human and automatic routes intentionally differ in legacy checks. Human
  acceptance never performed automatic evaluation; its existing basis/head
  validation and later consume checks remain. A real allocation refusal is not
  caused by stale-store semantics, and moving preparation to `consume` would
  change the approved boundary.
- `topo_order` already allocated before later validation in the old code. Moving
  it after per-step checks would change error precedence. The new code retains
  its output, rather than introducing that prevalidation allocation.

The expanded review (`order-admission-review-r2.result.json`) included the
standalone test package. Its primary verdict was PASS WITH NITS; its critic
output ended mid-question, so no complete critic pass is claimed. The suggested
short-iterator false acceptance is contradicted by the executable leaf's initial
`applied.len() != schedule.len()` refusal at `liminal-safety/src/lib.rs:395`.
Both present private iterators also have exact construction-derived lengths.
No panic assertion is added. Retaining the old infallible topological allocation
is intentional under DG17.6; its allocation failure is not converted into a
review reason (only the existing DAG error is mapped that way). The new typed
resource error on human acceptance is precisely the approved behavior change.

No verified correctness defect resulted. Mandatory commit review remains due
when this source slice is committed. These reviews do not cover the later
Prepare change described below.

## Prepare slice

`order-prepare-red-r1` reproduced the missing Prepare resource boundary: exit
101 at the assertion requiring refusal, after the old Prepare accepted the
intent. The fixture has graph-only effects, and its external executor panics if
called. All fixture construction and human acceptance precede the fault window.

Prepare now reserves and checks its ordering input after all legacy refusals,
including duplicate-intent rejection, but before constructing/persisting the
intent transaction. `IlrpError::ResourceExhaustion` preserves the typed error;
the admission-to-driver conversion also explicitly preserves that variant
instead of allocating an executor string. `order-prepare-green-r1` exited 0:
one matching allocation refused, unchanged head and node payload, no intent.
Peak memory was 207.9 MiB with zero swap. This is one bounded public-boundary
control, not completed Prepare/recovery integration or full driver verification.
The Prepare negative and unarmed positive controls are now in the standalone
repository harness: `resource-controls-r3` exited 0 with `--locked --offline`.
The positive preserves the original repair ID and creates one intent; the
negative asserts the typed driver error. The jurisdiction package was rerun
after Prepare (`order-prepare-tests-r1`): 31 tests and one doctest passed, exit 0.

## Recovery boundary slice

Shared `advance` now checks illegal-state refusal first, then prepares and
validates ordering before any Applying/Finalizing transition or effect. The
same schedule drives Apply and is passed to the private finalization validator;
the latter does not allocate a new proof input after effects. Existing intent,
store, graph predicate and head checks remain. Completed-history proof binding
is not implemented by this slice.

`order-recovery-red-r1` exited 101 at the new typed-refusal assertion. Its first
diagnostic did not print the actual result, so the assertion gained a diagnostic
message without changing its condition. `order-recovery-red-r2` exited 101 and
explicitly showed the old path returned `Ok(... Committed)` with the fault armed.
Both failures are retained. `order-recovery-green-r1` then exited 0: one refused
allocation, unchanged head, exact persisted intent rows and payload, followed by
successful unarmed recovery and an empty idempotent recovery. Peak memory was
238.2 MiB; swap zero.

`order-recovery-applying-r1` additionally passed after a real driver
AfterAcknowledge cutoff. The control verifies both the reached flag and exact
panic payload, persisted Applying state and one acknowledgement; it does not
accept an arbitrary panic as a crash witness. `order-recovery-states-r1` passed
with both AfterAcknowledge/Applying and BeforeFinalize/Finalizing cutoffs. The
intent and graph payload remain unchanged on resource refusal, and each unarmed
recovery subsequently commits. Expected caught cutoff panics appear in stderr;
the overall command exits 0. Draft omissions (missing Prepare and an undeclared
JSON dependency) were corrected during source inspection before execution.

`order-recovery-states-r2` exited 0 with all four nonterminal states covered.
ExternalApplied uses the pre-existing trusted fixture-assembly pattern: after a
real Applying acknowledgement, append only the legal state transition with
`state:external-applied` provenance and no graph operations. It is explicitly
not a runtime crash witness. All cases compare the exact persisted rows and
head before/after refusal, then complete recovery with allocation available.
This graph-only fixture does not cover mixed external/graph effects or every
reservation site.

`order-recovery-tests-r1`: 31 jurisdiction tests plus one doctest passed, exit 0.
`order-recovery-lint-r1`: scoped all-target Clippy passed, exit 0. These are not a
replacement for workspace CI or qualification.

Actual LAMU `order-driver-review-r1.result.json` reviewed the three production
files with stable before/after hashes. Primary and critic verdicts were PASS
WITH NITS. Verified dispositions: the illegal-state guard is textually before
allocation; the local intent state cannot concurrently change through another
alias during this exclusive borrow; the schedule depends on the immutable plan,
not changing store state. Changing recovery to continue after exhaustion would
alter the existing per-intent error contract and is not done. The old
IllegalTransition fields remain unchanged. Rechecking recovered data is
intentional; no persisted proof authority is trusted. Iterator-partition
cardinality and the defensive guard are as established above. Suggested helper
extraction and historical deserialization optimization are not correctness fixes.

## Remaining integration

The repository-owned standalone `verification/resource-controls` package now
consolidates the two admission routes' positive, targeted-refusal and legacy
precedence controls. `resource-controls-r1` ran with exit 0 under the same caps:
401 milliseconds, peak memory 138 MiB, zero swap. Its resolved lock was compared
independently: all 75 registry versions, sources and checksums match the root
workspace lock. It is separate from `just ci` and does not change production's
unsafe-code lint; the unsafe allocator boundary exists only in the standalone
test binary. Locked offline replay (`resource-controls-r2`) and its own
all-target Clippy run (`resource-controls-lint-r1`) also exited 0. Expanded diff
review is recorded above. The later production Prepare/recovery review is also
recorded above. The expanded four-state harness and exact anchor moves received
a later merge review (MiMo V2.5 Pro, PASS WITH NITS). The critic's general-code
analysis was truncated; no complete critic pass is claimed. See the linked
maintenance record for source hashes and verified dispositions.

Completed-history binding and pre-effect reuse through receipt reconstruction;
independent graph/applied projection binding; broader precedence and
reservation-site controls; affected source/coordinate maintenance; full CI,
fixed-source proof replay and later qualification. Existing green CI at the
baseline must not be presented as verification of these new host changes.

## Merge verification

Fresh full CI on this host slice passed: 630 nextest cases, 47 skipped, 22
bootstrap controls, 142 proof-support controls, all 33 canaries, and the existing
threaded/doctest/documentation/dependency gates. Separate jurisdiction tests,
the four-state resource harness and its all-target Clippy also passed. The debt
meter remains 630 active and 47 deferred. Two Phase 0 M17 gates remain deferred.

The source change moved five frozen mutant coordinates. AM-17.13 maintenance
was independently reviewed before editing them; no thresholds, mappings,
assertions or goldens changed. The initial stale-coordinate failure is retained.
Commands, exits, bounds and hashed raw receipts are in
[`dg17-6-ordering-host-2026-09-16.md`](../docs/execution/reference-maintenance/dg17-6-ordering-host-2026-09-16.md).
This is development integration, not a replacement HAQP campaign or formal
qualification. The separate resource harness is still not part of `just ci`.
