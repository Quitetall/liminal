# Successor candidate verification record

This record describes local verification of the **unregistered, unselected, and
unaccepted** `0.1.0-proposed.2` candidate. It is not acceptance, ratification,
authorization, a GO decision, or a qualification claim for the target runtime.

## Scope and provenance

- Accepted authority commit: `9e76fc99027c55ded5bd0cc61da43b0f2b68b049`
  (tree `2c631b0282e5573502126ef0c7c4a228dab01064`). The clean accepted archive at
  `/home/brianklam/Desktop/liminal-sas-migration` remained at that commit.
- Target source-history commit: `7e4ba39d6babb1e91a78aecd75bfa0262095c2c9`
  (tree `181d78a6295f04e73deeee91916d75ef19b70772`). It is local-only side history,
  is not an ancestor of the accepted branch, and must be published and fetched
  before a fresh-remote or hosted check can run.
- `scripts/sas_successor.py check` verified the accepted SAS SHA-256
  `53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`,
  all 777 unique accepted requirement rows, the accepted authority/configuration
  files, 65 historical source files, and exactly 10 modified plus 2 added source
  records. The 12 records remain pending successor incorporation disposition.
- The active workflow and default `just ci` lane remain unchanged. The complete
  local candidate lane is `just sas-successor-ci`.

## Commands and actual exits

Large debug and fuzz build caches used the owned
`/mnt/4tb/tmp/liminal-sas-successor-target` area. Small Cargo metadata and doc
output remained in the worktree's ordinary `target/` directory. Durable logs
and one-line exit status files are under the external target's
`sas-successor-logs/` directory.

| Command | Exit | Result |
| --- | ---: | --- |
| `python3 scripts/sas_successor.py generate` | 0 | generated exact candidate and reconciliation |
| `python3 scripts/sas_successor.py check` | 0 | successor integrity PASS |
| `python3 -m unittest scripts.test_sas_successor` | 0 | 19 tests passed |
| `python3 -m unittest discover -s scripts -p 'test_sas_migration.py'` | 0 | 40 tests passed |
| `env -u CARGO_TARGET_DIR just sas-successor-ci` | 0 | 503 active tests passed, 47 skipped; docs, dependency policy, inventory, and 33 canaries passed; successor checks passed |
| `CARGO_TARGET_DIR=/mnt/4tb/tmp/liminal-sas-successor-target just gates` | 0 | declared test-surface meter ran; 503 active and 47 deferred |

The successful full-lane log is `sas-successor-ci-final.log` with
`sas-successor-ci-final.exit`; the gate log is `gates.log` with `gates.exit`.

## Failed diagnostic attempts retained

Two full-lane attempts with an inherited `CARGO_TARGET_DIR` exited 100 after
502 of 503 active tests passed. The sole failure was
`haq::tests::sanitizer_replay_reproduces_the_canonical_build_and_refuses_the_rest`:
the nested fuzz build returned success, but its binary was redirected away from
the verifier's canonical `fuzz/target`, so the verifier correctly found no
`cst_parse` binary. Logs are `sas-successor-ci.log` and
`sas-successor-ci-rerun.log`, each with a matching `.exit` file.

The exact named test reproduced exit 100 with inherited `CARGO_TARGET_DIR`
(`sanitizer-single.log`) and passed with exit 0 in 15.207 seconds when that
variable was unset (`sanitizer-single-unset-target.log`). The canonical whole
`/var/tmp/liminal-haqp-build/9e76fc99027c55ded5bd0cc61da43b0f2b68b049/fuzz/target`
directory and the worktree `target/debug` directory were symlinked to the owned
4 TB target area for the large caches. No HAQP code, accepted source, evidence
claim, or prebuilt binary was altered to obtain the pass.

These results qualify only the local mechanics of this candidate bundle against
the accepted baseline. They do not select the candidate, dispose its T1 scope
impacts, qualify commit `7e4ba39d`, or establish hosted CI status.

After the full lane, a disposable-fixture probe demonstrated that overwriting a
hardlinked generated output could mutate its other link. The planted public-CLI
test first failed, then passed after generation was changed to refuse outputs
with multiple hardlinks. The real accepted tree was never used for this probe.
