# apps/ — planned rich clients

This tree is **deliberately empty**. Rich clients are Phase 9 work
(v4 Part XXII: "Rich web, phone, and tablet clients"); Phase -1 builds no
editors, no UI, and no client runtimes of any kind (Law 14, §125).

When they land, apps follow the v4 §117 layout: one crate per client at
`apps/<name>`, named `liminal-app-<name>`, joining the workspace `members`
list (root `Cargo.toml`) as each first crate lands. Clients are frontends
over the same graph, protocol, and Jurisdiction model — a phone dirty buffer
is a first-class `ClientScoped` Basis component like any other (v4 §7.5,
R4 §8), which is why the Phase -1 toy already simulates one.

| Directory | Future crate | Spec | Phase (v4 Part XXII) |
|---|---|---|---|
| `web/` | `liminal-app-web` | v4 §61–62 | Phase 9 |
| `mobile/` | `liminal-app-mobile` | v4 §63 | Phase 9 |
| `tablet/` | `liminal-app-tablet` | v4 §64 | Phase 9 |
