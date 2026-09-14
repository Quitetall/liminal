# Selected-input preflight and closure probes

Status: development evidence, not qualification. Source baseline:
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`.

The three public proof-input seams have 46 passing temporary-filesystem controls.
The original changed-source control failed against the permissive stub before
implementation, then passed with hash validation. The later deep-JSON trial
failed only because its expected error message was too specific: this Python
parsed 2,000 nested levels and the existing closed schema rejected the value.
That trial does not demonstrate a prior admission defect. `RecursionError` is
also translated to `InputFailure` for interpreters that impose a lower limit;
no custom nesting scanner remains. Tests do not establish resistance to
concurrent malicious filesystem swaps, which the trusted-host model excludes.

Durable logs are under `/mnt/4tb/liminal-formal-evidence/reviews/`:

- `proof-input-red.log`, `proof-input-green.log`: original nine controls.
- `proof-input-deep-red.log`, `proof-input-expanded-green.log`: expanded controls.
- `cold-leaf-source-closure-r2.log`: reconstructed committed source has all 30
  workspace members and the same 197-package metadata inventory. Both locked,
  offline metadata commands exited 0. The first probe omitted `benches` and
  failed; that failure remains preserved separately.
- `verus-clean-distribution.log`: freshly downloaded official archive matched
  SHA-256 `067f5f72a457fe66b77c0c10b180f2a919a9c7481a8baa024ffc716aa931a41b`.
  All 14 root distribution files matched installed copies. Recovery did not
  execute the tools. Four executable hashes alone are insufficient closure:
  bundled libraries, `vstd.vir`, sources and external Rust inputs also matter.
- `cold-leaf-ordinary-r2.log`: ordinary release build from reconstructed source
  exited 0 in 8 seconds, with two Cargo jobs, 440.3 MiB peak memory and no swap.
  The witness exited 0 with exact `acknowledgement-witness:5\n`, empty stderr,
  and unchanged eight selected source hashes. Executable SHA-256:
  `c6f836a4ff04c294aa5d4ae4d320ee0cc3dbc1ab711d30c82bf3d4dc5547b49b`.

The ordinary build used a fresh target directory but existing offline dependency
sources. It is not a fully cold dependency replay. Required next work remains
distribution/dependency closure integration,
controlled proof execution, two cold cycles, negative controls and evidence
binding. The full implementation plan and all sixteen obligation statuses remain
open; this preflight is not wired to a successful formal qualification command.

## Distribution controls and package verification follow-up

The separate distribution check adds eleven passing controls (30 combined).
The first red control demonstrated acceptance of a changed library while the
executable remained unchanged. Follow-up controls demonstrated that `Path.rglob`
could silently skip an unreadable directory containing an extra file under UID
1000, and that an empty ZIP was accepted. Explicit `os.scandir` error handling
and a nonempty regular-file inventory now refuse both. The permission control
is skipped only under UID 0, where that permission experiment does not apply.
Logs: `distribution-red.log`, `distribution-green.log`,
`distribution-inventory-red.log`, `distribution-inventory-green.log`.

Two later, separate reconstructed-source probes completed:

- `cold-leaf-verus-verify-r2.log`: full `cargo-verus verify --locked --offline
  -p liminal-safety`, exit 0, vstd 1862 and leaf 4 verified with zero errors,
  71.206 seconds, 1.6 GiB peak, no swap.
- `cold-leaf-verus-build-r2.log`: full verified release example build, exit 0,
  the same proof counts, and an example harness reporting zero checks (not
  counted as proof). The witness exited 0 with the exact same 26-byte marker
  as the ordinary build and empty stderr. Executable SHA-256:
  `a32ceb13d57360c48daeb1bcb0923d567d5ba7e8a127010a12a7ef0187ef907c`.
  Runtime was 26.372 seconds, peak 1.7 GiB, no swap.

Both used two Cargo jobs, fresh target directories and bounded systemd units.
The eight selected source hashes and fourteen root tool-file hashes remained
unchanged. Neither probe claims complete tool/dependency cold closure.
`cargo-archive-inventory.json` and `.md` independently establish that all 167
registry packages have compressed archives matching Cargo.lock checksums, with
zero missing, conflicting or wrong-checksum candidates. This is not yet an
extracted dependency source or build-environment binding.

The implemented distribution checker was also run against the recovered official
archive and extracted tree: all 2,580 regular files matched, returning only
`distribution-valid` and `qualification: false` in 2.63 seconds. A later malformed
10,000-digit JSON integer control exposed an uncaught parser `ValueError`.
The parser now translates that failure to `InputFailure`; the combined suite
has 31 passing tests. Logs: `proof-input-integer-red.log` and
`proof-input-integer-green.log`.

`cargo-vendor-closure.log` preserves a strict closure failure: Cargo successfully
vendored all 167 packages but omitted 77 Git-control files across 68 packages.
Common files matched byte-for-byte, and vendor checksum maps described the
filtered trees correctly. That does not establish byte-complete archive identity.
The dependency constructor below instead reconstructs every archive file.

## Dependency constructor and exact-vendor probes

The dependency constructor enforces the public contract summarized in `README.md`:
closed Cargo.lock version/source/checksum inputs, an explicit complete archive
list, bounded archive inspection before output creation, raw path and member-type
refusal, Git-control preservation, normalized modes, generated Cargo checksum
metadata, and input rehashing. It creates only a new external destination and
leaves partial output on I/O failure. Its success remains
`dependencies-prepared` with `qualification: false`; the caller must independently
bind lock and archive authority, and the producer's returned maps are not proof.
Positive byte-identical duplicate-copy and both-order bad-copy controls passed
without a production change; they harden coverage and do not show a prior bug.

The first real run failed closed on `convert_case-0.4.0/.gitignore`: its regular
TarInfo carried mode `0100664`, and the initial blanket mode mask incorrectly
classified the valid file-type bits as privileged permissions. The preserved v1
failure was not retried in place. After separating matching file-type bits from
special permission bits, v2 constructed all 167 locked packages from 214 explicit
archive candidates: 7,201 source files and 1,676 directories, producing 7,368
regular files including 167 generated checksum files. A separate verifier
rehashed and streamed the archives, and an independent tree comparator ignored
the producer result map while confirming source bytes, directory inventory,
executable bits and logical checksum maps. The v2 log SHA-256 is
`97c7e8243e76296b2d63a034d0699d26aa9acf071682f0bafbda67c5b61d71af`:
`dependency-constructor-real-v2.log`.

The prior `cargo-vendor-closure.log` omission remains evidence: ordinary Cargo
vendor dropped 77 Git-control files across 68 packages. The later exact-vendor
ordinary build succeeded but inherited the normal child environment after only
explicit compiler-related variables were unset; it is feasibility evidence, not
a strict environment replay. The separate strict allow-list `env -i` verified
run recorded 1,862 vstd and 4 leaf functions verified with zero errors and the
expected witness. These runs establish feasibility only, not qualification.

`source-build-script-closure.log` records an independent source-closure finding:
adding a new `crates/liminal-safety/build.rs` introduced a custom-build target
while all eight selected pins remained unchanged. No build script was executed.
The orchestrator therefore requires a closed source inventory; the selected eight
paths alone cannot bind implicit Cargo build inputs. The log SHA-256 is
`ecb13498685523d9c1955d241d08d88b343331a0fcbbcc528a0cb1ddb6bfd735`.

All sixteen obligations remain open. Archive reconstruction, dependency
construction, ordinary builds, strict verified builds and witnesses are input
closure or feasibility observations only; none is a full qualification result.
