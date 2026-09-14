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

Run its public-interface controls with:

```sh
python3 -m unittest discover -s verification/proof -p 'test_*.py' -v
```

These eight source hashes are not the complete Cargo build graph. These four
executable hashes alone do not bind bundled Verus libraries, Rust, dependencies or
ambient build configuration. The separate distribution check binds the packaged
files but not external Rust or host libraries. Neither check may be promoted to
`formal-proof` or treated as full input closure. The full runner still needs
pinned distribution and dependency closure, controlled fresh builds, two cold
replays, negative proof/runtime controls, executable bindings and durable raw
evidence. Existing formal qualification commands remain refusal-only.

The manifest pins the production acknowledgement fragment present at
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`. No pin refresh is automatic.
The fragment checks identity and dependency membership, not the complete
`ilrp-ack` obligation. See `../PLAN.md` and `../safety-ack-slice.md`.
