# Liminal production roadmap

This is the central delivery sequence for the complete Phase -1–12 program.
The [SAS](../sas/LIMINAL_Software_Architecture_Specification.md) was
[accepted by Brian](../migration/sas-acceptance-receipt.md) at its exact revision
and digest. OpenWarrant registration remains pending the human authority
register, so its machine view still reports proposed. Acceptance changes
specification authority; it does not authorize Phase 1, ratify a suite, or
resolve a Warrant.

Non-normative source history for a possible later revision is tracked in the
[successor candidate bundle](../migration/successor/README.md). It does not
change the accepted SAS selection or any phase status.

## Baseline and evidence

Migration starts from Liminal `0d8c32a4ae1afda9808e55c3c291445aad4e3d60`,
matching fetched `origin/main` at migration start. Phase 0 remains open at
[M17.5](../execution/M17.md). The committed
[HAQP packet](../../conformance/haqp/packet.json) is proposed, unratified,
`qualification_state=not-run`, `qualification_stage=1a`. HAQP requirement IDs,
thresholds and source coordinates remain unchanged. Existing substrate is not
completed Phase 1 work. An active campaign in the original checkout is outside
this migration worktree; partial fuzz success does not establish eligibility.

Phase -1's attributed decision is retained in
[ADR-0013](../adr/0013-phase-0-go-no-go-after-falsification-laboratory.md).
Old boxes and predicted test totals remain historical. Current coverage comes
from named tests, `just gates`, the HAQP inventory and independently reviewed
evidence on the exact candidate. Baseline/current counts belong in verification
records, not normative promises about future inventories.

## First delivery: qualified OpenWarrant compiler

| Order | Warrant / work | Prerequisites | Required exit evidence and decisions |
|---|---|---|---|
| 1 | [LIM-WAR-0001 — M17 closeout](../warrants/LIM-WAR-0001/atoms/40-work-order.md) | Existing M17.1–4 evidence; exact fixed campaign base | Complete HAQP-1a, fresh independent blind findings verified and fixed, full rerun after last defect, qualified packet, amendment disposition, Brian's separate M17.6 GO, `just ci`, named inventory reconciliation. |
| 2 | [LIM-WAR-0002 — M18 source/CST](../warrants/LIM-WAR-0002/atoms/40-work-order.md) | M17.6 GO and M17 closeout; accepted grammar | Rope-backed source, lossless error-tolerant CST, exact revision ranges and M18 gate evidence. |
| 3 | [LIM-WAR-0003 — M19 maps/HIR/graph](../warrants/LIM-WAR-0003/atoms/40-work-order.md) | M18 | Full source maps and annotations, HIR, all eight explicit syntax forms, compact/explicit round trip, derived graph and stable debug JSON; complete M19 contract. |
| 4 | [LIM-WAR-0004 — M20 formatter/CLI](../warrants/LIM-WAR-0004/atoms/40-work-order.md) | M19 | Formatter laws and exact CLI command/error/atomic-file semantics from M20. |
| 5 | [LIM-WAR-0005 — M21 incremental compiler](../warrants/LIM-WAR-0005/atoms/40-work-order.md) | M18–M20 | Component-scoped invalidation, exact Basis and independent full/incremental equivalence. |
| 6 | [LIM-WAR-0006 — M22 HTML](../warrants/LIM-WAR-0006/atoms/40-work-order.md) | M19–M21 | Full-document HTML and incremental patch equivalence, reviewed goldens and malicious-input refusals. |
| 7 | [LIM-WAR-0007 — M23 fuzz/benchmarks](../warrants/LIM-WAR-0007/atoms/40-work-order.md) | M18–M22 | Fuzz regressions and benchmark baselines with provenance; frozen thresholds and honest host-load classification. |
| 8 | [LIM-WAR-0008 — M24 qualification](../warrants/LIM-WAR-0008/atoms/40-work-order.md) | M18–M23 gates active legitimately; HAQP-1a retained | Exact seven-test aggregate, conditional persisted-format branch, HAQP-1b mutation requirements, complete CI/inventory, independent review, both-stage evidence, then separate Brian suite ratification. |
| 9 | [LIM-WAR-0009 — integration/protocol](../warrants/LIM-WAR-0009/atoms/40-work-order.md) | Qualified Phase 1 and separately ratified suite | First accept supported document/frontmatter semantics, versioned process boundary and canonical IR; then implement adapter and refusal tests against those interfaces. |
| 10 | [LIM-WAR-0010 — compiler profile/pins](../warrants/LIM-WAR-0010/atoms/40-work-order.md) | Accepted integration interface | Reviewed restricted profile, supported/unsupported boundary and actual compiler/toolchain/dependency/corpus pins. No placeholder pin establishes reproducibility. |
| 11 | [LIM-WAR-0011 — whole-corpus parity](../warrants/LIM-WAR-0011/atoms/40-work-order.md) | Profile/pins; byte and semantic observable design accepted before comparison | Every pinned OpenWarrant document compiled by both adapters; **zero byte and zero semantic differences**; mismatch, missing-document and empty-observable controls refuse. Retain per-document raw results. |
| 12 | [LIM-WAR-0012 — compiler distribution](../warrants/LIM-WAR-0012/atoms/40-work-order.md) | Qualified parity, independent verification and release authority | Reproducible builds, version pin, usage docs, compatibility limits, retained qualification evidence and explicit release decision. |

These Warrants are proposals with unestablished obligations. Their links and
dependency graph are also recorded in
[warrant-index.json](../migration/warrant-index.json). Source obligations and
existing P1-R/P1-T identifiers are linked in
[source-crosswalk.json](../migration/source-crosswalk.json). A contribution of
`partial` never proves the full SAS requirement fulfilled.

T1 gate design, golden acceptance, amendments and human decisions retain their
original responsibility tier. T2 implementation requires engineering review;
T3/T4 work is delegated within its frozen scope. The migration does not execute
M18–M24 or un-ignore their gates.

## Program after the compiler distribution

The first distribution is an additional Phase 1 delivery sequence, not a new
phase numbering scheme. All fourteen canonical phases remain declared by the
SAS, including `roadmap://LIM-PHASE--1`, 11 and 12.

| Phase | Delivery commitment | Dependencies / gates |
|---|---|---|
| -1 | Falsification laboratory: identity, Jurisdiction, repair, ILRP, projection, Basis, trace ergonomics | Retain historical M01–M12 evidence and accepted decision; no retrospective resolutions. |
| 0 | Constitution, profiles, corpora and conformance laws | M13–M17; closeout above. |
| 1 | External-file source-to-HTML compiler and qualified OpenWarrant distribution | M18–M24, then integration/profile/parity/release; full first-delivery chain above. |
| 2 | Persistent daemon, LDP, Neovim workflow and exact buffer/file/Git Basis | Phase 1 gate; accepted wire format; real-daemon SIGKILL matrix. |
| 3 | Transforms, graph rewrites, macros, Lua and semantic editing | Phase 2; same macro from editor/CLI/harness, source reparse equivalence. |
| 4 | Workspace manifest/lockfile, build graph, Cargo/LSP/Pandoc, resources and indexes | Phase 3; conversion-loss, lazy elaboration and corrupt-object gates. |
| 5 | Prose, planning, academic, tables, notebooks, long-form and graph-native pilot | Phase 4; profile and schema decisions; queue-backed agenda requires explicit prohibition retirement. |
| 6 | History, revisions, backup and local-first synchronization | Collaboration/serialization/retention ADRs before runtime; partition convergence, migration and merge refusals. |
| 7 | External relation policies, effect reactor, resolvers, freshness and reproducible freeze | Phase 6 and prior Basis machinery; deterministic replay and no traversal effects. |
| 8 | Images/PDFs, audio/transcript, ink, canvas and lecture resources | Phase 7 plus Phase 4 resources; offline reconstruction and distinct media/recognition authority. |
| 9 | Rich web, phone and tablet clients | Phase 8; expose only proven projection capabilities, cross-client semantic agreement and Wasm gate. |
| 10 | AI compiler, model profiles, context planner and approved semantic operations | Phase 9 plus Basis/provenance; stale-operation refusal and evaluation before encoding optimization. |
| 11 | Stable plugin ecosystem, versioned interfaces, packages and certification | Phase 10; third-party dialect and capability-refusal gates, schema/dialect ABI and governance decisions. |
| 12 | Advanced spatial/table/presentation/ink/visualization runtimes | Phase 11; specialized execution preserves semantics; any policy compilation needs differential equivalence including crashes and concurrent buffers. |

Every phase retains all detailed obligations in the incorporated v4/R4 clauses
and [historical phase breakdown](../execution/phases.md). Later mechanical
Warrants are authored at preceding gates; this roadmap does not invent future
interfaces or waive architectural experiments.

## Blockers, ownership and evidence required

| Blocker | Owner / responsibility | Required evidence to remove it |
|---|---|---|
| OpenWarrant registration of accepted SAS | Brian, T1 | Human acceptance received; human-authored authority register and acting-role response still required for tool ingestion. Phase GO and suite ratification remain separate. |
| Program-specific OpenWarrant phases | OpenWarrant compatibility Warrant performer + independent reviewer | Signed references and all fourteen LIM phases; OW compatibility; duplicate/malformed/undeclared/wrong-program refusals; missing/ambiguous/drifted authority unavailable. |
| HAQP-1a completion | M17 performer and blind reviewers; Brian owns GO | Exact final packet/tree/parent, all evidence lanes and fresh findings; no completion from partial campaign outputs. |
| Historical meter and ledger reconciliation | M17/M24 T1 owner | Named inventory deltas, disposition of every open amendment, explicit unresolved gaps rather than invented approvals. |
| Integration interfaces absent | Integration T1 design owner | Reviewed protocol, IR and document semantics before implementation; profile before pins; observable list before parity. |
| Actual whole-corpus parity absent | Parity performer + independent verifier | Both adapters over entire pinned corpus with zero differences in both observables; unit tests of the harness are insufficient. |
| Final baseline eligibility | Qualification owner | Preserve original campaign results, assess exact final candidate, rerun qualification in full where source eligibility changed. |

OpenWarrant source base for this migration is
`8bc3978a26de0c13605c6d206fe820a8440be737`, including the local integration
packet work. Its existing `OW-WAR-0040` amendment requires both byte and
semantic observables at zero differences. No cross-repository authorization or
federated resolution is inferred from this source citation.

## Adoption procedure

Human acceptance of the exact proposed revision has been received. The receipt
above is authoritative for that decision; tool registration and cross-repository
integration remain pending. The original procedure is retained below as the
sequence, with step 3's human decision completed but its registry import pending.

1. Review the proposed SAS, complete crosswalk, reconciliation table and twelve
   Warrants. Check source preservation, ID uniqueness, all phase declarations,
   protected HAQP thresholds and unknown/unresolved reporting.
2. Run migration validation, OpenWarrant validation/compile/drift checks,
   Liminal CI/inventory, and independent migration review. Retain exact exits,
   source commits and limitations in the verification record.
3. Propose the exact SAS revision through `war sas propose`; deliver the
   digest-bound request emitted by `war sas accept`. Brian's acceptance is
   required to establish SAS authority. Agents do not submit an acceptance
   response or fabricate authority-register entries.
4. Integrate only after campaign work is preserved. Reconcile source history
   against its final head, rerun appropriate gates, and requalify whenever the
   final baseline no longer meets HAQP eligibility rules.
5. Obtain M17.6 GO and final suite ratification through their separate work and
   decision records. A compiler release leaves Phases 2–12 outstanding.
