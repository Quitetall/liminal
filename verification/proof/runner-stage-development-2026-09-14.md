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
