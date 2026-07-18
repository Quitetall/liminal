# Identity guarantee matrix (Phase -1.1, M7)

git: git version 2.55.0
config: blake3:5b099f7c0961a82b7224d10c50ca910f85389d564d128c924c602ac1cdcae8a3
seeds: 8

| operation | inline-id | sidecar | content-hash | structural | revision-anchor | managed-graph |
|---|---|---|---|---|---|---|
| rename_move | Preserved(explicit) | Lost | Preserved(content-addressed) | Recovered(1.00) | Preserved(anchored) | Preserved(managed) |
| split | Preserved(explicit) | Lost | Lost | Lost | Lost | Preserved(managed) |
| merge | Lost | Lost | Lost | Lost | Lost | Preserved(managed) |
| copy_paste | Ambiguous | Ambiguous | Ambiguous | Ambiguous | Preserved(anchored) | Preserved(managed) |
| duplicate_identical | Preserved(explicit) | Ambiguous | Ambiguous | Ambiguous | Preserved(anchored) | Preserved(managed) |
| delete_recreate | Lost | Preserved(explicit) | Preserved(content-addressed) | Recovered(1.00) | Lost | Preserved(managed) |
| formatter_rewrite | Preserved(explicit) | Recovered(0.50) | Lost | Recovered(0.85) | Lost | Recovered(0.85) [foreign] |
| external_edit | Lost | Lost | Lost | Lost | Lost | Lost [foreign] |
| git_merge | Preserved(explicit) | Recovered(0.50) | Lost | Recovered(0.79) | Lost | Recovered(0.79) [foreign] |
| git_rebase | Preserved(explicit) | Recovered(0.50) | Lost | Recovered(0.79) | Lost | Recovered(0.79) [foreign] |
| git_cherry_pick | Preserved(explicit) | Recovered(0.50) | Lost | Recovered(0.79) | Lost | Recovered(0.79) [foreign] |
