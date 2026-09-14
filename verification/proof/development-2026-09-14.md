# Selected-input preflight and closure probes

Status: development evidence, not qualification. Source baseline:
`3b1ec265b9ffc8bdd3da596011b4ac2593c2369c`.

The three public proof-input seams have 63 passing temporary-filesystem controls.
`just formal-proof-self-test` runs them as a fast input-stage step in local
`just ci`, immediately after `formal-bootstrap-self-test`. It performs no verifier,
model, network or heavy-proof execution. Formal proof/model/adapter execution and
remote-CI wiring remain outside this integration and are not qualified.
The original changed-source control failed against the permissive stub before
implementation, then passed with hash validation. The later deep-JSON trial
failed only because its expected error message was too specific: this Python
parsed 2,000 nested levels and the existing closed schema rejected the value.
That trial does not demonstrate a prior admission defect. `RecursionError` is
also translated to `InputFailure` for interpreters that impose a lower limit;
no custom nesting scanner remains. Tests do not establish resistance to
concurrent malicious filesystem swaps, which the trusted-host model excludes.

`check_inputs` now also enforces `source-inventory.json`, generated mechanically
from 166 Git blobs at baseline
`3bd93a9617ff8705ace37f4a19440c70f867f689` (2,282,505 aggregate bytes; all mode
`100644`). Public controls refuse the previously admitted added `build.rs`, root
`.cargo`, missing or changed additional files, extra empty directories, executable
bit drift, links, malformed or oversized inventories and visible scan failures.
The projection excludes held-out, Git, nested-checkout and artifact traversal.
Its embedded baseline is descriptive rather than self-authorizing: orchestration
must still bind the runner, profile and execution commit before and after use.
Path-length, component-count and forbidden artifact-component controls make the
refusal work bounded before parent construction or traversal. The earlier checker
also refused those scratch manifests later through set mismatch; these controls
are bounded-work hardening, not evidence of a prior false-valid result.

The current checkout itself has tracked `.cargo/mutants.toml`; it is not Cargo
configuration. The public checker deliberately targets a reconstructed source
stage and refuses any root `.cargo`, so the live-checkout refusal is expected and
does not call for deleting or changing that file. `source-closure-live-v1.log`
records the separate staged public call returning only `inputs-valid` with
`qualification: false`; broader orchestration remains in progress.

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
paths alone cannot bind implicit Cargo build inputs. That inventory is now
implemented as a 166-entry input-stage check, but it does not bind the runner,
profile, execution commit or ambient build environment. The log SHA-256 is
`ecb13498685523d9c1955d241d08d88b343331a0fcbbcc528a0cb1ddb6bfd735`.

## Full-proof JSON format feasibility

`full-proof-json-v1.log` records a strict-environment, exact-vendor full-crate
format probe: exit 0 after 1m34.443s, 1.1 GiB peak memory and no swap. Verus emitted
two whitespace-separated JSON values—not JSONL and not one JSON object—with
`is-verifying-entire-crate: true`, zero errors, and verified counts 1,862 and 4.
`schema-report.json` parsed both values. Neither record contains an explicit crate
identity, source coordinate or cryptographic source binding; attribution to vstd
and `liminal_safety` is inferred only from function namespaces and ordering.
`func-details` cardinality differs from the verified count, so it cannot be treated
as a direct per-function expansion of that count. This is format feasibility, not
a formal receipt or qualification.

For commit `570fcfc6`, `dependency-constructor-570fcfc6-disposition.md` records
LAMU **PASS WITH NITS** and critic **PASS** after each finding was checked; the raw
receipt is `dependency-constructor-570fcfc6-review.jsonl`. Source-map verification
is recorded in `source-closure-independent-map.md`; the dependency source Spec and
Standards PASS reports are `dependency-constructor-spec-review.md` and
`dependency-constructor-standards-review.md`; and the source-closure Standards
PASS is `source-closure-standards-review.md`. These reviews do not establish runner
correctness, proof qualification, or phase authority.

## Bounded command execution development

The new `command.py` executes real commands in owned systemd user services and
retains exact requests, client/child environments, control commands and exits,
raw streams, observed limits, terminal state and cleanup evidence. Its 18 public
controls bring the fast proof suite to 81 tests. This is not full proof-runner
orchestration or qualification; the caller must bind sources, tools, profiles,
expected outcomes and replay authority.

The first seven preflight controls found three errors: missing executable and
existing output leaked raw OS errors, and output-parent symlink aliases were
not resolved before the source-directory check. `test-command-candidate-red.log`
preserves the 4-pass/3-error result (SHA-256
`7bb577e3c2b612d9a3d4dd70338dc5cd157b7ab820ac4ebb365e1df8d2a31e39`).
Typed refusals and canonical parent containment closed those gaps without
overwriting sentinel files. Later controls also reject malformed Unicode argv
and retain observed-limit drift, transport failure and cleanup failures.

`command-controls-v1.log` (SHA-256
`bbe7d0eb44382e5c766a96aef812bf9f4b6f9b0d530a46294bc87102bb8a7f47`)
records four real controls at module SHA-256
`250b6c1de2f8ea841b62c4e8abef612c21804de1d09b76bf12ddc9d6dd182f18`:
false exited 1; signal termination recorded signal 15 rather than an exit code;
`env` printed exactly the two supplied variables; raw-byte output retained `ff`
and `657272` with child exit 7 and matching hashes. No unit remained active.
These controls precede the subsequent preflight/cleanup changes and do not
qualify the final module revision.

The later `command-cleanup-v2` probe confirms why a nonzero `reset-failed` is not
itself cleanup failure: the service was already unloaded. The runner now retains
the reset exit and separately observes `LoadState=not-found`, `ActiveState=inactive`
and `SubState=dead`. Unit tests exercise both that valid outcome and failure to
establish absence. Earlier review advice to reject every nonzero reset was checked
against the real service and rejected; the actual observation gap was closed.

An initial LAMU `review_diff` response incorrectly treated this Python file as
outside an unrelated `lamu-rs` workspace. Its nominal PASS was not accepted as
review evidence. The raw response remains `command-precommit-review.jsonl`.

An independent literal-argument control then found a real wrapper defect:
systemd's default environment expansion removed an unset dollar argument before
the child ran. Its checker constructed the expected value from `chr(36)` rather
than embedding the same expandable literal, avoiding a self-consistent false
pass. The percent-literal control passed. Both before/after files bind this RED
probe to module SHA-256
`2cba780bf3b23e02fe096eb486efacdfd72969eb3fb442691361643b91f0006b`;
an initially reported older hash was corrected against those actual artifacts.
The probe is `probes/command-literals-v1` under the external evidence root.
`command-literal-argv-red.log` preserves the matching unit regression failure
(SHA-256 `5458a5a17b912630be322f5a4c337dc79731675398bd73c1a708a05d831530e8`).
The launcher now explicitly disables manager-side expansion with
`--expand-environment=no`, and the unit regression passes.
The separate `command-literals-v2` GREEN probe used the same independent checkers:
both commands exited 0 and cleanup independently established absence. Its
summary SHA-256 is
`d8c00bbeab9655f208e67f0b8bb8a153d88485b61dec7c9cfe5307d54e1b4df7`;
before/after module SHA-256 is
`7f60adc5a10ec5b7992e5f2a71b70737aac5d0b668837cbb9356f2e0faa373ef`.
This observes literal argument preservation for those two controls, not every
argument/path shape or complete runner correctness.

### Follow-up correction to the command test evidence

Commit `e894fea8` was committed before the final combined test exit was inspected.
Its claim of passing 18 new plus 63 existing controls was incorrect for that
revision: `command-suite-81.log` records 80 passed and one failed, exit 1
(SHA-256 `0f0bea49826c86a263de9820a30ec0fccf06336a3289d6a0bd7c9a3e8fa2e313`).
The loaded-unit cleanup assertion omitted the newly retained `state_exit` field.
This was a real stale expectation, not an intermittent failure. The follow-up
adds the exact `state_exit: 0` expectation while retaining all other comparisons;
no production behavior changes. `command-suite-81-corrected.log` records all 81
passing, exit 0 (SHA-256
`15477f31e317ea67cf4ddaa3220b94033dcad13b8cc0d44e9496acc0824a0fb1`).
The original failed log and commit remain intact.

Actual LAMU `review_commit` of `e894fea8` returned PASS WITH NITS from MiMo V2.5
Pro, with a critic pass, retained as `command-e894fea8-review.jsonl`. Both called
out the stale expectation. The review's statement that the commit message
acknowledged the failure is false; the review request acknowledged it, not the
commit. Path-name traversal advice was checked against the actual `Path.name`
and canonical-parent operations: a basename cannot contain separators, and
the physical containment check precedes directory creation. The `..` refusal is
not redundant because only the parent is resolved. Existing caller-controlled
`TMPDIR`, bounded observation slack and exclusive evidence files remain
intentional, documented development boundaries, not new qualification claims.

All sixteen obligations remain open. Archive reconstruction, dependency
construction, ordinary builds, strict verified builds and witnesses are input
closure or feasibility observations only; none is a full qualification result.
