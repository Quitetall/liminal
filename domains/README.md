# domains/ — planned domain crates

This tree is **deliberately empty**. Phase -1 is the falsification laboratory
(v4 Law 14, Part XXII): no domain subsystem may be built until the
Jurisdiction, identity, repair, and Basis experiments retire their risks.
The toy workspace in `crates/liminal-daemon` uses exactly two profiles
(external-file and graph-native, R4 §10) and no domain crates at all.

When domains land, they follow the v4 §117 layout: one crate per domain at
`domains/<name>`, named `liminal-domain-<name>`, joining the workspace
`members` list (root `Cargo.toml`) as each first crate lands — Phase 5 at the
earliest. Domain profiles supply Jurisdiction Contracts automatically
(v4 §7.11); users never author them for ordinary work (Law 3E).

| Directory | Future crate | Spec | Earliest phase (v4 Part XXII) |
|---|---|---|---|
| `prose/` | `liminal-domain-prose` | v4 §66 | Phase 5 |
| `org/` | `liminal-domain-org` | v4 §67 | Phase 5 |
| `academic/` | `liminal-domain-academic` | v4 §70 | Phase 5 |
| `code/` | `liminal-domain-code` | v4 §68 | Phase 5 |
| `notebook/` | `liminal-domain-notebook` | v4 §69 | Phase 5 |
| `table/` | `liminal-domain-table` | v4 §72 | Phase 5 (advanced runtime Phase 12) |
| `slide/` | `liminal-domain-slide` | v4 §73 | Phase 5 (advanced runtime Phase 12) |
| `image/` | `liminal-domain-image` | v4 §74 | Phase 8 |
| `ink/` | `liminal-domain-ink` | v4 §75 | Phase 8 (advanced spatial Phase 12) |
| `audio/` | `liminal-domain-audio` | v4 §76 | Phase 8 |
| `lecture/` | `liminal-domain-lecture` | v4 §77 | Phase 8 |
| `ai/` | `liminal-domain-ai` | v4 Part XIII | Phase 10 |

Note in particular: the agenda projection of the Org domain does **not** own
reconciliation debt — the core Reconciliation Queue exists from Phase -1 and
the agenda only projects it later (Law 3I, R4 §9).
