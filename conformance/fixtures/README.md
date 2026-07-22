# Conformance fixture inventory

Tracked public and sanitized fixture families are governed here. Locked
acceptance corpora are not inventory inputs and are verified only through their
black-box manifest gate.

| Family | Version | Parser | Canonicalizer | Malformed cases | Golden owner | Confidentiality |
|---|---|---|---|---|---|---|
| basis-selection | v1 | `liminal_revision::resolve` | ordered `WorkspaceBasis` serde | ambiguity and requester-less cases | M06 | public |
| conversion-loss | v1 | `liminal_conformance::pandoc` | `pandoc::normalize` | unsupported and foreign Pandoc nodes | M10 | public |
| identity | v1 | `identity::Config::load` | `Matrix::render` | duplicate, lost, and ambiguous identity | M07 | public |
| ilrp-crash-matrix | v1 | `ToyRun::baseline` | ordered `HitTrace` | third-state and torn-boundary cases | M02/M04 | public |
| malformed-source | v1 | conformance malformed-source class | byte-preserving negative oracle | malformed UTF-8 and syntax cases | Phase 1 | public |
| migration | v1 | conformance migration class | versioned canonical JSON | unsupported-version and corrupt migration cases | Phase 1 | public |
| phase0 | v1 | Phase 0 strict serde loaders | canonical TOML/JSON | unknown, duplicate, empty, dangling, and wrong-type cases | M13/M15 | public |
| phase1 | v1 | Phase 1 source/renderer fixtures | byte-exact Markdown and HTML goldens | malformed source and render-boundary cases | M18/M22 | public |
| repair-dags | v1 | `ScenarioScript::load` | topological step order | cycle, stale prestate, and unsafe candidate cases | M04 | public |
| scenarios | v1 | `ScenarioScript::load_dir` | canonical TOML serde | schema and expectation mismatch cases | M02–M11 | public |
| silence | v1 | `ScenarioScript::load` | byte-empty output oracle | unexpected diagnostic and item cases | M03/M16 | public |
| traces | v1 | `Trace::load` and session TOML loader | denominator event order | invalid event, timeout, and root-cause cases | M11 | sanitized |

Adding a family requires a version, parser, canonicalizer, malformed-case
policy, owner, and confidentiality class. Version history is append-only:
changed semantics create a new version. Golden changes require their owning gate;
fixture authors cannot tune stable profiles against locked acceptance traces.
