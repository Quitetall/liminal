# Conformance corpus law

This directory is the `liminal-conformance` workspace member: the
project's entire future test surface, plus the fixtures, corpora, and
golden reports those tests consume. Its data is governed by one law with
four zones. The zones exist because v4 §7.4 and R4 §2.2 forbid the
single failure mode that quietly kills conformance suites: **tuning the
implementation against the data that is supposed to judge it.**

> "A profile cannot graduate by passing only self-authored fixtures."
> — R4 §2.2

## The four zones

| Zone | Rule | Spec |
| --- | --- | --- |
| `fixtures/` | **Profile development. Freely tunable.** Authored by us, inspectable, editable at any time while developing profiles and the harness. | v4 §7.4 ("development corpus"), R4 §2.2 set 1 |
| `corpora/heldout/` | **LOCKED acceptance corpora.** Independently sourced, versioned, hash-manifested, frozen the moment scores are seen. Tuning against it is a process violation, enforced by CI. See `corpora/heldout/POLICY.md`. | v4 §7.4, R4 §2.2 set 2 |
| `corpora/regression/` | **Every discovered failure, minimized, retained forever.** Falsified assumptions are never deleted, even when the subsystem that produced them is rewritten. | R4 §2.2 set 3 |
| `golden/` | **Byte-exact reports.** Golden files are the project's published claims; comparison is byte-for-byte and every regeneration is a reviewed, spec-cited diff. See `golden/README.md`. | v4 §-1.1, §-1.4, §7.4 |

## Layout

```text
conformance/
├── src/                  # harness lib: scenario runner, crash matrix, laws, SLOs, debt meter
├── tests/                # the materialized test catalog (phase gates, laws, classes, SLOs)
├── fixtures/
│   ├── scenarios/        # 6 Phase -1 toy scenarios (R4 §10), *.scenario.toml
│   ├── ilrp-crash-matrix/    # M2/M4 — see its README
│   ├── repair-dags/          # M4
│   ├── basis-selection/      # M6
│   ├── silence/              # M3
│   ├── identity/             # M7
│   ├── malformed-source/     # Phase 1
│   ├── conversion-loss/      # M10 / Phase 4
│   └── migration/            # first persisted-format ADR
├── corpora/
│   ├── heldout/          # LOCKED — POLICY.md is binding
│   └── regression/       # append-only failure archive
└── golden/               # byte-exact reports (identity matrix, anchor recovery, …)
```

## Scenario fixtures

`fixtures/scenarios/*.scenario.toml` script the R4 §10 toy workspace:
`[scenario]` metadata with spec citations, `[[setup.file]]` /
`[[setup.graph]]` / `[[setup.buffer]]` world construction, ordered
`[[step]]` events (buffer edits, saves, foreign edits, Holder outages,
daemon restarts), and an `[expect]` block asserting plan shape,
auto-apply verdict and safety evidence, ILRP terminal state, checker
byte-silence, and Overlay/reconciliation debt. The harness `ScenarioScript`
types in `src/` are the authoritative schema; the six authored scenarios
are the day-one instances.

## Corpus hygiene

Fixture and corpus content is deliberately excluded from spell-checking
and formatting (`typos.toml`, `taplo.toml`): adversarial and malformed
inputs must never be "fixed". If a tool touches bytes under `corpora/`,
the tool is wrong.
