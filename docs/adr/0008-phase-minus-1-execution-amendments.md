# 0008. Phase -1 execution amendments ledger

- **Status:** accepted
- **Date:** 2026-07-17
- **Deciders:** Brian
- **Related:** docs/execution/00-protocol.md §2 (amendment procedure); all Phase -1 work orders

## Context

The execution protocol (docs/execution/00-protocol.md §2) requires that every
amendment to a frozen surface — a signature rename, a parameter addition, a
work-order correction — is recorded in the work order's Amendments section AND
appended to this ADR as a single line. This creates an auditable trail of every
deviation from the planned scaffold, reviewed at M12 (the final gate).

## Decision

We will maintain this file as the single ledger of all Phase -1 execution
amendments. Each line takes the form:

```
AM-<milestone>.<sequence> (M<nn>): <one sentence>
```

Amendments are ratified or reverted at M12. No amendment is recorded without
its corresponding work-order entry.

## Amendment log

AM-1.1 (M01): liminald CLI — hidden `__fire` subcommand added — the injector must be provable before `exec` exists, through the production env path (D01.1).
AM-1.2 (M01): test naming — plan shorthand gains the `crash_` prefix so the serialized nextest crash group matches; assertions unchanged (D01.3).
AM-2.1 (M02): ScenarioScript moves crates — model relocates to liminal_daemon::scenario; conformance/src/scenario.rs becomes a re-export shim.
AM-2.2 (M02): Setup gains `buffers: Vec<SetupBuffer>` — promote_single_step contains `[[setup.buffer]]` that serde currently drops silently. Additive.
AM-2.3 (M02): liminal_graph::store::ns module — aux-namespace constants get one home (protocol §7).
AM-2.4 (M02): conformance `[[bin]] lim-toy` — D02.6 binary-location resolution.
AM-2.5 (M02): test-name prefixing — recovery_never_guesses → crash_recovery_never_guesses (nextest crash-group filter). Assertions unchanged.
AM-7.1 (M07): IdentityGrade::strength(self) -> u8 exposed pub (was private) for the M07 corpus's D07.5 worst-outcome ordering. Additive; pre-authorized by M07.md Frozen surfaces.
AM-8.1 (M08): runner step vocabulary gains resolver_observe and query (additive).
AM-8.2 (M08): liminal_revision::basis::graph_key() — stable map key for the GraphSnapshot component.
AM-8.3 (M08): liminal_revision::durability module (v4 §24.1 transcription; additive).
AM-8.4 (M08): SourceId::from_name(&str) deterministic UUIDv5 + uuid v5 feature (resolves DG-8.1).
AM-8.5 (M08): GraphStore::put_working_aux — ephemeral unlogged aux write for buffer blobs (no head bump).
AM-8.6 (M08): ToyWorkspace::basis inserts GraphSnapshot{head} at graph_key() (M08.2).
AM-8.7 (M08.4): liminal_daemon::queries::World seam (LiveWorld now, FrozenWorld at M08.8); StateView gains nodes()/relations()/scan_aux(); runner::ingest_files/ingest_graph widened to pub(crate).
AM-8.8 (M08.3): runner::refresh_file_blobs — refreshes SYS_BLOB["file/<path>"] to a save's landed bytes, called from apply_or_review's AutoApply arm.
AM-8.9 (M08.6): liminal_graph::kind::EXTERNAL_VALUE — a new node kind constant for the MaterializeExternal target (toy: at most one per scenario).
AM-8.10 (M08.7): ToyWorkspace::root() widened pub(crate) -> pub, like store() — needed outside liminal-daemon to build a LiveWorld.
AM-8.11 (M08.7): runner::exec reopens ToyWorkspace once after ingest_files/ingest_graph — seed_durable_inputs runs at open, before exec's own ingest, so a continuous exec session never saw the just-ingested file's component until apply_query's ws.basis() call (the first scenario-runner path to resolve a Basis in-process). Verified inert for every existing scenario.
AM-8.12 (M08.8): liminal_graph::StateView::synthetic(...) — new pub constructor building a StateView from raw parts; FrozenWorld (a different crate) has no other way to construct one since state_at's log-replay path is unavailable once the workspace is deleted. Additive.
AM-11.5 (M11): tracegen `import` subcommand — real public-repo history importer (first-parent walk, ext filter, provenance + skip counts in header source, Consent::PublicGit). Algorithm D's "at least 1 imported real history" mechanized.
AM-9.1 (M09): workspace `members` gains `"conformance/spikes/*"` (two publish=false spike crates; D09.1).
AM-9.2 (M09): phases.md Phase 0 task list gains task 12 — re-measure annotated-source anchor recovery over the real CST; the M09.7 L2 demotion stands until a new boxed report says otherwise. User-ratified 2026-07-18.
AM-11.1 (M11): conformance `[[bin]] gen-manifest` — heldout MANIFEST.b3 generator (D11.6: conformance bin, no new lim subcommand). Pre-declared in M11.md Amendments.
AM-11.2 (M11): conformance `[[bin]] scorecard` — the §7.4/R4 §2.3 scorecard pipeline runner (D11.6). Pre-declared in M11.md Amendments.
AM-11.3 (M11): conformance `[[bin]] tracegen` — the five Algorithm-B importers/generators (D11.6). Pre-declared in M11.md Amendments.
AM-11.4 (M11): liminal_daemon::runner::StepRunner — exec's setup + step loop factored into a public per-step seam (open/open_buffer/step) so the M11.5 pipeline can collect observables per event through the ONE runner; exec delegates, behavior unchanged.

**What would falsify or reverse this:** an M12 audit finding an amendment that
was applied in code but not recorded here, or recorded here but not in the
work order.

## Consequences

- **Positive:** every deviation from the scaffold is visible and auditable.
- **Negative:** a small amount of bookkeeping per amendment.
- **Follow-ups:** M12 audits this file against the work orders and code.

## Alternatives considered

- **No ledger** — rejected because silent deviations to frozen surfaces would
  accumulate unchecked, and Phase -1's conclusions depend on the scaffold
  matching the spec.
