# Baseline HAQP campaign result — 2026-09-14

**NOT QUALIFIED.** `just haq-lane run-1` completed its stages but the final
packet flip refused five unresolved blind-review findings. Actual command exit:
2. No metadata child commit was made; campaign checkout HEAD remains
`29f9f159acbd1746e83be0f9516888d877daeaa4`.

The campaign receipt records source tree
`904f288a211fd3ace02cd24dddf0eeb72b2efb6f`, clean start, 7384 seconds and
`result=fail`. The clock file remains the last completed campaign, not a new
successful receipt. This baseline does not qualify later proof-core changes.

## Observed stages

- CI: 605 passed, 47 skipped; actual stage exit 0.
- Threaded/full workspace replay and all 33 canaries: stage exits 0.
- Generated evidence: 100,000 accepted cases per registered family.
- Crash evidence: 8 registered boundaries, 8 exercised.
- Mutations: 65 `not-ready` rows; zero mutation kills claimed. This is the
  explicitly staged HAQP-1a boundary, not HAQP-1b qualification.
- Seven AddressSanitizer fuzz targets: each ran at least 1800 seconds; all
  exits 0, zero reported crash artifacts, 231,886,752 total executions.
- Blind reviews: first reported five unresolved findings; second reported none.
  The frozen final flip still counted five after existing signed rulings.

Source verification maps A01 to RISK-006 and A02/A03 to RISK-005. A04's claimed
ruling clearance does not satisfy the recorded attack-class match; A05 attacks
hostile-qualifier receipt forgery outside the adopted non-hostile-runner scope.
The external disposition records this reasoning without changing either blind
answer, signing a ruling, or treating a reviewer disagreement as a pass. The
existing gate requires an authoritative disposition before a successful flip.

## Durable artifacts

All paths below are under `/mnt/4tb/liminal-formal-evidence/reviews/`:

| Artifact | SHA-256 |
| --- | --- |
| `haqp-29f9f159-run-1-r2.log` | `ab0970fd6bdb47244b34754a2445cbd8e27be26002f1e315d8c299f216ecd7fe` |
| `haqp-29f9f159-r2-evidence.tar.gz` | `dfb80f2c7f1a42c0bd36f334633027ab4e160d8e80bb04eccc27d5121bbe9e39` |
| `haqp-29f9f159-r2-runtime-logs.tar.gz` | `7197cd6d8124314e484304edac4b0b7c5faaaaf6b7c46e01bb72fad5370ea4d0` |

The archives preserve emitted evidence, raw runtime logs and JUnit; original
campaign files remain in the isolated checkout. The failed initial directory
setup attempt is preserved separately. No held-out corpus was manually opened
or tuned against, and no residual risk was silently removed.
