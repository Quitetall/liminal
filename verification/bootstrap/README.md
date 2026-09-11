# Formal-tool bootstrap fixtures

These portable fixtures record a bounded tool bootstrap: one toy arithmetic
function and one four-Boolean model. They are not a production binding, a real
ILRP proof, adapter qualification, full Package B closure, human adoption, or an
independent-source reviewer result.

`verification/tool-pins.json` records the exact versions and hashes. The
pin-enforcing runner is the canonical bootstrap entry point; it writes raw logs
and a machine-readable result only to a new external directory:

```sh
just formal-bootstrap "$VERUS_ROOT" "$TLC_JAR" "$ABSENT_EXTERNAL_OUTPUT"
```

The output directory must not exist and must resolve outside both this source
repository and the pinned Verus root. The runner validates every tool and source
input before execution, uses isolated target/work directories, requires the
positive proof/model evidence and precise negative-control failures, then
rehashes all inputs. Its only outcome is `bootstrap-only`.

## Cargo and Verus

The accepted bootstrap shape requires both Verus commands to exit `0` and report `2045
verified, 0 errors` for `vstd` plus `1 verified, 0 errors` for the application.
Ordinary Cargo build and both binaries must exit `0`, followed by a cold repeat
in new target directories.

The direct negative control must exit `1` with `0 verified, 1 errors` and `postcondition not
satisfied`. A generic nonzero exit is not equivalent to this control result.

## TLC

The positive model must exit `0` after complete exploration: 10 generated states,
5 distinct states, 0 queued states, depth 5. The false model differs only by
removing the `intent` guard from `Apply`; it must exit `12` with the specific
`EffectImpliesIntent` invariant violation after 3 generated and 3 distinct
states. Explicit stuttering is modeled, so no liveness claim is made.

The former manual command sequence is diagnostic-only: individual invocations
cannot establish an accepted bootstrap result because they do not enforce all
pins, repetitions, semantic controls, and end-of-run rehashes together.

## Bootstrap incident

The first Verus download was interrupted and an overlapping resume produced a
malformed local archive with 147,456 extra bytes. It was never extracted or
executed. After all writers ended, a single clean download of the same pinned
asset matched the original SHA-256 and passed ZIP integrity and path-safety
checks. No pin, tool version, secret, global configuration, or runtime default
was changed to repair the cache.

Until the repository authority and qualification work explicitly changes their
status, `formal-proof`, `formal-model`, `formal-adapters`, and `formal-gate`
remain refusal paths. These fixtures do not satisfy or bypass them.
