# Sandbox request constructor development record
The public seam is
`prepare_sandbox(source: Path, rust: Path, verus: Path, vendor: Path,
destination: Path, operation: str) -> dict`. It prepares, but never executes,
exactly `verify`, `ordinary-build`, or `verified-build`. Success is
`sandbox-prepared` with `qualification: false`.
## Contract and boundary
The fixed bubblewrap request uses unshared namespaces, disables nested user
namespaces, clears the environment, mounts `/usr` and all four inputs read-only,
adds the named Rust-toolchain overlay, and provides fresh writable target,
Cargo-home, rustup-home, temporary and log trees. Strace, guest paths and offline
Cargo configuration are fixed. Verify fixes the selected package; verified-build
fixes the selected example. Both request JSON output and `--no-cheating`.
Ordinary-build is the fixed acknowledgement-witness Cargo command and carries no
Verus JSON/strict flags. There is no runtime execute operation in this slice.
The destination and six child directories are mode 0700 under an existing safe
parent; `command-evidence` remains absent for `run_command`. Checks precede
creation, existing output is untouched, and construction I/O failure retains
partial output. Inputs must be absolute, existing, non-symlink directories and
pairwise disjoint. Input/destination overlap, retained `..`, NUL/non-UTF-8,
forbidden names, filesystem-root or `/usr` aliases, and unsafe parents refuse.
Forbidden components are rejected before that path is inspected. Other checks
inspect ancestor metadata but never enumerate input-tree contents. `Path` has
already normalized `.` and repeated separators, so the seam validates the
resulting value and any retained `..`.

This constructor does not hash or authorize inputs, execute bubblewrap/Verus,
establish actual tool selection, interpret output, or qualify a proof or phase.
No real command has been launched through it.
## Evidence
Seven public tests bring proof-support discovery to 128. Logs are under
`/mnt/4tb/liminal-formal-evidence/reviews/sandbox-development/`:
| Slice | RED SHA-256 | GREEN SHA-256 |
|---|---|---|
| verify | `c6420f54dd87f455c54022dc49977a89240de3258b9b7ca4c8dfd26517349da1` | `4254e4a045e347998dc610c8d77c612c733cc051c5f5e03d960445e8ef65dd6a` |
| overlap | `393b0986fca7812f92083a89496dca108a0090bdf5363627c84a6ad256dfb7f1` | `f7184413aaf2e9c6c307dc739d2949a58b20c0b30ec7a583745a01f8f3f8342a` |
| build operations | `7ec08fa2ffca9198c9289bb13fc9b5a2e5072e63e9d9d29557d2dc6e787bd717` | `3c344a6f24994c8dc3b90e6f3f7d871d20c134bfe8ca9407597a17e2e9e4ba3b` |
| runtime aliases | `6f7b3f0c9307d35e73b9197299f177a17282aad1e18a20e24b4802d837a31b32` | `fc5af5240463da6b4ef931f15f0a3d0a4328898e853b39c41b0c607a4722f235` |

The first alias RED (`runtime-alias-red.log`, SHA-256
`423db8e1c6e15efda83a6097eb2403fa0eb235730d7f2ba88cf771cc86328d55`)
reused a destination after unexpected acceptance and contaminated later
subcases. It remains preserved; the independent RED above shows `//usr` and
`//usr/bin` accepted while `//` already refused. Resolved-path checking closed
the gap. Other invalid controls were green on arrival; an added unhashable
operation exposed a raw `TypeError`, normalized to `SandboxFailure`.

The final support run passed 128 tests in 5.582 seconds, exit 0; log SHA-256 is
`6689e7b1439936e8f0cce1448ce8092139de9b3771a83d9ae0f992242c962dc2`.
Frozen implementation/test SHA-256 values are
`fb66d9ce30d29abdd7542ffdfcee806b6dfa44b2bc25fac400b79405af1677aa`
and `3a261b041365fd82e9d5216b51d8a89136e6dbd29aeab4aef43ae0b5cd85e988`.

An independent comparator exited 0: verify 116/116, ordinary build 114/114,
verified build 119/119. Result/script SHA-256 values are
`3139b907947c04bc2309633bcdab14e75c052b1c6ff0a7825fdacd5a45d953d3`
and `22c9a39bf780b2164fe1dfc4916b0c648fb215a02f6ccecfb388ef9344008002`.
Argv equality is not execution, selection, proof success, full CI, or qualification.
Review follow-up added one green-on-arrival public test for final-component
symlinks, covering both an existing-directory target and a dangling target while
preserving the links and target state. Destination-specific assertions were also
tightened without weakening prior controls. The implementation hash remained
`fb66d9ce30d29abdd7542ffdfcee806b6dfa44b2bc25fac400b79405af1677aa`;
the current test hash is
`f9d28b87bf8b65f3c2d3d7cfd2b6566854cdc1c3dda9aa26bc1a8b768a2b7833`.
The 129-test support run passed in 5.578 seconds, exit 0; log SHA-256 is
`81367ee57ca2007c6c8f070d7c41183682b50b04635b50097f037aeaea146200`.
Trusted-host operation excludes same-user hostile filesystem races; no stronger
race-hardening claim is made.
