# Rust payload constructor development evidence

Baseline: `9bd14be7198b2fd9cd47c8804b7c0d89add81d64`. Candidate implementation
is isolated from main in `/mnt/4tb/liminal-formal-runner-2026-09-14`.
No qualification, formal adoption, or phase authorization follows from this work.

The installed-tree comparator remains exit 1: two selected archive empty
directories are absent from the installed tree. The bundled installer is
manifest-driven, but this is not a trace of historical rustup behavior. The new
constructor does not weaken that comparison: it preserves all selected payload
directories in a fresh root. Archive observations are retained in
`/mnt/4tb/liminal-formal-evidence/reviews/rust-empty-directory-semantics.md`.

The first constructor draft and initial ten controls were developed together;
do not describe that initial slice as observed red-before-green. Subsequent
adversarial controls established actual failures before their fixes:

1. Unknown top-level component directories were silently discarded. A real tiny
   archive control returned success when refusal was expected. Closed metadata
   admission now rejects them. Actual exits 1 then 0 are retained in
   `rust-constructor-metadata-red.log` and `rust-constructor-metadata-green.log`.
2. Independent source review identified aggregate budgets enforced too late.
   A two-archive fixture crosses the global member budget before a later invalid
   path. The old code diagnosed the later path instead of the limit, failing the
   expected early-refusal assertion. Remaining byte/member budgets now pass into
   each archive stream. Independent follow-up verified the finding resolved.
   `rust-constructor-aggregate-red.log` preserves the actual exit-1 run.
3. That same run also contains an oversized-fixture writer error: Python's tar
   writer requires a body for nonzero regular files. This was not a constructor
   defect. The corrected fixture writes an oversized tar header without its body;
   the constructor refuses the declared size before attempting body reads.

All logs above are under `/mnt/4tb/liminal-formal-evidence/reviews/`.
Final command:

```sh
python3 -B -m unittest discover -v -s verification/proof -p 'test_*.py'
```

Observed exit 0, 105 tests passed in 5.350 seconds. Raw log:
`rust-constructor-support-green.log`. Fourteen constructor controls use the public
interface and real tiny xz archives; they cover complete payload/empty directory
preservation, archive and channel drift, exact package presence/version, output
preservation and parent aliases, unsafe/duplicate paths, symlinks, missing
declared payload, unknown top-level content, aggregate member refusal, identical
versus conflicting file overlaps, and oversized file-header refusal.

An additional public-interface process control sets RLIMIT_FSIZE to two bytes,
ignores SIGXFSZ in that child, and observes ToolchainFailure from writing the first
three-byte fixture file. The fresh destination and its two-byte partial file
remain; all input archives remain unchanged. No filesystem methods are mocked.
The focused suite now contains 15 controls, observed exit 0 in 2.236 seconds;
raw log `rust-constructor-retention-control.log`. The complete support suite now
contains 106 controls; its next full CI run is separate evidence.

## Real construction and independent comparison

The constructor source remained SHA-256
`6ae3b29f486f0fab880a63c58856ac7f5ee6572b2f14b86f405521c34e7ca858`.
The exact official channel manifest pin was
`03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf`.
Its six archive hashes were rechecked by both constructor and independent
comparator. Acquisition and individual archive bindings remain in the external
probe records rather than being inferred from installed-toolchain metadata.

Real construction ran once at
`/mnt/4tb/liminal-formal-evidence/probes/rust-constructor-real-v1/`.
The external execution-helper copy differs from committed command.py at only the
two recorded file-size-limit literals: 64 MiB to 256 MiB. Its exact diff is
`command-copy.diff`. Committed command.py remains unchanged. Observed limits were
CPU quota 200%, memory high/max 2/4 GiB, zero swap, 256 tasks, 256 MiB per file,
and ten minutes. Child exit 0; peak memory 643,727,360 bytes; CPU 20.04848 seconds.
Cleanup established the unit absent. Producer output has 6,822 files and 1,358
directories and retains qualification=false. Producer receipt SHA-256:
`3ffe031a1fc351395ad8f12ffd8026447d30d082aa9c558ef5b86ad93956ad09`.
Command receipt in sibling `rust-constructor-real-v1-command-parent/run/`:
`9298f0acb19f373697c93590f736c46c348f07e612b760adc4171ed3ba165071`.

The independent comparator imports no constructor and consumes no producer map.
It streams all six pinned archives to derive expected complete payload, then
separately scans output paths, types, modes, sizes and bytes. Review corrected its
raw terminal-slash handling and pre-read aggregate limits before execution.
Its single positive run observed child exit 0 and exact equality of 8,181 rows
including root, with 1,559,067,836 regular bytes and no missing/extra/different
entries. Standard command limits, including zero swap, were observed; peak memory
1,032,757,248 bytes; cleanup established absence. Evidence:
`rust-constructor-independent-v1/result.json`, SHA-256
`0de883617f6bdab387c7a0e7c65f0eec9d9681451f70cccf0ead9b6bde14579d`;
sibling command receipt SHA-256
`5b2b76505dcf01c21e2a56e8e5a9c2b1eb36c66b2b608133d36715f5516d1977`.

A negative control overlays only bin/rustc with three bytes inside a read-only
bubblewrap namespace. The unchanged comparator returns actual exit 1 and reports
exactly that path different, with no missing/extra paths. Controller exit 0 means
the expected refusal was observed, not that the corrupted tree passed. Original
compiler hash/mode/size remain unchanged before and after; no payload was edited.
Evidence under `rust-constructor-negative-v1/`: controller-result SHA-256
`bedb04c47cee4eda3edb5427e5dc1f5513a022a05177696970a2dbc0a649fece`,
command receipt SHA-256
`a1fe6222082c33d98fe6facb1913ecae875f88eb9d8dade9cc88edbd9d5c951e`.

## Review and remaining boundary

Fresh stdio LAMU review_diff completed with PASS WITH NITS; the critic tail was
truncated and contained verified false positives. Exception translation,
second-pass regular-file checking and umask handling already exist as intended.
The useful partial-write coverage request produced the real process control above.
Full disposition is retained in external `reviews/rust-constructor-diff-disposition.md`.
Independent source Spec and Standards reviews passed after the aggregate fix.

Remaining before integration: full CI; mandatory commit review and verified
disposition. The returned producer map is not its own oracle. Actual
compiler selection, cold proof cycles and formal-command aggregation remain
separate unfinished work in `verification/PLAN.md`.
