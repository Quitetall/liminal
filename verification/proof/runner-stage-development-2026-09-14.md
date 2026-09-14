# Runner staging development record

`stage_runner(repository: Path, commit: str, destination: Path) -> dict` stages
only these committed files:

- `verification/proof/{run,stage,dependencies,toolchain,distribution,command,observation,sandbox}.py`

The exact lowercase 40-hex commit is resolved with the existing hermetic,
30-second bounded Git plumbing. `stage_source` retains its prior default
projection and behavior. Runner blobs are limited to 1 MiB each and 8 MiB total,
must be UTF-8 without NUL, and must have Git mode `100644` or `100755`. All eight
blobs, modes and hashes are validated before creating a new private external
destination. Writes are exclusive, executable mode is preserved, existing output
is untouched, and partial write failure remains for diagnosis.

Success returns only `runner-staged`, `qualification: false`, the requested
commit, and per-file SHA-256/mode maps. The constructor does not enumerate or use
working files, import staged modules, execute a command, authenticate runner or
commit authority, establish input closure, or qualify proof. The copied
projection requested commit `736ec3d1bb3fc43bc2485b291774900944c1a9b1`, while
the constructor itself remained uncommitted during this record. A caller must
independently authorize/compare both identities and launch the staged tree using
an isolated interpreter under the trusted-host boundary. The requested projection
commit and the executing constructor hash below are distinct identities.

Seven real scratch-Git public controls cover the exact projection despite dirty
files and an opaque untracked cache; malformed/unknown/missing/non-file selection;
symlink and Gitlink modes; an existing sentinel destination; oversize, NUL and
non-UTF-8 blobs; and stale timestamp-valid bytecode. The first public tracer
failed because `stage_runner` was absent, then passed after implementation. Its
RED/GREEN logs have SHA-256
`25a75dcdb45074bebead88487d82b6418448d55527908d26c65320031cd2d1f5` and
`07b5db7710bd9f585fea4f12b245d0a9966fa2e80277cc760648619eb4231bcd`.
Later controls were green on arrival.

Review found that the original stale-bytecode check executed a script as main,
which does not exercise module-loader cache selection. It was replaced with a
two-sided `importlib` loader control: the original checkout loads stale bytecode,
while the isolated staged path loads the committed fresh source. The same
follow-up added a real index Gitlink refusal. Focused follow-up passed two tests;
log SHA-256 is
`b1abddc0c753b8a14ba70581e5dcd8b1bb43379d111c409a9b2dddf250f2e6df`.

Final proof-support discovery passed 136 tests in 5.960 seconds, exit 0. Log
`/mnt/4tb/liminal-formal-evidence/reviews/runner-stage-development/review-followup-all-proof-green.log`
has SHA-256
`79d8eede6f944e94ec89fe39c4c54aa15ffb4363f739adde0035a58a31709481`.
Final implementation/test SHA-256 values are
`973d0715a8dd40078d0dc1985fcb8fb5a43be63817ed41de8c062840185cf77a`
and `d4dccea284692c40e9e33a246bc509a6d535c8389203277a89feb9175fa9c6a0`.
This is support evidence, not full CI, execution, review, or qualification.

Independent comparison then exited 0 and matched 11 rows: the mode-0700 root,
two mode-0755 directories, and eight files. A separate copied output with only a
comment mutation in `sandbox.py` exited 1 and reported exactly that coordinate.
The comparator's initial expected root mode was corrected from 0755 to the
constructor's specified 0700 before either recorded run; there is no preserved
failing comparison result to claim. Under
`/mnt/4tb/liminal-formal-evidence/reviews/runner-stage-development/`, SHA-256
values are `fa618c5640e8e93c2a40c2e470f7cb01f2ca625b46c6d9bf8ca17d63ed65ed64`
for `compare_runner_stage.py`,
`8613d1b41ea57989d1b9274e24b11c1c5adf697e09ea378f7be5538489495252`
for `positive-comparison.json`, and
`2ea00e4a927909ccf7259cf74546cd2c49fb423b1dde797be07c209ef5d62f05`
for `negative-comparison.json`. This binds the staged projection to the requested
commit under that comparator; isolated execution and authority remain pending.

## Commit-review follow-up

Full CI at `f032ce660889a985052520d08074fdac62c796ad` completed with child exit
0, unchanged source, 620 nextest tests passed / 47 skipped, 22 bootstrap and 136
proof-support controls, and 33/33 canaries. The actual LAMU commit review returned
primary and critic PASS WITH NITS. Its directory-mode observation was confirmed:
the intermediate directory created by `parents=True` inherited umask-dependent
permissions. The mode-0700 root prevented cross-user exposure, but construction
metadata was not deterministic. This follow-up changes only runner staging;
the historical source-staging behavior is unchanged.

A public-seam test runs separate interpreter children under umasks 000, 022 and
077, without changing the parent's umask. On the old code, it failed for 000
(intermediate 0777) and 077 (intermediate 0700); actual test exit was 1. Explicit
creation and mode normalization of the root and two fixed child directories
made the same test pass, exit 0. Exclusive file writes and partial-output
retention remain intact. The positive byte/hash fixture also uses independently
known `abc` and `hello` digests rather than eight identical payloads.

Evidence is under `/mnt/4tb/liminal-formal-evidence/reviews/`:

- `runner-mode-red.log`: `323efa56163893e2cfae2f249a24ddee3b0484a5a2e044e865b7cfb309e7d9c3`
- `runner-mode-green.log`: `c9921cc137de3dbc5a1e098ca9c64e5c664017fdc03b581f056c391b22049657`
- `runner-mode-all-green.log`: `0dd4d3550170cadf2d791b74480e39ae748401b607cff518e501e08b1d23822f`

The final support run passed 137 tests in 6.020 seconds, actual exit 0. Current
implementation/test SHA-256 values are
`a317751f85b9d6ac8537281b53248cb64aeb8da7cae888352be57ada843852a7` and
`5a8b10d0152bd1de45eeacc395cc70c853c22a865a70ebe55c20cf714c29ab37`.
The earlier full CI and projection-comparison receipts remain bound to their
earlier source, not this follow-up. Fresh full verification and review of the
follow-up remain separate requirements.
