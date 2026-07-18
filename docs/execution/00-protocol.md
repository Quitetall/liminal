# Execution protocol — the executor rulebook

This directory contains **execution-grade work orders**: every decision is pre-made,
so any executor — a junior developer or an AI agent — implements mechanically. The
work orders `M01.md`–`M12.md` cover Phase -1 in full mechanical detail; `phases.md`
holds the Phase 0–12 work breakdown; `template.md` is the mandatory skeleton for
every future order. This file is the contract the executor operates under.

Spec citations: "v4 §N" = `spec/v4/liminal_master_architecture_plan_v4.md`;
"R4 §N" = `spec/v4/liminal_architecture_revision_4.md`. The spec is canonical;
the work orders are its mechanical projection.

## 1. Definition of done

A milestone is done when, and only when:

1. Every test named in the work order's **Exit gate** table either (a) flips from
   `#[ignore = "Phase -1 M<n>: …"]` to passing, or (b) — for tests the plan names
   that the tree does not yet contain — is **created un-ignored and born passing**
   in the exact file the work order specifies. Creating a named exit-gate test with
   an `#[ignore]` attribute is forbidden.
2. Assertions are **UNWEAKENED**. The assertion text in each test's doc comment and
   in the work order is the contract. Weakening — loosening a comparison, deleting
   an assert, shrinking a matrix, converting byte-equality to substring, adding
   retries, widening a timeout to hide a hang — is a **spec change**: stop, write an
   ADR, get it accepted before the change lands. `retries = 0` in
   `.config/nextest.toml` is itself a frozen assertion.
3. `just ci` passes end to end (fmt-check, clippy `-Dwarnings`, nextest ci profile,
   doctests, rustdoc `-Dwarnings`, cargo-deny).
4. `just gates` shows exactly the delta the work order predicts (which ignored
   counts decrease, by how much). Any other delta is a finding, recorded in the
   order's **Discovered gaps**.
5. Every previously passing test still passes. There are no known-flaky exemptions;
   a flaky crash test is a finding, never retried away.

## 2. API-freeze rule

The scaffold's public signatures — everything in each work order's **Frozen
surfaces** list — are frozen. The executor implements `todo!()` bodies; the executor
does not rename, re-parameterize, re-home, or "improve" frozen items.

**Amendment procedure** (the only path when a frozen signature cannot work or a
needed item does not exist):

1. Stop implementing the affected step.
2. Write the amendment into the work order's **Amendments** section as
   `AM-<milestone>.<n>`: what changes, why the frozen shape cannot work, which call
   sites are affected.
3. Append one line to `docs/adr/0008-phase-minus-1-execution-amendments.md`
   (created at M01 from `docs/adr/template.md`):
   `AM-<m>.<n> (M<nn>): <one sentence>`.
4. Only then edit code.

Purely **additive** items (new modules, new pub fns, new trait impls on existing
types, new constants, struct fields added with `#[serde(default)]`) are
pre-authorized when the work order lists them; list any others in Amendments anyway
for the audit trail. Every AM is ratified or reverted at M12 (the amendment-ledger
ADR audit).

## 3. Ambiguity procedure

If a work order under-specifies anything — a field's serialization, an ordering, an
error path, an env var, a filename — the executor **STOPS** on that step and records
the gap in the order's **Discovered gaps** section: what is ambiguous, the candidate
readings, which step is blocked. Continue only with later steps that provably do not
depend on the gap; otherwise halt the milestone and surface the gap.

**Never improvise semantics.** This project is a falsification laboratory: an
improvised semantic silently becomes the thing under test, and the experiment stops
measuring the spec. An invented behavior that happens to pass poisons every
downstream conclusion.

## 4. The mechanical loop

1. Read the work order top to bottom before touching any file.
2. Implement steps strictly in listed order. Do not reorder, batch, or "get ahead".
3. After each step, run that step's listed command and compare against the listed
   expected output.
4. Flip (or create) exactly the tests the step lists — no others.
5. After the last step: `just ci`, then `just gates`; verify the predicted delta.
6. Update the order's checkboxes (`- [ ]` → `- [x]`) as each step's command passes.
   The checkbox edit belongs in that step's commit.

## 5. Standing prohibitions

Mirror of `docs/implementation-plan.md` §4 — violating one is an ADR-level event:

1. `CompiledJurisdictionPlan`, indexed dispatch, generated policy code (v4 §125).
2. Any CRDT or OT implementation (R4 §11.7).
3. Real parser, CST, or rope — the M3 ~50-line toy grammar is the entire Phase -1
   surface (v4 §9, Law 14).
4. Agenda subsystem or vocabulary (`no_agenda_symbols` greps for it) (R4 §9).
5. Sync, cloud, or Git-as-sync (v4 §88).
6. Real editors or the Liminal Document Protocol (v4 §58).
7. Persistent socket daemon — `liminald` stays exec-and-exit.
8. Production store work (speed, GC, generality) — the toy store exists to be
   crashed and read with eyes (ADR-0007).
9. Performance work, salsa, benchmark gates (Law 14, v4 §116).
10. `FacetId` or any third primitive (v4 §7.2, Law 1).
11. Rich UI, badges, notifications (R4 §3).
12. Schemas, macros, transforms, plugins, AI, HTML backend (phase-gated).
13. Overlay GC or archival tooling (Law 3I).
14. **Held-out-corpus ban:** never open, read, tune against, or hash-update
    anything under a locked `heldout/` corpus version. Drift is caught by
    `verify_heldout_manifest`; the fix for drift is a new corpus version, never an
    edit (v4 §7.4).

Plus the **test-weakening ban** (§1.2) and: never delete or rename an `#[ignore]`d
backlog test — the ignored set is the backlog; it shrinks only by passing.

## 6. Commit conventions

- One commit per work-order step: `M<nn>.<step> <area>: <imperative summary>` —
  e.g. `M02.4 liminal-daemon: FsExecutor verify/apply for WriteFile`.
- The step's code, its tests, and its checkbox flip land in the same commit. Never
  mix steps.
- Amendments commit separately, **before** the code that needs them:
  `M<nn> amendment AM-<m>.<n>: <summary>` (includes the work-order Amendments entry
  and the ADR-0008 line).
- Commit bodies cite spec sections (`v4 §7.8`, `R4 §10`) for anything semantically
  load-bearing.

## 7. Executor tiers

Every work-order step carries a tier tag `[T1]`–`[T4]`. A tier describes the
**level of responsibility the step demands** — not which model or person
executes it. Anyone may execute above a step's tier; never below. The tag is a
floor of care.

- **T1 — mission critical; human-grade / extreme rigor.** An error here can
  pass the tests and still poison the architecture, or the step's output IS
  constitutional. The executor must bring the project's highest scrutiny, and
  the result gets an independent second look before it lands. T1 steps include:
  the ILRP driver core, the auto-apply conjunction, freezing any constitutive
  golden, the corpus-lock ceremony, the M12 audit and go/no-go, and Phase-gate
  work-order authoring.
- **T2 — implementation-level judgment.** The algorithm is fully specified, but
  realizing it well requires real engineering decisions: data-structure and
  error-path choices, edge handling, invariant-preserving refactors. Mistakes
  are usually caught by the oracles, but poor judgment degrades the experiment.
- **T3 — minor implementation details only.** The shape is fully given; the
  executor writes idiomatic Rust filling small gaps (naming, small helpers,
  test scaffolding). No semantic choices exist to make.
- **T4 — mechanical.** Pure transcription and task execution: code or config
  given verbatim in the order, file moves, ignore-flips, checkbox close-outs,
  running listed commands.

**Blanket rules (override any step tag):**

1. Accepting any golden under `conformance/golden/` — and any `cargo insta
   review` acceptance of a NEW snapshot — is a **T1 act**: a golden is spec.
2. The escalation triggers are tier-independent: an ambiguity (§3), a red
   `crash_*` test, any assertion edit, any new Amendment, or any Discovered gap
   promotes the moment to **T1** no matter what the surrounding step is tagged.
3. A lower-tier executor who cannot complete a step WITHOUT exceeding its tier
   (i.e. a "T4" step turns out to require a decision) has, by definition, found
   a Discovered gap — stop and record it (§3).

Delegation guidance: T4 and T3 are safely delegable to cheaper/faster agents
prompted with this protocol (§3 verbatim). T2 is delegable to a strong mid-tier
executor with a T1-level review of the diff. T1 is never delegated below the
project's most capable executor, and its artifacts get human sign-off.

## 8. Shared conventions all orders rely on

- **Aux namespaces:** every aux-namespace constant lives in
  `liminal_graph::store::ns` (`ILRP_INTENT`, `JUR_ALIAS`, `JUR_PLAN`,
  `JUR_DECISION`, `JUR_REPAIR`, `JUR_OVERLAY`, `JUR_OVERLAY_LOG`, `JUR_RECONCILE`,
  `SYS_BLOB`, `SYS_UNAVAILABLE`, `SYS_CLOCK`, `SYS_EPOCH`). Introduced at M02/M03;
  no namespace string literal appears at a call site.
- **Milestone tests:** new milestone tests live in
  `conformance/tests/milestones/m<nn>.rs` under a `milestones/main.rs` that
  declares the modules. Crash tests are named `crash_*` anywhere (routed into the
  serialized nextest crash group).
- **Name aliases:** where `docs/implementation-plan.md` uses a shorthand test name,
  the work order states the alias once (e.g. `capture_never_rejected` ≡ the gate
  test `no_edit_is_rejected`); tests are never duplicated to satisfy both names.
- **Crash-matrix scope:** `runnable_crash_scenarios()` in
  `conformance/src/harness.rs` is the single source of truth for which scenarios
  the derived crash matrix covers. M02 seeds it; M04 appends.
- **Amendment ledger:** `docs/adr/0008-phase-minus-1-execution-amendments.md`,
  one line per AM, audited at M12.
