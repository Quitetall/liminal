# Basis-selection fixtures

**Populated by:** M6 (Basis Perspectives, three snapshots,
component-granular invalidation — killer experiment #3). Empty until
then by design.

**Spec:** v4 §114 ("expected BasisPerspective selection for
concurrent-client scenarios"), v4 §7.5, v4 Laws 3D and 3J, R4 §8, R4 §10.

## What lives here

Concurrent-client perspective cases with expected snapshot content:

- A durable file plus two independent dirty buffers (Neovim and phone)
  over it, each a first-class `BufferGeneration` Basis component with
  its own `epoch + generation`.
- For each requested perspective — `ClientScoped(neovim)`,
  `ClientScoped(phone)`, `DurableOnly` — the exact expected snapshot
  content. Three perspectives, three *intentional* snapshots (R4 §10).
- Negative space: no perspective may yield a chimera that mixes both
  clients' dirty buffers into one semantic state (Law 3J), and
  `ClientScoped` never reads another client's dirty buffer (R4 §8).
- Invalidation cases: which cached computations a given buffer
  generation bump invalidates. Dependency tracking follows the
  *selected* Basis components, not the whole available-input map —
  client A's edit invalidates A-scoped computations only (R4 §8).

## Consuming tests

`tests/phase_minus_1.rs::three_perspectives_yield_three_intentional_snapshots`,
`no_computation_sees_a_chimeric_basis`,
`buffer_generations_invalidate_only_dependents`, and the M6 tests
`three_perspectives_three_snapshots`, `prop_no_chimeric_basis` (proptest
over random interleavings), `invalidation_component_scoped`.

The `liminal-revision` crate's `resolve` / `BasisPerspective` API is the
subject under test; these fixtures are its expected-output oracle.
