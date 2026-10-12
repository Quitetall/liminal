# HAQP-1 run-1 archive (first run under AM-17.20)

This is the durable archive of the failed HAQP-1 run launched on 2026-10-07 from
base `503caee898d9002b5b42fd08dfb0c24c70a15818` (tree
`7134d46ce52cf9a8c9ba572dca41207a6fbdd2a2`). The receipt records 592 seconds,
`clean=1`, and `result=fail`. This is not qualification evidence.

The reviews stage completed. The ci stage then failed in 2 seconds, because
`taplo` was missing from the host (deleted on 2026-10-03 by a LamQuant
self-hosted CI job's `Swatinem/rust-cache` post step; see F-96).

Pass 1 verified five findings, classed under AM-17.20: A02 (S1), A08 (S1), A04
(S3), A09 (S3), A10 (S3). Pass 2 reported none. The run would have failed on
the two S1 findings regardless of the ci stage.

The receipt, both structured and raw reviewer outputs, the pass-1 session,
scope traces, scope rows, and lane parts are preserved here. `SHA256SUMS`
records each archived file's digest. This evidence stays bound to the failed
base and must not be presented as a later run's evidence.
