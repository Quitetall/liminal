# Pandoc adapter loss report (Phase -1.4)

pandoc: 3.6.1 (pinned) | pipeline: graph -> pandoc-json -> markdown-smart(--wrap=none) -> pandoc-json -> graph
corpus: conformance/fixtures/conversion-loss/pandoc/

| dimension          | in | out | survived | verdict       |
|--------------------|----|-----|----------|---------------|
| blocks             |  5 |   5 | 5/5      | exact         |
| block order        |  - |   - | preserved | exact         |
| ids                |  3 |   3 | 3/3      | exact         |
| text (normalized)  |  5 |   5 | 5/5      | exact         |
| comment relations  |  2 |   0 | 0/2      | DECLARED LOSS |
| foreign raw block  |  1 |   1 | 1/1      | preserved     |

declared capability level: 1 (import/export, declared loss)   [v4 §8.4]
paragraph-only subset second-pass stable: yes              (informative)
