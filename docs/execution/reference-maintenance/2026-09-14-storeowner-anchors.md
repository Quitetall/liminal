# StoreOwner and checked-repair reference maintenance

Authority: AM-17.13 for exact reference maintenance; AM-17.12 for the pending
authorized production migration in this same change; DG17.3 for the generated
ILRP probe's trusted setup migration. No containing commit self-hash is embedded.
This record does not authorize a new mutant, change a threshold, establish a
kill, or qualify the hardened candidate.

Old source commit: `f11e130c19f8717811e9c9396a6bc7f993ce7af5`.
Exact old/new files, line coordinates, enclosing symbols, source expressions,
and file SHA-256 values are in the independent
[source audit](2026-09-14-storeowner-review.md), SHA-256
`a854e6d8b00aed210f824403e2e13b43febb2815bf07ca499288641f063d8979`.
Reviewer `/root/reference_anchor_audit` used GPT-5.6 Sol without filesystem
writes. Primary checked the reported file hashes and each old/new source line
before changing references. Repeated expressions were resolved by symbol and
arm, not by nearest text.

## Unchanged mutant metadata

P1-M017–M026 below retain family `graph/interchange codecs`; P1-M045–M051
retain family `repair/ILRP/recovery`. All remain `predeclared`. The exact defect
strings and all other packet fields remain byte-for-byte unchanged.

| Mutant | Operator | Requirement | Killing-test mapping |
|---|---|---|---|
| P1-M017 | threshold-minus-one | P1-R009 | P1-T14, P1-T15 |
| P1-M018 | missing-enum-dispatch | P1-R010 | P1-T14, P1-T15 |
| P1-M019 | success-error-substitution | P1-R011 | P1-T13, P1-T14 |
| P1-M021 | wrong-holder-selection | P1-R012 | P1-T15, P1-T16 |
| P1-M022 | skipped-durable-transition | P1-R015 | P1-T22, P1-T07 |
| P1-M023 | threshold-plus-one | P1-R015 | P1-T18, P1-T19 |
| P1-M024 | threshold-minus-one | P1-R009 | P1-T14, P1-T15 |
| P1-M025 | missing-enum-dispatch | P1-R008 | P1-T14, P1-T15 |
| P1-M026 | success-error-substitution | P1-R011 | P1-T13, P1-T14 |
| P1-M045 | success-error-substitution | P1-R016 | P1-T17, P1-T18 |
| P1-M048 | skipped-durable-transition | P1-R015 | P1-T22, P1-T07 |
| P1-M049 | disabled-crash-point | P1-R015 | P1-T22, P1-T07 |
| P1-M050 | ordering-nondeterminism | P1-R015 | P1-T20, P1-T21 |
| P1-M051 | oracle-short-circuit | P1-R015 | P1-T07, P1-T17 |

The same operators remain directed at the same expressions. Only fourteen
registry line numbers and matching packet `source` fields changed. No mutant
was moved to a different expression or given a different transformation.
Admission and recovery strengthening are separately authorized production
changes, not disguised reference edits.

Runtime behavior/kill preservation remains UNKNOWN for the deferred patches.
In particular, retain the audit's P1-M050/P1-M051 redundancy risk, P1-M048
changed downstream detection risk, and pre-existing P1-M044 label mismatch.
The reference update does not upgrade those rows from `not-ready`, certify
equivalence, or replace their mapped tests. M24 must resolve concrete patches
and execute them before qualification.

## Packet and development verification

- Old packet digest: `ad9441f5a5dc096155710853ce9b142128f622ad1ab295563f333df461c4527a`.
- New packet digest: `0e439fe364210f0dd22a1121b9f4d7bc717a7a93933202679197e728397242b9`.
- Existing producer: `cargo run -p liminal-xtask -- haq packet-digest`, exit 0.
- `docs/execution/phase1-suite-review.md` mirrors that produced digest.
- `cargo run -p liminal-xtask -- haq verify-inventory`, exit 0.
- `cargo test -p liminal-xtask generated_case_builders_match_their_recorded_goldens`,
  exit 0; one actual golden test passed, 232 unit tests filtered. The other test
  binaries ran zero matching tests and are not additional evidence. No golden
  constant was edited. This exercises the migrated ILRP probe in the existing
  generated-family witness and confirms its unchanged output contribution.

Durable logs under `/mnt/4tb/liminal-formal-evidence/reviews/`:

| Log | SHA-256 |
|---|---|
| checked-repair-packet-digest-2026-09-14.log | `fc1bbf1d5e819735ed99988c53476bd5ee57b8185aab410814b09645c35f226a` |
| checked-repair-inventory-2026-09-14.log | `7a2639b7239b285e0836748def7d66b658c399e9ff895fc95dab95dd94db8e6c` |
| checked-repair-generated-goldens-2026-09-14.log | `56e6b587d15643ffbc67e87657ba6837b3db15de12372f1e7c61bbd5575bcdd6` |

Full fixed-candidate CI, campaign provenance refresh, and complete post-commit
review are still required. Old evidence remains historical; no old provenance
is rewritten to claim it exercised these new source bytes.
