# Architecture Decision Records

ADRs record **repository and implementation decisions** — toolchain, layout,
test stack, supply chain, throwaway substrates. Anything **spec-observable**
(kernel semantics, Contracts, protocol surface, conformance-visible behavior)
goes through `spec/rfc/` instead, per the RFC-and-ADR process v4 §131
recommends. ADRs are numbered sequentially and are **immutable once
accepted**: a reversed decision gets a new ADR that marks the old one
`superseded` — the old text is never edited, because the audit trail of *why*
we believed something is itself falsification evidence. Phase gate outcomes
(v4 Part XXII, R4 §10) also land as ADRs, referencing the conformance tests
and corpus evidence that decided them. Every ADR states what would falsify or
reverse it — this project runs on falsification (Law 14), and a decision that
cannot name its own kill-condition is not a decision, it is a hope.

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [0000](0000-record-architecture-decisions.md) | Record architecture decisions | accepted |
| [0001](0001-apache-2.0-license.md) | Apache-2.0, single license | accepted |
| [0002](0002-toolchain-edition-msrv-policy.md) | Pinned stable toolchain, edition 2024, no MSRV promise pre-publish | accepted |
| [0003](0003-cargo-monorepo-workspace-layout.md) | One Cargo monorepo workspace per v4 §117 | accepted |
| [0004](0004-justfile-task-runner.md) | `just` as task runner; xtask deferred | accepted |
| [0005](0005-test-stack-nextest-insta-proptest-divan.md) | Test stack: nextest + insta + proptest + divan | accepted |
| [0006](0006-cargo-deny-supply-chain.md) | cargo-deny for supply-chain policy | accepted |
| [0007](0007-hand-rolled-phase-minus-1-toy-store.md) | Hand-rolled Phase -1 toy store per v4 §92 | accepted |
| [0008](0008-phase-minus-1-execution-amendments.md) | Phase -1 execution amendments ledger | accepted |
| [0009](0009-cap-identity-promises-at-phase-minus-1-evidence.md) | Cap identity promises at Phase -1 evidence | accepted |
| [0010](0010-freeze-measured-projection-capability-levels.md) | Freeze measured projection capability levels | accepted |
| [0011](0011-freeze-jurisdiction-measurement-denominators.md) | Freeze Jurisdiction measurement denominators | accepted |
| [0012](0012-ratify-phase-minus-1-amendment-ledger.md) | Ratify the Phase -1 amendment ledger | accepted |
| [0013](0013-phase-0-go-no-go-after-falsification-laboratory.md) | Phase 0 go/no-go after the falsification laboratory | accepted |

## Writing a new ADR

Copy [template.md](template.md), take the next number, keep one decision per
ADR, and fill in the falsification line. Open questions the spec itself
reserves for ADRs are listed in v4 Part XXIII (§119–§131) — when one of those
is decided, the ADR must cite the Phase -1 evidence that decided it.
