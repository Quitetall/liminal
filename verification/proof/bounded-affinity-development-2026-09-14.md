# Bounded initial-affinity request: development boundary

This additive slice implements `prepare_bounded_sandbox` without changing
`prepare_sandbox`, `run_command`, verifier flags, or proof scope. It does not
establish any formal obligation or adopt a qualification profile.

## Motivation and contract

A fresh staged-runner probe at `2be4c2c53ebf9dc3c729f83ae1705bd64b638a77`
timed out at its unchanged 600-second limit. A separate fresh-output control
using the same source and flags with initial CPU affinity 0-2 completed in
94.292683 seconds, with peak memory 1,867,177,984 bytes. This supports a
resource-overparallelism explanation, not a unique causal or reproducibility
claim. The two runs reused Rust/vendor inputs; neither was cold independent
construction. The preserved investigation is
`/mnt/4tb/liminal-formal-evidence/reviews/fresh-runner-execution-continuation.md`.

The new constructor observes the caller's Linux affinity, requires a nonempty
native set of nonnegative exact integer CPU IDs, and chooses its lowest at most
three entries. Missing or malformed affinity refuses before output creation.
One or two available CPUs remain valid; no unavailable CPU is invented.
It prefixes the entire existing argv with `/usr/bin/taskset --cpu-list <CPUs>`.
The distinct request schema records the selected IDs, original argv and
`linux-initial-affinity-at-most-three-v1` profile. Initial affinity is not a
hostile-child restriction or total thread-count cap. The existing command
service retains independent CPU, memory, task, time and file-size limits.

## Development evidence before commit

Five new public-seam controls supplement the eight unchanged sandbox tests.
The OS-error test failed with an unwrapped OSError before the error translation.
The malformed-observation test failed in all six cases (four assertion failures,
two TypeErrors) before validation, then passed. Small-affinity and unsupported
API controls passed on arrival and are not presented as regression red/green.
Full proof-support discovery passed 142 tests in 5.934 seconds; diff checks passed.

Independent comparison against hash-bound prior verify/build probes matched the
complete three commands: 119 verify, 117 ordinary-build and 122 verified-build
arguments. This comparison did not execute a verifier. Evidence under
`/mnt/4tb/liminal-formal-evidence/reviews/`:

| Artifact | SHA-256 |
| --- | --- |
| `bounded-affinity-validation-red-green.json` | `d4e798cfd9adcc2c477da451f1ee64c8f03fe0f118e470ce987824b11bd89765` |
| `bounded-affinity-comparison-v2.json` | `29f9b50617e5bbcc09eef9d9e12b86a497be64fac56a5953638ded3ec43cb903` |
| `bounded-affinity-review-result.json` | `ddec87847e3ab0419db3c316c516eeaaf8eeb24455353ff8fd0e6f00a4a7f5b6` |

LAMU diff review returned PASS and NO_CRITIC_FINDINGS. Its boolean-type question
was checked at the actual validation expression: exact `type(cpu) is int`
intentionally rejects booleans. Its phrase "CPU 0 as the target" is imprecise:
the argument zero identifies the current process, not CPU zero. No fix follows
from that wording. Hardcoded tool paths and the new schema are explicit Linux
development contracts, not compatibility claims for other hosts or old callers.

## Remaining work

Post-commit external review, exact-commit CI, independently compared fresh runner
staging, and actual positive/false-proof execution of this constructor remain
required. Earlier prefix experiments do not substitute for its execution.
Full core proofs, adapters, models, controlled replay, evidence aggregation,
HAQP and human adoption/phase decisions remain open under `../PLAN.md`.
