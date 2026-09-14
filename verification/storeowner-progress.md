# StoreOwner implementation ledger

Baseline: `64ee56528df5f95b0f3c12f7f881942a6b2343f3`.
Authority: AM-17.12 and its caller-supplied ActorId clarification. Full verification
is not authorized yet. This is a staged migration, not a closed-authority claim.

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

## Remaining work before the full-verification checkpoint

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

## Testing speed policy

Reuse normal Cargo development artifacts and run affected-package controls during
implementation. Final qualification retains its prescribed clean source, fresh
caches, fixed matrices, seeds, budgets, zero retries and serialized crash tests.
No frozen qualifier optimization or assertion change is authorized by this ledger.
