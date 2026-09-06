# SAS migration verification

This is the verification snapshot at migration commit `d984f66`. A subsequent
[human acceptance](sas-acceptance-receipt.md) accepts the exact SAS revision;
OpenWarrant registration remains pending. The original observations below are
retained as recorded and make no claim about that later tool import.

Status: proposed migration, locally implemented and reviewed; adoption remains
pending. This record does not establish HAQP qualification, human acceptance,
Warrant resolution, Phase GO, suite ratification, or a compiler release.

## Candidate and retained authority

- Liminal branch: `migration/sas-roadmap`, based on
  `0d8c32a4ae1afda9808e55c3c291445aad4e3d60`. This record travels with the
  migration commit; no push or merge is claimed.
- OpenWarrant branch: `migration/program-sas-phases`, commit
  `03f7d05b2ebe11811559b064d3b212c4a08c9364`, based on
  `8bc3978a26de0c13605c6d206fe820a8440be737`. No push or merge is claimed.
- SAS revision: `0.1.0-proposed.1`, SHA-256
  `53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`.
  The [acceptance request](sas-acceptance-request.toml) is tool-emitted and
  remains unanswered. The human authority register is absent.
- Original Liminal campaign checkout was not edited. Migration work and build
  artifacts use separate worktrees and caches. The frozen committed packet
  remains proposed, unratified, `not-run`, stage `1a`.

## Source coverage and proposed contracts

The [crosswalk](source-crosswalk.json) records 65 historical sources, 933
structural blocks and 732,090 exact source bytes. The SAS contains 759 source
requirements plus 18 consolidation requirements: 777 unique stable IDs.
All 55 baseline HAQP requirement IDs are mapped. All fourteen phases, -1
through 12, remain declared.

Classification totals: 146 context, 3 example, 25 historical status, 673 mixed
clause, 49 normative clause and 37 open gap blocks. Applicable clauses inside
mixed blocks remain conjunctive. Coverage is lossless; byte coverage alone is
not proof of semantic agreement or implementation.

Six absent baseline test implementations remain explicit evidence gaps:
P1-T12, P1-T20 and P1-T35 through P1-T38. Historical checkboxes and obsolete
absolute test-count forecasts remain historical. The scoped reconciliation
table preserves HAQP thresholds and separates M17.6 GO from M24 ratification.

Twelve draft Warrants cover M17, M18–M24 and integration/profile/parity/release.
Each contribution is partial and obligations remain unestablished. Tier and
executor kind are distinct: human decisions remain human; agent review and
reporting work does not acquire human authority merely because it is T1.
Protocol, IR, profile and observable design precede their implementation or use.

## Liminal checks

The full `just ci` run exited **0**:

- Formatting, TOML formatting, spelling and clippy passed.
- Migration check and its then-current 37 planted-defect/history tests passed.
- Nextest: 503 passed, 47 skipped. Threaded test pass: 503 passed, 47 ignored.
- Workspace doc-tests and rustdoc with warnings denied passed.
- Cargo-deny advisories, bans, licenses and sources passed.
- HAQP inventory passed; all 33 canaries were caught.

`just gates` and standalone `just haq-inventory` also exited **0**. The meter
reports 503 active and 47 deferred tests, including 31 Phase 1 tests; it is an
inventory, not qualification evidence. After the final executor metadata change,
the migration check and all **40** planted-defect/history tests passed. Their
log is bound by the retained evidence manifest.

The isolated build cache required a worktree-only `target/debug` symlink
because the existing conformance harness locates toy binaries there. Initial
formatting, fixture-in-progress and binary-location failures were retained
separately; the final full CI log is the successful run.

Fresh OpenWarrant `war check --generated` on Liminal exited **0**:
91 PASS, 37 WARN, 0 UNKNOWN, 0 ERROR. The warnings describe draft authority and
missing controlled assurance/independence evidence. Generated views show all
fourteen LIM phases with exact SAS titles, draft authority, twelve draft
Warrants and zero satisfied requirements. Existing draft action hints are
structural authoring hints and provide no execution authorization.

## OpenWarrant checks and adoption blocker

All 656 workspace tests and clippy passed. Tests cover signed references,
canonical serialization, all fourteen LIM phases, the declared eleven OW
phases, malformed/duplicate/undeclared/wrong-program references, missing or
ambiguous SAS candidates, digest drift and draft/unavailable status. The
selected SAS revision binds the phase source and requirement snapshot.

Baseline OpenWarrant `war check --generated` exited **0**, with no errors.
On the candidate, ten **new** `deliverable.digest-drift` errors affect prior
authorized records in OW-WAR-0005, 0013, 0055, 0057, 0058, 0062 and 0063.
Those records were preserved. The complete old/new digest list is retained in
OpenWarrant's `docs/warrants/OW-WAR-0065/evidence/adoption-blockers.json`.
The pending OW-WAR-0064 correction act and human disposition must address
these impacts before adoption; refreshing controlled records was not inferred
from authorization to implement the compatibility change.

The final clean committed `cargo xtask gate` exited **1**, with 7 of 9 steps
passing. Corpus validation and planted violations remain non-green. Generated
views pass drift checks. Plants: 139 passed, 6 failed; one optional slow fixture
was skipped. All negative migration cases passed. The six failures are positive
fixtures that expect exit 0 and inherit the controlled digest refusals: clean
corpus, independence undeclared, independence sufficient, prose lineage fields,
local choice without ADR, and unstated contribution. Post-plant worktree was
clean at the same commit. This is a governance adoption blocker, not a claim
that aggregate verification passed.

Existing legitimate OW status was compared against the original binary on the
same baseline corpus: 22 satisfied and 2 superseded requirements, phases 1 and 6
achieved, and all eleven objective achievement fields unchanged.

## Review and limits

Independent migration review used separate read-only agents:

- Standards axis: no remaining findings after committed-history, merge-topology
  and index-validation fixes.
- Spec axis: no remaining findings after design-before-implementation sequencing
  was made explicit in the integration Warrants.

These agents inherited migration context. Their reviews are not HAQP blind
passes, formal Warrant verification, or evidence of the nine independence
dimensions declared false in `openwarrant.toml`.

OpenWarrant commit `03f7d05` received LAMU `review_commit`: **PASS WITH NITS**.
The disconnected connector returned `Transport closed`; a fresh stdio transport
called the same tool with its automatic reviewer chain. The reported double
file-read issue was checked and rejected: `sas_snapshot` reads once and reuses
those bytes. Repeated SAS parsing across Warrants is a real efficiency nit,
deferred without changing correctness or the adoption boundary. Final Liminal
commit review is retained separately to avoid a self-referential review commit.

## Evidence and next decision

Raw logs remain under `/mnt/4tb/tmp/liminal-sas-verification/`; the
[evidence manifest](verification-evidence.json) binds the completed checks by
SHA-256. Earlier failed runs remain in that directory and are not substituted
for the successful full CI run. OpenWarrant's Warrant also retains baseline
validation, the earlier gate run and exact adoption impacts in its commit.

Review the [SAS](../sas/LIMINAL_Software_Architecture_Specification.md),
[roadmap](../roadmap/PRODUCTION_ROADMAP.md), [crosswalk](source-crosswalk.json)
and [decision proposal](adoption-decision.md). Acceptance must name the exact
revision and digest above. Establishing the human authority register and
recording acceptance remain human-controlled. Preserve campaign results before
integration, reconcile changed historical sources, and requalify the final
baseline wherever source eligibility changed. M17.6 GO and final suite
ratification remain separate decisions after SAS acceptance.
