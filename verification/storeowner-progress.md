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

## Remaining work before the full-verification checkpoint

- Close GraphStore raw begin/snapshot access after migrating all write callers.
  StoreOwner::store still exposes the legacy GraphStore surface in this slice.
- Enumerate and enforce bootstrap, reactor, capture/debt, bookkeeping and ILRP
  scopes; ensure no general writer reaches ordinary request handling.
- Require caller-supplied ActorId at explicit acceptance, with negative controls.
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
