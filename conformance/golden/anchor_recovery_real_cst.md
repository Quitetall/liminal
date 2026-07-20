status: measured
grammar_adr: docs/adr/0015-select-phase-1-source-grammar.md
substrate: tree-sitter-md 0.5.3 + tree-sitter 0.26.3
git_commit: 94228049ab59a12cb9cc1ef52a7331d5ab9ffdd5
measurement_tree: 3d053c9519ce646323653b6d52180bbe95ff314a
declared_level: 2
merge_recovered: 8
merge_total: 8
merge_rate: 1.000000
wilson95_low: 0.675592
wilson95_high: 1.000000
seed_set_sha256: e6edb440a7e9567c226bb3be36789f8a0cd905fee95ec7f30f685b4875f74c8e
elapsed_ms: 2

level_3_blocker: no independently qualified lossless native CST serializer
upstream_risk: tree-sitter-md documents known Markdown output inaccuracies

| case ID | edit class | seed | expected anchor | observed anchor | outcome | declared loss |
|---|---|---:|---|---|---|---|
| delete-anchor-negative | delete | 17202 | <lost> | <lost> | lost | foreign edit removes every byte of the anchored selection |
| duplicate-anchor-negative | duplicate | 17201 | <ambiguous> | <ambiguous> | ambiguous | foreign edit creates two CST-compatible exact candidates |
| insert-prefix | insert | 17103 | 20..36 | 20..36 | recovered | none |
| malformed-nul-byte | malformed | 17303 | 9..28 | 9..28 | recovered | none |
| malformed-unclosed-fence | malformed | 17301 | 7..26 | 7..26 | recovered | none |
| malformed-unclosed-link | malformed | 17302 | 1..20 | 1..20 | recovered | none |
| wrap-mark | mark | 17104 | 4..18 | 4..18 | recovered | none |
| merge-blockquote-lines | merge | 17003 | 9..24 | 9..24 | recovered | none |
| merge-code-boundary | merge | 17006 | 5..20 | 5..20 | recovered | none |
| merge-emphasis-boundary | merge | 17005 | 8..23 | 8..23 | recovered | none |
| merge-heading-body | merge | 17004 | 3..18 | 3..18 | recovered | none |
| merge-link-boundary | merge | 17007 | 5..20 | 5..20 | recovered | none |
| merge-list-continuation | merge | 17002 | 8..23 | 8..23 | recovered | none |
| merge-paragraph-gap | merge | 17001 | 27..42 | 27..42 | recovered | none |
| merge-table-adjacent | merge | 17008 | 0..15 | 0..15 | recovered | none |
| move-section | move | 17101 | 18..32 | 18..32 | recovered | none |
| split-paragraph | split | 17102 | 8..23 | 8..23 | recovered | none |
