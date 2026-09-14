# Production proof runner under construction

`check_inputs(repository, verus_root)` in `run.py` checks the closed selected
source and executable hash maps in `inputs.json`. It rejects malformed manifests,
symlink inputs and pin drift. Its only success status is `inputs-valid`, with
`qualification: false`. It does not execute a proof or discharge an obligation.

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
python3 -m unittest discover -s verification/proof -p 'test_*.py' -v
```

These eight source hashes are not the complete Cargo build graph. These four
executable hashes alone do not bind bundled Verus libraries, Rust, dependencies or
ambient build configuration. The separate distribution check binds the packaged
files but not external Rust or host libraries. None of these input operations may
be promoted to `formal-proof` or treated as full input closure. The full runner still needs
pinned source/distribution/dependency authority, controlled fresh builds, two cold
replays, negative proof/runtime controls, executable bindings and durable raw
evidence. Existing formal qualification commands remain refusal-only.

The manifest pins the production acknowledgement fragment present at
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`. No pin refresh is automatic.
The fragment checks identity and dependency membership, not the complete
`ilrp-ack` obligation. See `../PLAN.md` and `../safety-ack-slice.md`.
