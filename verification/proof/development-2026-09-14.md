# Selected-input preflight and closure probes

Status: development evidence, not qualification. Source baseline:
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`.

The public `check_inputs` seam has 19 passing temporary-filesystem controls.
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
The next closure step reconstructs all archive files directly rather than adding
an exclusion to the comparison.
