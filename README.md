# Liminal

Liminal is a local-first, multimodal, revisioned graph runtime whose semantic
universe contains exactly two primitives: **Node** and **Relation** (v4 Law 1).
Documents, text, tables, tasks, transcripts, slides, ink, histories, and
policies are all compositions of those two. A tiny kernel does not make the
total system simple (v4 Law 2A) — ownership, write routing, identity, foreign
edits, and replication stay hard — so all of that complexity is deliberately
concentrated in one typed subsystem, **Jurisdiction** (v4 §7, R4 §1), instead
of leaking separately into every editor, resolver, synchronizer, and renderer.
Liminal is not another note-taking app and not a plan to rewrite Neovim, Word,
Excel, Pandoc, or Jupyter: it is the infrastructure layer *beneath* editors,
notes, papers, and media (v4 §0). Existing tools remain sovereign and may
themselves hold Jurisdiction over the data they govern (v4 Law 7).

> **STATUS: PRE-ALPHA — PHASE -1 FALSIFICATION LABORATORY.**
> **Nothing here is usable. The current goal is to DISPROVE the architecture
> as cheaply as possible, not to ship features (v4 Law 14, Part XXII).**
> Risk-retirement order precedes dependency order: identity round-trips,
> cross-Jurisdiction repair, crash-recoverable cross-Holder mutation (ILRP),
> Overlay lifecycle, and concurrent-buffer Basis selection are being tested
> with a deliberately small interpretive toy before any production parser,
> daemon, or optimization exists. If a Phase -1 gate fails, the constitution
> is revised — that outcome counts as success for this phase.

## What Liminal is

- A semantic kernel of Nodes and Relations, with everything else derived
  (v4 §4–6).
- One Jurisdiction resolution for every independently governable Node or
  Relation at an immutable Workspace Basis (v4 Law 3, Law 3D).
- A repair model where *save, sync, merge, writeback, and Promotion are one
  mechanism* — a RepairPlan dependency DAG executed by one interpreter under
  the Intent-Logged Repair Protocol (R4 §4–7).
- Local-first and offline-normal (v4 Law 9), recoverable without its richest
  runtime (v4 Law 16), with explicit effects (v4 Law 5) and declared
  conversion loss (v4 Law 8).
- Separate compiler targets for human and AI projections (v4 Law 10).

## What Liminal is not (v4 §2)

Non-goals are load-bearing here. Liminal does not:

- Replace Neovim, Helix, Emacs, or VS Code, or ship a mandatory editor
  distribution.
- Replace programming-language compilers, language servers, debuggers, or
  package managers.
- Reimplement every Pandoc reader and writer.
- Invent a new CRDT before evaluating existing approaches.
- Invent new image, audio, video, or PDF formats without necessity.
- Promise to round-trip every historical DOCX or PPTX quirk.
- Force every domain into one physical storage layout, or expose raw graph
  operations as the only public API.
- Make Internet connectivity a prerequisite for ordinary use.
- Require users to understand the internal graph — or the words
  "Jurisdiction", "Holder", "Overlay", "Promotion" — to write a normal note
  (v4 Law 3E, R4 §3).

## Repository map

The monorepo layout follows v4 §117, plus one crate added by RFC-0001.

```text
liminal/
├── spec/            The constitution. spec/v4/ holds the canonical documents
│   │                verbatim (never edited); topical indexes and spec/rfc/
│   │                carry amendments (RFC-0001 adds liminal-jurisdiction).
├── crates/          22 workspace crates.
│   │                Tier A (real Phase -1 lab code):
│   │                  liminal-id           identity vocabulary (§4.1, §19, §44)
│   │                  liminal-graph        Node/Relation, transactions, toy
│   │                                       crash-safe store (§4–5, §86, §92)
│   │                  liminal-source       file-Holder staging (§7.8, §8.5)
│   │                  liminal-revision     WorkspaceBasis + Perspectives
│   │                                       (§7.5, R4 §8)
│   │                  liminal-jurisdiction the complexity sink: Contracts,
│   │                                       checker, Overlays, RepairPlan,
│   │                                       ILRP (§7, R4 §4–9; RFC-0001)
│   │                  liminal-daemon       toy harness `liminald` + crash
│   │                                       injection (not a real daemon)
│   │                  liminal-cli          `lim` — check / overlays / repairs
│   │                Tier B (reserved stubs, doc-only until their phase):
│   │                  text, cst, hir, cir, query, transform, format,
│   │                  resource, history, resolver, sync, plugin-api,
│   │                  wasm-host, lua, protocol
├── conformance/     liminal-conformance: the §112 correctness laws, the
│                    R4 §10 Phase -1 gates, fixtures, and the held-out
│                    corpus policy. Ignored tests ARE the multi-year backlog.
├── benches/         liminal-benches: the §116 benchmark harness. Baselines
│                    only — never CI gates until Phase 1 (Law 14).
├── domains/         Planned domain crates — deliberately empty. See README.
├── backends/        Planned output backends — deliberately empty. See README.
├── integrations/    Planned tool integrations — deliberately empty. See README.
├── apps/            Planned rich clients (Phase 9) — deliberately empty.
└── docs/            docs/implementation-plan.md (Phase -1 milestones),
                     docs/execution/ (execution-grade work orders), and
                     docs/adr/ (architecture decision records).
```

## Reading order

1. [`ARCHITECTURE.md`](ARCHITECTURE.md) — the codemap: crate layering, the
   correctness laws, and the boundary invariants a contributor must not break.
2. [`spec/v4/`](spec/v4/) — the canonical architecture: the master plan
   (cited as "v4 §N") and Revision 4 (cited as "R4 §N"). These documents are
   the constitution; code cites them, never the reverse.
3. [`docs/implementation-plan.md`](docs/implementation-plan.md) — the Phase -1
   milestone plan, the crash-injection architecture, and the explicit list of
   what must NOT be built yet.
4. [`docs/execution/`](docs/execution/00-protocol.md) — execution-grade work
   orders: every milestone's algorithms, schemas, constants, and step-by-step
   checklists, pre-decided so implementation is mechanical. Start with the
   protocol, then the current milestone's `M<nn>.md`.

## Development quickstart

The toolchain is pinned by `rust-toolchain.toml`; with
[rustup](https://rustup.rs) installed, the first `cargo` invocation fetches it
automatically. Tasks run through [`just`](https://github.com/casey/just):

```sh
just setup   # one-time: install cargo-nextest, cargo-insta, cargo-deny,
             # taplo-cli, typos-cli
just ci      # everything CI runs: fmt/lint (clippy -Dwarnings), tests,
             # doc build, cargo-deny
just gates   # the spec-debt meter: per-phase passed/ignored test counts —
             # the project's live progress bar
```

There is no application to run. The closest things are the toy binaries:
`cargo run -p liminal-cli -- --help` and `cargo run -p liminal-daemon -- --help`.

## License

Apache-2.0 (see [`LICENSE`](LICENSE); v4 §131, ADR-0001). Unless you
explicitly state otherwise, any contribution intentionally submitted for
inclusion in Liminal is licensed as Apache-2.0, without any additional terms.
