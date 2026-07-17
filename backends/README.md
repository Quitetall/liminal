# backends/ — planned output backends

This tree is **deliberately empty**. Phase -1 forbids building projections
and renderers before the falsification gates pass (v4 Law 14, §125); the only
adapter work in Phase -1 is the read-only Pandoc boundary *prototype*
(v4 Part XXII §-1.4), which lives in the conformance harness, not here.

When backends land, they follow the v4 §117 layout: one crate per backend at
`backends/<name>`, named `liminal-backend-<name>`, joining the workspace
`members` list (root `Cargo.toml`) as each first crate lands — Phase 1 at the
earliest, with HTML first (the Phase 1 vertical slice is source-to-HTML,
v4 Part XXII; §27 defines the HTML fast path). Every backend is a declared
projection: conversions state what they require, preserve, introduce, and may
discard (Law 8), and human and AI projections are separate compiler targets
(Law 10).

| Directory | Future crate | Spec | Earliest phase (v4 Part XXII) |
|---|---|---|---|
| `html/` | `liminal-backend-html` | v4 §27 | Phase 1 (first backend) |
| `pandoc/` | `liminal-backend-pandoc` | v4 §105, §118A | Phase 1 (adapter, not canonical graph) |
| `terminal/` | `liminal-backend-terminal` | v4 §65 | Phase 2 |
| `web/` | `liminal-backend-web` | v4 §61 | Phase 9 |
| `ai/` | `liminal-backend-ai` | v4 Part XIII | Phase 10 |
| `pack/` | `liminal-backend-pack` | v4 §100 | with the first packed-interchange ADR |
