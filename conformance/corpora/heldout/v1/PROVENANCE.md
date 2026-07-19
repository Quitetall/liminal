# heldout/v1 provenance (assembled 2026-07-19, user-ratified composition)

Every import: full first-parent history via `tracegen import` (AM-11.5),
zstd-19 in-tree (AM-11.6), exact base/head SHAs + commit counts + skip
counts recorded in each trace's own header `source` field. Traces embed
file contents from the source repositories; licenses below.

| trace | content class | source | license |
|---|---|---|---|
| import-rust-book | structured technical prose (md) | github.com/rust-lang/book | MIT/Apache-2.0 |
| import-tldr-pages | many-small-files, heavy i18n (md) | github.com/tldr-pages/tldr | CC-BY-4.0 |
| import-rust-rfcs | merge-heavy multi-author argument (md) | github.com/rust-lang/rfcs | MIT/Apache-2.0 |
| import-ripgrep | systems code + docs + config (rs/md/toml) | github.com/BurntSushi/ripgrep | MIT/Unlicense |
| import-serde | macro-heavy refactor-torture code (rs/md/toml) | github.com/serde-rs/serde | MIT/Apache-2.0 |
| import-hott-book | collaborative mathematics in LaTeX (tex/md) | github.com/HoTT/book | CC-BY-SA-3.0 |
| import-shakespeare-hamlet | dramatic verse + prose (xhtml/opf) | github.com/standardebooks/william-shakespeare_hamlet | public domain / CC0 |
| import-frankenstein | long-form novel, editorial history (xhtml/opf) | github.com/standardebooks/mary-shelley_frankenstein | public domain / CC0 |

Synthetic traces (12): `tracegen` generators, seeds disjoint from dev
(git 33/44, adversarial 77/88 vs dev 11/22/55/66), both profiles.

Deliberately absent from v1 (no runtime can replay them yet; POLICY
versioning adds them with v1 scores retained):
- **Wikipedia edit history** — not in git; requires a MediaWiki-dump
  importer (named v2 candidate; do not fake with a lookalike repo).
- Images / audio-interval / spreadsheet classes — Phase 8/12 runtimes.
- Real consented editor sessions (Consent::ConsentedAnonymized) — none
  captured yet.
