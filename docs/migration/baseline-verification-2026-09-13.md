# Consolidated baseline verification — 2026-09-13

Tested source: `dbb76d9d50871a7d460a8cc1548f552582590c8e`.
This records baseline integration verification, not M17 closure, formal safety
proof, HAQP campaign qualification, phase GO, suite ratification or release.
See [integration scope and source ledger](baseline-consolidation-2026-09-13.md).

## Exact-source results

Tests ran in the isolated worktree `/mnt/4tb/liminal-baseline-2026-09-13`.
The recorded pre-run and post-run Git status were empty. The user-owned
recursive directory in the primary checkout was neither changed nor copied
into this test worktree. No assertions, retries or coverage were weakened.

| Command/result | Actual exit and evidence |
|---|---|
| `just ci` | 0; format/TOML/spelling, clippy, 22 bootstrap controls, nextest, threaded tests, doctests, rustdoc, cargo-deny, HAQP inventory and canaries completed |
| Nextest CI profile | 577 passed, 47 skipped; completed in 39.159 seconds |
| Retained crash evidence tests | All 8 registered ILRP boundaries exercised; this is the existing bounded registry, not every production durability boundary |
| HAQP canaries within CI | All 33 caught |
| `just gates` | 0; 577 active, 47 deferred, including 2 Phase 0 M17 and 31 Phase 1 deferred tests |
| `just formal-check` | 0; structural registry validation only, explicitly NOT QUALIFICATION |
| `just haq-inventory` | 0 |
| Ordinary bootstrap Cargo build/run | Both 0; a standalone diagnostic control, not a full pinned Verus/TLC bootstrap replay or production proof |

The meter's +12 active tests over the campaign's recorded 565 are the twelve
new Rust formal-registry tests. The 22 Python controls are not included in that
Rust meter. This explains this integration's delta; it does not reconcile all
earlier M17.8 predictions or close its decision.

Raw evidence directory:
`/mnt/4tb/liminal-formal-evidence/baseline-2026-09-13/`.
The successful managed run is `attempt-2/`; `exits.tsv` records every exit,
including overall 0. The first managed run is retained at the directory root:
CI exited 127 because the service PATH omitted the installed `taplo`. Only the
service environment was corrected for attempt 2; source and checks were unchanged.
The separate primary-checkout formatting refusal and immutable-archive whitespace
diagnostics are described in the integration record; none is reported as a pass.

| Artifact | SHA-256 |
|---|---|
| `attempt-2/ci.log` | `25aa29d72ddcdedebf9ccecd280eeccf34ad62ee8dc810c5e9d108f73f6fc886` |
| `attempt-2/gates.log` | `75c07ee09f7a0a39bb980a4d3d67195f17fbcfc0bc07afde5f00d131080c1e40` |
| `precommit-review.jsonl` | `c922198b635e5dbffe410b1ca02751884e66f6d05d9baca7098988fe1f4dbe5c` |
| `commit-review.jsonl` | `01257cfdb2aa8a55329dd06c14852bd4c01acdc96735997b302529208fff3c4f` |

## External review and verified dispositions

Actual LAMU `review_commit` for `dbb76d9d` returned **PASS WITH NITS**; its
critic also returned **PASS WITH NITS**. The reviewer was MiMo V2.5 Pro, selected
by LAMU's default routing rather than a per-call review model override.
The complete 152,614-byte first-parent integration-code delta was supplied
explicitly because default merge display and multi-megabyte archive restoration
are insufficient review inputs. The unchanged 129 archive files were verified
by exact Git-blob comparison, not claimed as newly reviewed specification text.

Each actionable claim was checked against source before disposition:

- **False positive:** `Callable` is not unused. It types `process_runner` at
  `verification/bootstrap/run.py:335`; removing its import would break loading.
- **False positive:** the bootstrap fixture is not exclusive to `cargo-verus`.
  Ordinary Cargo build is an explicit runner control at `run.py:472–491`.
  An independent ordinary `cargo build --release --locked` and produced-binary
  execution both exited 0 after the baseline CI run. Their output artifacts
  were moved to the external evidence directory; no source was changed.
- **Optional style:** the all-assumptions alias is redundant but correct;
  retain the reviewed imported implementation during consolidation.
- **Declared limit:** the `/proc` process-state witness is Linux-specific;
  it establishes no non-Linux process-death guarantee. Keep the 1.5-second
  control bound: increasing it to 3 seconds could accept a surviving child
  that naturally exits after 2 seconds. No timing failure was observed here.
- **Declared assumption:** same-user hostile source swaps are outside the
  non-hostile-qualifier assumption. No TOCTOU protection claim is added.
- **No defect:** the runner already rejects missing inputs at `run.py:223–227`;
  duplicating that validation in the Just recipe is optional. Unknown phases
  fail closed in the existing gate command.
- **Immutable artifact:** the historical acceptance-request field and its
  explanation remain unchanged; no governance meaning or authority is inferred
  from a reviewer wording suggestion.

The prior AM-17.13 commit `503fe7f0` also received actual `review_commit`:
PASS WITH NITS, critic PASS. Its verified optional wording nits were retained.

## What remains

The tracked baseline is consolidated. All fourteen current-core formal
obligations remain not established. Current-core implementation and adapter
qualification come next, followed by complete M17 evidence/ledger/meter
reconciliation and separate human decisions. Phase 1 remains unauthorized.
No fresh full HAQP lane, blind qualification pass, production proof, signature,
push or release was performed by this baseline verification.
