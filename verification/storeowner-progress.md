# StoreOwner implementation ledger

Baseline: `64ee56528df5f95b0f3c12f7f881942a6b2343f3`.
Authority: AM-17.12, its caller-supplied ActorId clarification, and approved
DG17.3 supplement. Required full verification is now authorized with bounded
local compute. This is a staged migration, not a closed-authority claim.

## Slice A: epoch and volatile working capture

Trusted `ToyWorkspace::open` assembles a non-serializable StoreOwner. The owner
lends two private-construction capabilities, bound by reference to that store:

| Capability | Exact writable surface | Durable effect |
| --- | --- | --- |
| EpochWriter | `SYS_EPOCH["epoch"]`, integer SessionEpoch | Existing single graph transaction; no graph operations |
| WorkingCapture | `SYS_BLOB["buf/{ClientId}/{BufferId}/{u64 generation}"]`, text | None; original in-memory write, no graph revision advance |

These capabilities accept no arbitrary namespace or key and cannot be rebound
to a second store. The old public `put_working_aux` is now crate-private. Session
and epoch callers use these capabilities; existing lazy persistence and error
handling are preserved. No claim is made that previously ignored capture or epoch
errors now become durable success; that existing behavior still needs the later
capture/receipt work.

Development evidence lives outside the source checkout under
`/mnt/4tb/liminal-formal-evidence/reviews/`:

- `storeowner-owner-red.log`: exit 101, missing StoreOwner type (expected red).
- `storeowner-owner-green.log`: two new runtime tests pass.
- `storeowner-doctests.log`: two private-construction compile-fail tests pass.
- `storeowner-daemon-check.log`: targeted daemon check passes.
- Initial clippy reported needless pass-by-value for the epoch helper; corrected
  to borrow the capability, without changing the write contract.
- One attempted Cargo command combined `--test owner` with `--doc`; Cargo refused
  before running tests. The two selectors were then run as separate commands.

These are targeted development controls, not full CI, HAQP, proof establishment,
or qualification. New tests add two runtime cases and two doctests; no existing
test is removed, renamed, unignored, or weakened.

## Inventory verification

MiMo V2.5 Pro mechanically inventoried daemon write sites; raw response is
`storeowner-inventory.json` in the evidence directory above. Inspection found its
claimed `blob/{hash}` key grammar incorrect: `blob::put` in
`crates/liminal-jurisdiction/src/blob.rs` writes bare `hash.to_hex()` keys.
The raw model response is retained, not silently corrected or treated as authority.
Reconciliation helper writes likewise require inspection of their producing code.

The first affected-package check missed two external conformance callers of
`put_working_aux` in `freeze_rejects_stale_file_bytes`. The attempted targeted
M08 build failed with E0624 (exit 101); the external review also flagged this
visibility risk. This was a real omission, not a false positive. The full source
caller search now includes conformance as well as crates. Original failure is
retained in `storeowner-m08-acceptance.log`.

## Slice B: explicit acceptance actor and retained corruption controls

Brian required caller-supplied ActorId. `accept_repair` parses the step's `actor`
before entering the acceptance routine; missing, non-string and malformed values
refuse. The resulting HumanApproval records exactly that claimed identity, not an
authenticated-person claim. DAG acceptance and the existing M08 fixture supply
explicit test actors; their original assertions remain unchanged.

`storeowner-actor-red.log` records both new tests failing on the old implementation
(exit 101): missing identity still applied effects, and explicit identity was
replaced by a random actor. `storeowner-actor-green.log` records both passing.

Existing M08 corruption is retained through an explicit owner-granted
`BlobFaultInjector`: exactly volatile `buf/{client}/{buffer}/{generation}` and
`obs/{source}/{hash}` writes, never generic namespaces, ILRP records or durable
transactions. Root fixture assembly grants it before transferring StoreOwner to
`ToyWorkspace::open_with_owner`. Ordinary workspace/read access cannot obtain the
grant. This is an explicitly untrusted-data fault adapter, not normal production
capture. The owner and grant share the same store allocation; tests must drop
both to release the file lock before reopen. Existing M08 tests do so and retain
all stale/missing/hash-corruption and reopen assertions.

`open_with_owner` refuses a store whose directory differs from `root/state`
before recovery or staged-file cleanup. A new test covers wrong-root refusal.
The four existing M08 tests pass in `storeowner-m08-fixed.log` (exit 0), including
the previously uncompilable corruption witness. Initial actor-test clippy found
a missing statement semicolon; fixed without assertion change, with separate
failure and rerun logs retained.

Review of slice A: PASS WITH NITS; critic's downstream caller concern was verified
and fixed above. The suggested epoch ActorId extension is not applicable: epoch
bookkeeping is not the explicit human-acceptance route. The commit tuple remains
visible because it names the exact durable write result; it is not an ILRP
committed receipt or a broader qualification claim.

Slice B pre-commit review: PASS WITH NITS. Its proposed normal-capture observation
writer is rejected because that would expand WorkingCapture authority beyond
buffer capture. The critic's Arc coercion compile-error concern is a false
positive: `WorkingCapture.store` is `&GraphStore`; Rust deref coercion and the
workspace all-targets check confirm the conversion. Owned fault grants deliberately
keep the same allocation and lock alive after owner transfer/drop, without gaining
new authority. A lifecycle control verifies reopen refuses with `StoreError::Locked`
while the grant exists and succeeds after its drop; no deadlock is asserted away.
Some critic output was truncated by the review service; it is not an independent
complete adversarial pass or qualification evidence.

## Slice C: actual store-lock identity at assembly

Post-commit review of slice B returned PASS WITH NITS, with several speculative
critic findings. Verification at the actual code established:

- `StoreOwner::store` still returns `&GraphStore`; the Arc fields are private.
- Actor scenarios are freshly parsed inside each loop iteration.
- `field` returns `&str`; ActorId parsing uses `Uuid::parse_str`, not an
  arbitrary-nonempty-string check. Caller identity remains a claim by design.
- GraphStore accepts an arbitrary store directory; only ToyWorkspace appends
  `state`, so the graph-level lock test uses the correct directory.
- Public owner-consuming assembly does not hand an owner to ordinary readers.

The path concern produced a real counterexample: open an owner, rename its
`state` directory, create a replacement at the same pathname, then assemble with
the old owner. Path equality accepted the wrong store. The new test failed on
that code (exit 101, `storeowner-replacement-red.log`) and passes after the fix
(`storeowner-replacement-green.log`). No existing assertions were weakened.

The coordinator now retains its originally locked File inside a `same-file`
Handle and compares it with an independently opened handle to the named lock.
Both handles stay open during comparison; no clone of the held lock is required.
The named lock is opened without create/truncate/write effects. Path spelling
must still match. Canonicalizing two path strings alone would not detect the
replacement and is not the fix.

`same-file` is pinned to the already locked version 1.0.6; only a direct dependency
edge is added, not a version upgrade. Its cached source was inspected: Unix
comparison uses device/inode while both files remain open. Windows file-ID
limitations documented by that implementation, including ReFS, mean this is not
Windows identity-qualification evidence. Current witness is Unix, under the
approved Linux-first host scope. This is a point-in-time check, not a directory
lease against later same-user replacement/hard-link tampering. Those stronger
host guarantees are not established by this slice.

Targeted store regression includes all nine existing store tests plus three owner
tests (`storeowner-lock-regressions.log`); two actor and two workspace-owner tests
pass in `storeowner-identity-regressions.log`. Full verification remains pending.

Pre-commit identity review: PASS WITH NITS. Adopted the missing-lock result as
`Ok(false)` rather than an I/O error; a targeted control first failed on the old
result and then passed. Other I/O errors remain fail-closed errors. Kept the
write-capable, non-creating/non-truncating open: it matches GraphStore's original
write-lock permission requirement and does not introduce a requirement to read
the lock file. The store is a writable coordinator, not a read-only mount service.
Renamed the actively used field to `lock_handle` for clarity.

## T1 blocker before completing raw-API closure

DG17.3 in `docs/execution/M17.md` records the exact frozen qualifier dependency:
`generated_ilrp_probe` constructs a raw driver and supplies unchecked safety
evidence for a Node absent from an empty store. A checked admission cannot accept
that fixture. AM-17.13's coordinate-only exception does not authorize adapting
it. Request a narrow fixture/interface migration exception that retains all
probe obligations, assertions, output bytes, goldens and thresholds. No qualifier
code was changed; no unchecked compatibility bypass is authorized.

## Remaining work before the full-verification checkpoint

DG17.3 was approved in the subsequent conversation. Its narrow probe migration
may now proceed subject to all preservation conditions above. On this host the
initial check found 44 GiB available RAM and no active liminal-lane unit. Run
one compute job at a time, with at most four Cargo workers and explicit cgroup
memory/CPU limits for sustained jobs. Resource exhaustion is a retained failure,
not a retry or a reduced qualification matrix.

- Close GraphStore raw begin/snapshot access after migrating all write callers.
  StoreOwner::store still exposes the legacy GraphStore surface in this slice.
- Enumerate and enforce bootstrap, reactor, capture/debt, bookkeeping and ILRP
  scopes; ensure no general writer reaches ordinary request handling.
- Extend the explicit ActorId route into the forthcoming checked admission type;
  input parsing and positive/negative actor controls are implemented in slice B.
- Checked Basis and complete interpretive repair admission; validate recovered
  intent history, step identity, dependencies, acks and observed poststates.
- Finalization permits and durable committed receipts; no committed outcome from
  raw DTOs, contested completion or later bookkeeping writes.
- Remaining public-interface bypass tests, same-body proof integration and
  reference-maintenance evidence. Full verification remains a later checkpoint.

## Slice D: ILRP acknowledgement and persisted-record structure

DG17.3 approval is recorded in amendment commit `1fb7b34a`; its mandatory
external review returned PASS. The frozen probe itself is still unchanged.

Before insertion/persistence, acknowledgements now require the exact planned
step key, mutation identity, expected poststate and acknowledged predecessors.
Both run and recovery validate persisted repair identity, step-key identity,
DAG structure, every acknowledgement, empty acknowledgements in Prepared, and
complete acknowledgements in ExternalApplied/Finalizing/Committed. Invalid
records remain untouched and refuse; terminal labels do not skip this check.
This is structural validation only, not proof of durable history, subject
authority or a committed receipt. Those interfaces remain pending.

Two added tests provide red/green controls for malformed acknowledgements and
missing acknowledgements at finalization/terminal states. The first red stops
at the wrong-ID case; wrong-poststate is additionally covered by the green
matrix, not claimed independently red in that log. Five pre-existing tests
retain all assertions; their positive fixtures now provide real planned IDs
and poststates, and complete acknowledgements when claiming completed effects.
Seven ILRP tests pass. All 29 milestone tests pass (24 skipped), and targeted
Clippy passes. All evidence is under the reviews directory above.

MiMo's mechanical fixture inventory missed `ilrp_recover_twice_is_noop` and
incorrectly mentioned a Committed case in the nonterminal fixture matrix.
Inspection of the actual fixtures corrected both; no review suggestion to
leave terminal acknowledgements unchecked was adopted.

Pre-commit code review returned PASS WITH NITS. Verified dispositions: map
lookup does not imply equality of a public payload's `id` field, so retain that
check. Recovery refusal of malformed historical data is the approved
revalidate-else-preserve policy, not grounds to skip validation, strip invalid
acks, or wipe the store. Each persisted predecessor ack is validated before
advance; individual new acks validate before insertion. Error reclassification
as corruption is deliberate for persisted DTOs. Toy-scale complexity and
test-name/style nits are deferred, not correctness fixes.

Four frozen mutant coordinates moved by one line, with independent pre-change
review and source hashes in
`docs/execution/reference-maintenance/2026-09-13-ilrp-validation.md`.
Inventory passes. No expected result, threshold or frozen probe was changed.
Fixed-source full CI and later independent stages have now run; the failed
recipe, fresh producer-generated crash evidence, five remaining frozen-registry
failures and resource measurements are recorded in
`verification/ilrp-verification-2026-09-13.md`. DG17.4 needs Brian's decision.

Observed build-cache interference: the packet-digest build lost a native BLAKE3
archive and returned 101; target/debug disappeared during inspection. The
cleanup actor is unknown. An isolated external Cargo target directory passed
the identical build; no code fix or cache deletion was needed. Largest observed
job was the milestone test build/run at 2.1 GiB, with zero cgroup swap use.

## Testing speed policy

Reuse normal Cargo development artifacts and run affected-package controls during
implementation. Final qualification retains its prescribed clean source, fresh
caches, fixed matrices, seeds, budgets, zero retries and serialized crash tests.
No frozen qualifier optimization or assertion change is authorized by this ledger.
