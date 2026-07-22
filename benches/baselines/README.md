# Phase 1 benchmark evidence

`phase1.json` is the accepted reference baseline and requires Brian's T1
decision under M23. The sampler writes an untracked
`phase1-candidate.json`; `bench-gate` refuses to run until an accepted baseline
exists and verifies exact 30-sample provenance, median, and p95 values.
