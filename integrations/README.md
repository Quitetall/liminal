# integrations/ — planned tool integrations

This tree is **deliberately empty**. Phase -1 builds no editor plugins, no
protocol servers, and no tool bridges (v4 Law 14, Part XXII); the toy harness
drives everything through scripted scenarios instead.

Integrations are how sovereign external tools connect (v4 Law 7): an editor
buffer or a file may itself *hold Jurisdiction* over text while Liminal's
graph is derived — these crates declare those boundaries rather than wrap the
tools. They follow the v4 §117 layout: one crate per integration at
`integrations/<name>`, named `liminal-integration-<name>`, joining the
workspace `members` list (root `Cargo.toml`) as each first crate lands —
Phase 2 at the earliest, with Neovim first (v4 §59: Neovim is the first-class
text frontend; the plugin is thin Lua speaking the Liminal Document Protocol
to `liminald`).

| Directory | Future crate | Spec | Earliest phase (v4 Part XXII) |
|---|---|---|---|
| `nvim/` | `liminal-integration-nvim` | v4 §59 | Phase 2 (first integration) |
| `helix/` | `liminal-integration-helix` | v4 §60 | Phase 2+ (same protocol as Neovim) |
| `lsp/` | `liminal-integration-lsp` | v4 §57, §68, §106 | Phase 2+ |
| `cargo/` | `liminal-integration-cargo` | v4 §54 | Phase 4 (build graph) |
| `jupyter/` | `liminal-integration-jupyter` | v4 §69, §105 | Phase 5 |
| `git/` | `liminal-integration-git` | v4 §89 | Phase 6 (the Phase -1 identity torture corpus uses the real `git` CLI directly, not this crate) |
