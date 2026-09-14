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

Run its public-interface controls with:

```sh
just formal-proof-self-test
```

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
