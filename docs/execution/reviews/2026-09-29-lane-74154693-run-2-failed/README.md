# Failed HAQP attempt: run=run-2

This is an archival copy of the review and scope artifacts from the failed
qualification attempt. The receipt's run id is literally `run=run-2`; it is
malformed, and is preserved as produced rather than silently normalized.
The receipt bytes are stored as base64 in `run=run-2.receipt.base64`, because
the original ends in a trailing space. Decode it with `base64 -d` to recover
the exact bytes.

- Fixed source commit: `7415469317059c14d253630a2b174efa4a7c209d`
- Fixed source tree: `7c12e157092d8980f2a72da3d59be8ed8398a917`
- Attempt result: `fail` (298 seconds); this is not qualification evidence.
- Pass 1: two verified findings, A08 and A09.
- Pass 2: pass, zero unresolved findings; it did not reproduce pass 1 A08/A09.
- A08 repeats the file-scoped ruling concern covered by draft R-008; it remains
  a Brian-owned T1 decision, not resolved by this archive.
- A09 was reproduced at `crates/liminal-xtask/src/haq.rs:11437`. The gate
  accepted `unratified — ratified and approved by Brian`. The correction and a
  regression test were committed afterward in `e1618ad1`.
- Empty `lanes.json.parts` and `scope-rows.ndjson` are retained exactly as
  captured. They do not establish completed lane evidence.

`SHA256SUMS` covers every file in this directory except itself. The full
repository merge profile was later run on clean commit `e1618ad1` with the
repository-pinned Pandoc 3.10.2 and passed; that still does not establish HAQP
qualification or M17.6 authority.
