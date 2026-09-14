# Production proof runner under construction

`check_inputs(repository, verus_root)` in `run.py` checks the closed selected
source and executable hash maps in `inputs.json`. It rejects malformed manifests,
symlink inputs and pin drift. Its only success status is `inputs-valid`, with
`qualification: false`. It does not execute a proof or discharge an obligation.
The same seam also validates `source-inventory.json`: 166 files and their Git
executable semantics from baseline commit
`3bd93a9617ff8705ace37f4a19440c70f867f689`, across only the declared repository
roots. Added, missing or changed files, extra directories, links, special files,
scan errors and root `.cargo` configuration refuse. The embedded baseline commit
identifies the inventory; it does not bind the current runner or execution commit.
The inventory is limited to 2 MiB and 10,000 files; paths to 4,096 UTF-8 bytes and
128 components; each file to 32 MiB; and aggregate source bytes to 128 MiB.
Artifact, held-out and Git-control directory components are never traversed.
The current checkout contains tracked `.cargo/mutants.toml`, which is not Cargo
configuration. `check_inputs` nevertheless targets a reconstructed source stage
and intentionally refuses any root `.cargo`; this is a staging boundary, not an
instruction to delete or change the tracked file. A staged public-seam probe is
recorded separately while orchestration work continues.

`stage_source(repository, commit, destination)` in `stage.py` reconstructs the
closed source projection and its two manifests from exact Git blobs, ignoring
dirty working files. It accepts only a full lowercase commit identity, compares
the entire selected Git file set, modes and SHA-256 values with the committed
inventory, and validates all blobs before creating a new external destination.
Git output is bounded while reading, with a 30-second per-command deadline;
replacement objects, lazy fetch, global/system configuration and prompting are
disabled. Existing destinations and physical aliases into the repository refuse.
Partial output after write failure is retained, never deleted or overwritten.

The constructor and input checker share the same byte-level manifest validators.
Its result separates the requested `commit` from historical `inventory_origin`
and includes both manifest hashes. A different requested commit can reproduce
the same inventory when its exact selected source projection is unchanged.
Commit identity is not approval: the caller must bind authorized commit/profile
hashes and runner identity. `source-staged` always has `qualification: false`.
Ten real scratch-Git controls bring the proof-support suite to 91 controls.
This constructor does not establish toolchain closure, execute a proof, or provide
crash-durable/replay-qualified evidence.

`check_distribution(archive, extracted_root, expected_sha256)` in
`distribution.py` binds the complete extracted Verus file and directory inventory
to a checksum-validated distribution archive. It compares all file contents,
including libraries and packaged caches, and refuses scanning errors. The caller
must supply the approved archive pin; accepting a caller-chosen archive digest
does not establish that archive's authority. Its success status is only
`distribution-valid`, with `qualification: false`.

`prepare_dependencies(lockfile, archives, destination)` in `dependencies.py`
constructs a checksum-closed Cargo source tree at a new destination outside the
repository. It accepts only Cargo.lock version 4 registry packages from the exact
crates.io index source and an explicit, complete `.crate` list; preserves all
archive files including Git-control files; writes Cargo checksum metadata; and
returns only `dependencies-prepared` with `qualification: false`. It performs no
build or network access, never overwrites or deletes a destination, and preserves
partial output after construction I/O failure for diagnosis.
Byte-identical duplicate archive candidates are allowed only when every supplied
copy matches the locked checksum. Archive and file result maps are keyed per
package; they do not assert that only one physical candidate copy existed.

The constructor refuses lockfiles over 4 MiB, archives or individual regular
members over 512 MiB, and aggregate inputs over 1,000 package/candidates, 100,000
members, or 2 GiB of regular bytes. The caller remains responsible for binding
Cargo.lock and archive authority. The returned hash maps are constructor output,
not an independent correctness proof.

`construct_rust_toolchain(channel_manifest, channel_sha256, archives, destination)`
in `toolchain.py` constructs a fresh external Rust 1.97.1 Linux payload from
exactly six explicitly supplied archives. The caller owns approval of the channel
manifest digest; the constructor checks that digest, the selected package hashes,
availability, target and Rust version. It never executes an installer, registers
a rustup toolchain, or copies unrelated ambient components.

The projection preserves every selected component file and directory, including
undeclared empty directories. Undeclared regular payload refuses. Only named
top-level installer metadata is excluded from output; unknown top-level content
refuses. Links, special files, unsafe or duplicate paths, non-directory parents,
and conflicting cross-package entries refuse. Identical overlapping entries merge
only after content/type/mode equality checks. Existing output and aliased parents
refuse; partial output after a write failure remains for diagnosis.

Limits are 2 MiB for channel/component manifests, 512 MiB per compressed archive,
1.5 GiB aggregate compressed input, 256 MiB per regular file, 2 GiB aggregate
uncompressed regular bytes, and 20,000 aggregate members. Remaining aggregate
budgets are enforced at each archive member before its body is read. This is
not a process resource envelope: real construction additionally needs a bounded
job profile that accommodates the observed 199,494,192-byte LLVM files. The
existing proof-command 64 MiB file limit is unchanged and insufficient for that
construction job.

The result is `toolchain-constructed`, always `qualification: false`, with the
channel/archive digests and projected entry map. The caller must independently
compare complete output, bind approved tools/source/profile, and establish actual
compiler selection. Fifteen controls bring proof-support tests to 106.
Real construction completed under the separate bounded profile; independent
archive/output comparison matched all 8,181 rows including the root, and a
namespace-only corrupted-file control was rejected without changing the original.
This construction evidence is not by itself compiler selection or qualification.
A later bounded sandbox probe observed the constructed compiler and pinned
verifier selection, and a two-sided scratch-source control observed strict
argument forwarding in that reconstructed environment. See
`selection-development-2026-09-14.md`. A bounded same-source ordinary/verified
witness pair also observed stable per-build binaries and byte-identical runtime
output. This is development witness binding, not a qualified executable. Cold
independent construction cycles and aggregate qualification remain unfinished. See
`toolchain-development-2026-09-14.md` for the constructor evidence.

Run all public-interface controls with:

```sh
just formal-proof-self-test
```

`run_command(argv, cwd, environment, output)` in `command.py` now supplies the
Linux execution seam. It requires an absolute executable, an allow-listed child
environment, an existing source directory and a new external evidence directory.
It resolves output-parent aliases before enforcing separation from `cwd`, never
overwrites evidence, and runs every command (including witnesses) in an owned
systemd user service. A working user bus is required. The child receives no
inherited environment; the validated bus variables belong only to the client.
Relative `cwd` and `output` Path parameters resolve against the Python caller's
working directory; child-relative argv paths use the service working directory.
the request records the resulting absolute source and evidence locations through
its cwd and retained launcher arguments.
Manager-side argument expansion is explicitly disabled, preserving literal dollar
arguments rather than silently expanding them before the child starts.

Each service requests and checks CPU quota 200%, MemoryHigh 2 GiB, MemoryMax
4 GiB, zero swap, 256 tasks, a 64 MiB per-file limit and a ten-minute runtime
limit. The parent also bounds observation and control-command waits. This is a
resource envelope, not a hostile-code filesystem or network sandbox. The caller
must bind the tool, source, flags, working-directory ancestry and dependency
configuration; this module does not establish that authority or offline behavior.

Requests, launcher/control commands and exits, raw stdout/stderr, observed states,
terminal state and receipt are retained. Nonzero exits and signals are recorded
as observations, not converted to passes. Limit drift, failed launches or missing
cleanup evidence refuse with `execution-incomplete`. Cleanup checks actual unit
absence: a nonzero `reset-failed` is permitted only when independent observation
confirms the unit is absent, as happens after successful garbage collection.
The receipt always has `qualification: false`. Retained files are development
artifacts, not crash-durable or independently replayable proof receipts.

The command seam adds 18 fast public-interface controls to the existing 63
input controls. Process-boundary fixtures are not real execution evidence.
Separate real controls exercised success, nonzero exit, signal termination,
exact environment, raw-byte output and unit cleanup; see the development record.

This fast input-stage recipe is wired into local `just ci` immediately after the
bootstrap self-test. It does not run a verifier, model, network operation or heavy
proof. Broader formal proof/model/adapter execution and remote-CI wiring remain
unqualified and unfinished.

These eight source hashes are not the complete Cargo build graph. These four
executable hashes alone do not bind bundled Verus libraries, Rust, dependencies or
ambient build configuration. The separate distribution check binds the packaged
files but not external Rust or host libraries. None of these input operations may
be promoted to `formal-proof` or treated as full input closure. The full runner still needs
pinned source/distribution/dependency authority, controlled fresh builds, two cold
replays, negative proof/runtime controls, executable bindings and durable raw
evidence. Existing formal qualification commands remain refusal-only.
The source inventory is a closed repository projection, not the whole build
environment. The orchestrator must independently bind the runner, profile and
execution commit before and after use.

The manifest pins the production acknowledgement fragment present at
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`. No pin refresh is automatic.
The fragment checks identity and dependency membership, not the complete
`ilrp-ack` obligation. See `../PLAN.md` and `../safety-ack-slice.md`.
