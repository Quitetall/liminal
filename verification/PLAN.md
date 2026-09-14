# Proven safety core implementation contract

Status: implementation plan approved in conversation; companion specification
adoption, phase GO, suite ratification and release authorization remain separate.
Planning baseline: `fb172797ab715f5a44aec9d0563a86d865af550e`.

Integration update, 2026-09-13: the user confirmed the other writer stopped and
directed baseline consolidation, current-core hardening, honest Phase 0/M17
closure, and only then Phase 1. See
[`baseline-consolidation-2026-09-13.md`](../docs/migration/baseline-consolidation-2026-09-13.md).
This records sequencing and implementation scope, not final phase authorization.

## Current delivery boundary

The first implementation slice supplies the closed candidate obligation and
assumption registries, structural validation and refusal-only qualification
commands. `formal-check` currently validates those registries and their source
coordinates; tool/executable/proof bindings are not yet part of that command.
Do not interpret its successful exit as the complete future check contract below.

The committed bootstrap fixtures demonstrate pinned-tool feasibility: a true
arithmetic contract, its false implementation, ordinary and verified builds,
cold replay, and positive/negative Boolean-model controls. They are not production
code and do not discharge any of the sixteen obligations. The checked runtime
ILRP/StoreOwner slice landed at `51361e19`; its development and verification
records are linked below. The candidate leaf now proves executable acknowledgement
identity/dependency membership used by production, with constrained vstd
admission approved in DG17.5. This narrow predicate does not discharge `ilrp-ack`.
Further protocol/core proofs, qualified adapters and formal evidence aggregation
remain unimplemented. See `safety-ack-slice.md`, `haqp-29f9f159-result.md`,
`bootstrap/README.md` for reproducible tool-control commands and
`checked-repair-verification-2026-09-14.md` for the runtime evidence boundary.

The runner slice under `proof/` validates selected source pins and complete
extracted Verus distribution bytes, and constructs a checksum-closed dependency
tree from an explicit archive set at a new external destination. It also supplies
a bounded Linux command-execution seam with raw output and observed service
receipts. Exact-commit Git reconstruction now supplies fresh source stages;
the constructor and checker share bounded manifest validators. Requested commit
and historical inventory origin remain distinct. The Rust payload constructor now
reconstructs all six selected pinned Linux archives, preserving empty directories
and refusing conflicting overlaps. Independent output comparison and a corrupted
namespace-view control establish this development construction boundary. A later
bounded sandbox probe observed constructed-compiler selection and a two-sided
scratch-source control observed strict forwarding in that reconstructed
environment. A bounded same-source ordinary/verified witness pair also observed
stable per-build binaries and identical runtime output. This remains development
witness binding, not verifier-source correctness, a qualified executable, cold
independent construction or qualification. Its 106 support controls and development statuses retain
`qualification: false`, never proof or qualification. The caller must bind
source/tool/profile and lock/archive authority, and independent
comparison—not the producer's returned map—checks constructed output. Real probes
reconstructed all 167 locked packages and demonstrated ordinary and strict
verified build feasibility. The selected-eight build-script counterexample is now
closed at the input stage by the 166-entry source inventory; runner/profile/commit
authority, controlled replay and formal receipt aggregation remain unfinished.
This is not yet integrated with the formal
commands. See `proof/development-2026-09-14.md` and
`proof/selection-development-2026-09-14.md` for exact limits, failures,
environment boundaries, evidence hashes and remaining execution work. All
sixteen obligations remain pending.

The fast proof-input recipe is now part of local CI. Full `just ci` at
`f31d727c` passed: 620 Rust tests, 47 skipped, 85 Python controls and all 33
existing canaries. See `proof-ci-2026-09-14.md` for fixed-source evidence,
resource bounds and the preserved pre-CI launcher failure. This does not qualify
the formal runner or replace the required HAQP campaign.

`authority-amendment-proposal.md` records the concrete StoreOwner/read-view
split approved by the user on 2026-09-13 as AM-17.12, recorded in M17 and the
Phase 0 ledger before production migration. The user authorized implementation
and targeted development controls (steps 1–4). The subsequent DG17.3 approval
authorizes the narrow frozen-probe migration recorded in M17 and required full
verification with bounded local compute. This does not adopt the formal
companion or authorize a phase.

## Authority and scope

The user selected whole-core scope, a small proven safety core, trusted host,
Linux-first adapter qualification, scoped interface amendments, phase-aligned
proof gates, explicit resource limits, distinct volatile/durable capture, and an
audited verifier with independent replay. The subsequent instruction was
"Implement the plan." This records implementation authority, not a signature or
an OpenWarrant acceptance record.

The accepted SAS is revision `0.1.0-proposed.1`, SHA-256
`53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`, at
commit `9e76fc99027c55ded5bd0cc61da43b0f2b68b049`. Its 777 requirement meanings
and historical bytes remain unchanged. References below reuse existing IDs;
local obligation labels are not new SAS requirements. New formal obligations
must be incorporated through the successor acceptance procedure before they
become authoritative phase gates. The accepted receipt, not the historical
source-crosswalk's original proposal label, determines human SAS acceptance.

AM-17.11 freezes the existing HAQP instrument. AM-17.13 permits only tracked,
independently verified, behavior-preserving reference maintenance under
[`haqp-reference-maintenance.md`](../docs/execution/haqp-reference-maintenance.md).
DG17.3 additionally permits the exact generated ILRP probe setup migration
recorded in M17, preserving its assertions and outputs. DG17.5 permits exact
vstd pin/feature admission and its refusal controls, not a general dependency
allow-list expansion. Otherwise do not modify
`crates/liminal-xtask/src/haq.rs` or `scripts/haqp_*`.
Keep existing
HAQP thresholds, packet, evidence, locked corpora, M18-M24 gate names and M24's
exact seven-test aggregate intact. Separate candidate formal evidence is not
HAQP evidence. Production changes require the applicable later requalification.

No phase is authorized here. No existing test or assertion may be removed,
weakened, renamed or un-ignored. Interface changes require the amendment record
before the affected production change. ADR-0018's single interpretive semantic
path remains: verify the executable bodies used by production, not a replacement
compiled policy or a separately handwritten "verified twin."

## Packages and sequencing

1. **Obligation and adoption package.** Record this contract, closed obligation
   inventory, exact source anchors, assumption inventory and tool pins. Candidate
   readiness reports are separate from acceptance. Missing obligations refuse.
2. **Proof-tool bootstrap.** Verify pinned tools, a real positive proof, a
   deliberately false proof, runtime admission, and cold replay. Record actual
   measurements. Tool incompatibility stops migration; it never licenses an
   unverified substitute.
3. **ILRP vertical slice.** Introduce the dependency-leaf `liminal-safety` module
   and checked translations, then move deterministic ILRP decisions behind its
   interface after the necessary amendment. Validate data at runtime before any
   ghost precondition is relied on. Keep the production executor and shared
   run/recovery path. Prove the protocol relationships listed below.
4. **Core and adapters.** Extend to transaction/replay validation and receipts,
   Basis and determinism, mutation-local admission, capture/exhaustion, then
   formal-evidence aggregation. Qualify Linux production adapters alongside
   deterministic injected-fault adapters.
5. **Phase integration.** Attach the separate formal companion gate only after
   specification adoption. Require applicable current-core proofs before GO;
   extend at authorized M18-M24 work and at preceding gates for later phases.
   Do not implement future features early or claim their proofs exist.

Primary owns invariants, contracts, proof reasoning and T2 integration. Delegated
workers own bounded mechanics. Each code-bearing slice uses red/green controls
through its agreed public interface. Every commit receives external review;
findings are verified before fixing. Independent Standards and Spec reviews
cover the accumulated delta against the planning baseline.

## Construction and proof obligations

Independent review found that the first candidate-registry transcription omitted
the existing v4 §7.8 Apply and Revert steps. The two local rows below correct that
inventory omission; they reuse `LIM-SAS-RQ-115`, introduce no SAS requirement or
constitutional meaning, and remain pending adoption and not established.

| Local obligation | Existing requirement anchor | Required result |
| --- | --- | --- |
| identity-only | LIM-SAS-RQ-104 / Law 1 | Proof representations add no third semantic primitive. |
| authority | LIM-SAS-RQ-107,108,110,113,114 / Laws 3,3A,3C,3F,3G | Only validated, subject-local authorization permits an accepted effect; uniqueness alone is insufficient. |
| capture | LIM-SAS-RQ-109,116 / Laws 3B,3I | Failed promotion retains capture and visible debt; volatile capture is never described as durable. |
| basis | LIM-SAS-RQ-111,117 / Laws 3D,3J | Immutable, perspective-correct selected inputs; no independent dirty-buffer chimera. |
| ilrp-order | LIM-SAS-RQ-115 / Law 3H; v4 7.7-7.8 | Reject invalid DAGs and unsupported graph-before-external dependency shapes before effects. |
| ilrp-intent | LIM-SAS-RQ-115 / v4 7.8 Prepare | No external application before durable intent. |
| ilrp-apply | LIM-SAS-RQ-115 / v4 7.8 Apply | Before every step, verify its prestate; stage, flush, and atomically replace files where supported; use service idempotency keys where supported. |
| ilrp-ack | LIM-SAS-RQ-115 / v4 7.8 Acknowledge | Ack identity, step, observed poststate and dependency completion agree. |
| ilrp-finalize | LIM-SAS-RQ-115 / v4 7.8 Finalize | Required effects precede exactly-once accepted graph finalization; graph effects and Committed intent share a transaction. |
| ilrp-recover | LIM-SAS-RQ-115 / v4 7.8 Recover | Repeated recovery is safe; neither-pre-nor-post state cannot be guessed past. |
| ilrp-revert | LIM-SAS-RQ-115 / v4 7.8 Revert | A valid inverse executes through the same ILRP rather than bypassing its ordering and recovery rules. |
| store | v4 92; ADR-0007; TM-01,TM-02 | Invalid transaction leaves no accepted trace; recovery validates durable prefix and fails closed on interior corruption. |
| receipt | v4 7.8; TM-02,TM-10 | Receipt certifies only the exact writes whose durability contract completed. |
| evidence | LIM-SAS-RQ-003,005,012,017; ADR-0020/0021 | No formal PASS from missing, stale, partial or mismatched evidence; no claim of HAQP verifier correctness. |
| phase1-extension | LIM-SAS-RQ-006 through 012 | Add applicable range, source-preservation, derived-Basis and output-admission proofs as the authorized implementations arrive. |
| program-extension | LIM-SAS-RQ-018 | Later phase interfaces extend the obligation inventory before acquiring new effects. |

All obligations start **not established**. A passing bootstrap proves tooling
feasibility only. Proof of state-label transitions does not discharge the ILRP
obligations. A bounded model is not an unbounded implementation proof.

The interface contracts are `ValidatedBasis`, `AuthorizedRepair`,
`RecoveryState`, `FinalizationPermit`, scoped durable receipts and validated
proof evidence. Their names describe planned contracts, not implemented types.
Runtime constructors must check all conditions needed by callers. Public raw
DTOs may remain compatible; they confer no trusted capability. Deserialization
must not construct trusted states directly. Checked conversion preserves exact
identity, revision, operation and source meaning.

The leaf core must not depend on graph, revision or jurisdiction crates that
consume it. The known bypass inventory includes public graph transactions,
generic ILRP aux writes, public `RepairIntent` fields and direct `WorkspaceBasis`
construction. Internal bootstrap/reactor/session/bookkeeping rights are distinct
from subject-local mutation authority and must be narrowly scoped.

Current ILRP finalization covers graph operations and committed intent. Later
file mirrors and repair records are separate transactions. Do not silently
extend a receipt across those writes, collapse crash boundaries, or call the
plan's input Basis its proven resulting Basis. Preserve honest scope while
qualifying follow-up work separately.

## Tools, models and trust

Initial Verus pin: `0.2026.08.30.b432e82`, source commit
`b432e82fed7e05090fd53b5e5fc39020f725aabe`, Rust `1.97.1`.
Linux archive SHA-256:
`067f5f72a457fe66b77c0c10b180f2a919a9c7481a8baa024ffc716aa931a41b`.
Pin the matching libraries, solver and exact flags before evidence is emitted.
Use full `cargo verus verify` / verified builds; `focus`, skipped verification,
function filtering and development caches do not qualify the core.

Independent TLC pin: `1.7.4`. Finite models cover up to three steps, two Holders,
two repairs, all dependency shapes, pre/post/neither observations and repeated
crashes. Record completed state-space exploration, not elapsed time alone.
Model constants bound the experiment, not the product. Record fingerprinting,
abstraction and fairness assumptions. Liveness requires eventual availability;
safety does not imply termination during perpetual failure.

Trusted host means declared OS/filesystem durability semantics, compiler and
verifier behavior, applicable cryptographic assumptions and a non-hostile
qualifier. No claim covers arbitrary same-user tampering or broken hardware.
Project assumptions and external bodies require exact coordinates, rationale,
owner and affected obligations. No unexplained assume/admit/proof bypass is
eligible. Runtime syscalls remain tested adapters, not magically proved code.

Initial Linux qualification targets x86-64/ext4, recording the actual kernel,
filesystem and mount settings. Other-platform CI remains enabled without
inheriting Linux durability claims. Real process-death injection is not physical
power-cut evidence. Resource ceilings are explicit parameters; qualified values
come from public/generated workloads, never locked acceptance data.

## Public command contracts and acceptance

- `just formal-check`: check closed inventory, exact source/tool pins, assumptions
  and binding metadata; this is structural verification only.
- `just formal-proof`: execute every applicable registered implementation proof.
- `just formal-model`: complete the declared finite-model explorations.
- `just formal-adapters`: run real and injected-fault adapter contract tests.
- `just formal-gate <phase>`: aggregate applicable evidence and adoption state;
  missing/unimplemented/partial evidence must return nonzero, not an empty pass.

Qualification records source commit/tree, obligation and assumption hashes,
tool binaries/versions, dependency pins, exact flags, commands, actual exits,
raw artifacts, measurements and produced executable bindings. Reproduce from a
clean fixed source and fresh caches. Missing tool, timeout, zero verified
functions, empty inventory, substituted source or partial verification refuses.

Required controls include handcrafted committed state, wrong/missing ack,
invalid DAG, stale Basis, wrong subject, raw commit bypass, malformed persisted
data, before/after durability failures, follow-up bookkeeping failure,
resource-boundary and overflow inputs, and false or stale evidence. Independent
specification witnesses and adversarial review attack vacuity and weak theorems.

Existing full CI, crash tests, HAQP campaign, mutation floors and independent
reviews remain required. No deleted tests or weakened assertions substitute for
new interface coverage. Qualification finishes only when every applicable
critical obligation is established with its declared adapters and assumptions,
all reviews are complete and the separate human decisions are recorded.
