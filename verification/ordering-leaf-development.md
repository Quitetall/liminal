# Ordering leaf development

Baseline: `2fe72b7923f3096cb7cddd77b2759191b9d26909`.
The user confirmed the direct `liminal_safety::ordering_matches` test seam.
This uncommitted slice is not integrated into production admission or ILRP.
Runs r35-r36 establish a warm-cache development proof of the leaf contract;
they do not establish any registered whole-core obligation or qualification.

## Observed red/green sequence

Artifacts: `/mnt/4tb/liminal-formal-evidence/reviews/`.
Each bounded run used the isolated checkout, two CPU quota units, 4 GiB memory,
zero swap, 256 tasks, a 600-second deadline, and two Cargo build jobs.

1. `order-leaf-first-red.*`: exit 101, unresolved public import before any
   implementation. Initial literal test requires dependency 2 -> 1 to accept
   [2, 1] and reject [1, 2]. `order-leaf-first-green.*`: exit 0, one passed.
2. `order-leaf-tie-red.*`: exit 101, one passed and one failed. Independent
   steps incorrectly accepted reverse numeric order. Smallest-ready selection
   added; `order-leaf-tie-green.*`: exit 0, two passed.
3. `order-leaf-partition-red.*`: exit 101, two passed and one failed. Correct
   external-first applied partition was rejected. Stable two-group comparison
   added; `order-leaf-partition-green.*`: exit 0, three passed.
4. `order-leaf-graph-edge-red.*`: exit 101, three passed and one failed.
   Graph-before-external dependency was incorrectly accepted. Explicit edge
   classification refusal added; `order-leaf-graph-edge-green.*`: exit 0,
   four passed.

No existing assertion was changed, disabled, or relaxed. No production caller
was routed through the intermediate incomplete versions. Integer indices and
borrowed slices require no allocation; identity values remain all 128 bits.

Additional boundary coverage (`order-leaf-malformed.*`) initially reported six
passing tests and one failure. Source inspection found a new fixture error:
its positive external-first ordering marked the external subgroup as graph
steps. Correcting those fixture flags preserved the expected-order assertions;
the reciprocal graph-subgroup fixture was added. Duplicate-key coverage was
also isolated from the declared-identity check by using matching repeated keys
and declared IDs. `order-leaf-malformed-corrected.*` records exit 0, seven
passing tests. The fixture failure is not an implementation RED or a waived
failure. Coverage includes empty and full-width identities, malformed map and
schedule membership, unknown endpoints, self-edges, cycles, duplicate valid
edges, and stable order within both applied subgroups.

The full declarative `ordering_valid` contract now appears in the executable
function's equivalence postcondition. Merely stating it does not prove it.
At this initial stage the loop invariants did not establish full equivalence.
The chronological proof iterations below retain those failures and conclude
with the first full development proof pass.

Initial direct verifier launch (`order-leaf-verifier-r1.stderr.raw`) refused
with exit 101 before crate verification: the isolated rustup home lacked the
required named `1.97.1-x86_64-unknown-linux-gnu` toolchain entry. This is an
environment refusal, not proof RED. Existing pinned compiler bytes are reused
read-only through a named entry in the successor scratch root; no toolchain
download or historical-root update is authorized by that correction. The
successor job's outcome is recorded below.

Successor `order-leaf-verifier-r2.*` reached the current crate and exited 101:
Verus could not infer a trigger for the schedule-to-step membership quantifier.
This is a proof-encoding refusal, before a logical verification result. Source
SHA-256 at that run was
`b4a96908b415a1ff02b0ed83d5991d53e3809380ef4b58cc5e61d3bd892425a8`.
Runtime 2min42.726s, peak memory 1.3G, zero swap. An explicit outer
`schedule[i]` trigger was then added without changing executable behavior.

An independent external runtime oracle (`order-exhaustive3.stdout/.stderr`)
completed before that trigger edit with exit 0: 147,456 cases, exactly 122
accepted. It enumerates all 512 directed edge sets on three vertices including
self-edges, all eight graph classifications, and all 36 original/applied
permutation pairs. Its oracle selects the lexicographically minimum valid
permutation, rather than reproducing the checker readiness loop. Identities
are 0, 1, and `u128::MAX`; generated steps have matching unique declared IDs.
The accepted count is independently checked as 25+36+36+25 colored DAGs.
This finite family excludes malformed identities and arbitrary lengths; the
literal controls cover selected malformed cases. It is not an unbounded proof,
TLC model result, HAQP receipt, or full qualification.

## Proof iteration follow-up

Development verifier runs r3 through r6 are retained under the same external
reviews directory as `order-leaf-verifier-rN.*`. Run r3 failed before checking
the crate because its forwarded-argument flag was misspelled. The fixed launcher
uses `--fwd-verus-args-to roots`; r4 reached the proof and reported 14 verified,
3 errors (five printed postcondition diagnostics), exit 101.

Run r5 added the incoming-dependency prefix witness for the smallest-ready
rejection. It exposed a missing outer-loop invariant, `selected < candidate`.
Run r6 carries that invariant through the candidate loop. The smallest-ready
rejection diagnostic is absent; the whole-crate result remains unsuccessful:
15 verified, 2 errors, exit 101, with four printed postcondition diagnostics
covering the initial length rejection, two partition rejection paths, and final
acceptance. These counts are verifier-reported units, not established project
obligations. Source hashes before and after each launch are retained by the
launcher. Run r6 used 259.1M peak memory, zero swap, and 1.855 seconds elapsed.

These edits add proof invariants only; they do not change executable acceptance
rules or weaken `ordering_valid`. Runs use a warm development cache and do not
constitute qualification. The next proof work must establish the partition
length/prefix relationships and carry all validated facts to final acceptance.

Run r7 adds a recursive ghost lemma proving that the two complementary filters
have combined length equal to the input sequence length. The proof unfolds the
pinned library's filter definition and inducts on `drop_last`; it introduces no
assumption, external body, runtime allocation, or product size bound. Calling
that lemma at entry discharges the initial length-rejection diagnostic. Whole
crate verification still exits 101: 16 verified, 2 errors, with three printed
postcondition diagnostics (the two partition rejection exits and final
acceptance). Counts do not imply three independent remaining obligations or
exhaustive diagnostics. Raw outputs and before/after hashes are retained as
`order-leaf-verifier-r7.*`. This remains development evidence only.

Runs r8-r10 develop the partition prefix proof. Run r8 failed type checking
(a `nat` sequence index needed an `int` cast, and the pinned filter-push lemma
takes element before predicate). Run r9 corrects those errors and verifies
`filter_scan_step`: advancing a selected prefix appends exactly that element,
whose index in the full filtered sequence equals the prior filtered length.
Whole-crate output reports 17 verified, 2 errors, exit 101.

Run r10 uses that lemma in the runtime partition scan and adds cursor-count
invariants. It remains failing: the two partition rejection exits, both outer
group-transition invariants, and final acceptance have printed diagnostics.
The predicate used by the scan must still be related explicitly to the two
canonical complementary predicates; accepted-prefix equality and previously
validated ordering facts also remain to be carried. No runtime branches or
acceptance conditions changed. All three runs are retained, including failures,
under `order-leaf-verifier-r8.*` through `order-leaf-verifier-r10.*`.

Run r11 proves extensional equality between the scan predicate and each canonical
group predicate at the completed scan. Both outer group-transition diagnostics
disappear. Run r12 uses the same equality at each selected element and proves
its exact index and value in the full stable partition. Both partition-rejection
diagnostics disappear. The whole-crate result still fails: 18 verified, 1 error,
exit 101, with the final acceptance postcondition as the remaining printed
diagnostic. This is not a completed ordering proof: accepted membership,
dependency, readiness and matched-prefix facts still need to survive all loops
and jointly imply the full contract. All changes in these two runs are ghost
proofs; executable acceptance behavior is unchanged. Raw evidence remains under
`order-leaf-verifier-r11.*` and `order-leaf-verifier-r12.*`.

Runs r13-r14 establish positive facts at the first two loop exits. The membership
loop now proves matching declared identities, pairwise distinct map keys, and
schedule occurrence of every map key. The dependency loop now proves strict
schedule precedence for every edge and exclusion of graph-to-external edges.
Explicit assertions at both exits verify. Whole-crate verification still fails
at final acceptance (r14: 18 verified, 1 error, exit 101). These local facts must
still be retained through subsequent loops; equal-size membership also needs
its permutation argument, and readiness and matched-output prefix proofs remain.
The added annotations have no executable effect. Evidence is retained under
`order-leaf-verifier-r13.*` and `order-leaf-verifier-r14.*`.

Runs r15-r17 develop `equal_size_membership`. Run r15 lacked the set-equality
argument; r16 proved set equality by strict-subset cardinality contradiction but
still lacked explicit sequence-membership instantiation. Run r17 supplies that
instantiation and verifies the lemma: equal lengths, distinct source keys and
membership of every source key imply distinct schedule entries and reverse
membership. The argument uses pinned vstd cardinality lemmas, no size bound or
assumption. It is not yet connected to the step-key projection at the caller.
Whole-crate r17 still exits 101 with 19 verified and 1 error at final acceptance.
Failures and successful local proof progress are retained in
`order-leaf-verifier-r15.*` through `order-leaf-verifier-r17.*`.

Runs r18-r20 connect positive proof components. Run r18 applies the permutation
lemma to the actual step-key projection and verifies schedule uniqueness and
reverse membership. Run r19 verifies the smallest-ready condition at its loop
exit, including the blocked-candidate witness. Run r20 retains matched applied
elements through the partition scan and verifies the final extensional equality
between applied order and the canonical stable partition. Whole-crate r20 still
fails final acceptance: 19 verified, 1 error, exit 101. It also reports an index
recommendation on the matched-prefix invariant; its partition-length bound must
be made explicit. Earlier facts must be checked at the final point rather than
assuming their local assertions imply the whole postcondition. No executable
behavior changed; all runs are preserved as `order-leaf-verifier-r18.*` through
`order-leaf-verifier-r20.*`.

Runs r21-r24 probe every contract clause at the final return. Run r21 exposed
reverse membership there; r22 reconstructs its key projection and witnesses
at that point. Run r23 verifies the explicit cursor-completion assertion but
fails the combined `ordering_valid` assertion, despite the individual clause
assertions. Run r24 adds the partition-length invariant to make prefix indexing
bounds explicit but its fuel-qualified reveal is rejected before proof checking:
that form requires a recursive specification with a decreases clause. Run r25
uses ordinary `reveal` instead and reaches proof checking; the combined assertion
still fails. Both runs exit 101 and are preserved.
The combined specification remains unproved and unchanged. These are retained
diagnostic iterations, not a passing proof or justification to weaken the
contract. Evidence: `order-leaf-verifier-r21.*` through `order-leaf-verifier-r24.*`.

Runs r26-r28 isolate the combined-contract failure. Explicit filter-predicate
extensional equality (r26) and additional specification reveals (r27) do not
resolve it. Run r28 introduces a ghost contract-assembly lemma with the original
clauses as explicit premises; that lemma verifies. Its caller fails precisely
the reverse-membership quantified premise (every schedule ID names a step).
Thus the earlier claim that local return checks suffice was too strong: the
quantified witness must be exported in the form required by the contract.
Whole-crate r28: 20 verified, 1 error, exit 101. No runtime or contract change.
Raw evidence: `order-leaf-verifier-r26.*` through `order-leaf-verifier-r28.*`.

Runs r29-r32 isolate the witness export in `step_reverse_membership`, whose
postcondition is exactly the original reverse-membership clause. Its caller's
proof obligations verify conditional on that contract, but the helper's own
quantified postcondition fails; therefore the crate remains unproved. Parentheses
and an explicit guarded `==>` formulation do not resolve it. These unsuccessful
probes are retained, not counted as completed proof. No assumptions, external
bodies or relaxed requirements were introduced. The next action is to resolve
this isolated quantified implication, not integrate an unproved helper.

## First whole-crate development proof pass

Runs r33-r34 retain unsuccessful explicit-counterexample probes (r33 lacked a
choice trigger; r34 still failed witness export). Run r35 extracts the unchanged
existential `exists j: 0 <= j < steps.len() && identity == steps[j].0` into the
open specification `names_step`, used by the same universal reverse-membership
clause. This definitional factoring resolves quantified composition. The full
crate reports success: 22 verified, 0 errors, exit 0. No runtime branch changed,
and no size bound, assumption or external body was added.

Run r36 adds an explicit definition-equivalence lemma and verifies it together
with the entire crate: 23 verified, 0 errors, exit 0. The definition lemma states
exact equivalence to the original existential, not a weakened membership check.
Raw outputs, exact flags and source hashes remain under
`order-leaf-verifier-r35.*` and `order-leaf-verifier-r36.*`.

These are warm-cache development runs, not cold qualification, host integration,
an established project obligation, or Phase 0/1 authorization. Runtime controls
must be rerun against final source; false-proof controls, independent review,
proof cleanup and fresh source bindings remain required.

## Remaining work

Fresh LAMU diff review completed in `reviews/order-leaf-review-r1.result.json`:
primary MiMo V2.5 Pro verdict **PASS WITH NITS**, stable before/after source
hashes. No concrete correctness defect was reported. Root disposition: keep
the supplied `applied` sequence because checking actual caller order is the
purpose of that independent input; do not replace it with self-derived evidence.
Keep `position < schedule.len()` because `ready_at` is a standalone specification
with no prefix-bound precondition. Dense predicate/style comments are nonblocking;
public Boolean tests intentionally do not specify internal rejection reasons.
The returned critic text ends mid-section and no complete second-vendor result
is present, so this is not claimed as a complete ensemble or phase review.
Per-commit review remains mandatory once a commit exists.

Post-refactor proof sensitivity was rerun at `probes/order-controls-root-v2/`
with new writable cache and the same minimal manifest and dependency lock from
v1. Copied leaf bytes matched the checkout. Positive: exit 0, 25 verified / 0
errors, 54.953 seconds, 1G peak, no swap. Negative: only public checker final
return at `src/lib.rs:490` changed from `true` to `false`; exit 101 at the
unchanged ordering postcondition. Restored exact original: exit 0, 25 verified /
0 errors. Positive and restored input hash manifests match; each run's before
and after hashes match. This replaces no historical receipt and establishes
only bounded development proof sensitivity, not formal qualification.

Post-refactor runtime confirmation: `reviews/order-final-20260916T060734/`
records 7 literal tests passed, exit 0, unchanged leaf hashes. Its new oracle
workspace refused a missing lock under `--locked`, and its lint commands again
selected the wrong toolchain; those refusals remain preserved.

Root independently ran `/usr/bin/cargo clippy --locked --offline` with the
pinned vendor config, `-p liminal-safety --all-targets -- -D warnings`, using
clean `/usr/bin` PATH and explicit system Rust/Cargo. Unit
`liminal-order-lint-root-r1` exited 0 (356 ms, 70.4M peak, no swap). Root then
ran the existing dependency-checked oracle workspace with `cargo run --locked
--offline` and the same vendor routing: unit `liminal-order-oracle-root-r1`
exited 0, 147,456 cases / 122 accepted (506 ms, 71.9M peak, no swap).
These commands rebuilt the refactored leaf and are development evidence only.

Attached LAMU `review_diff` returned `Transport closed`. Preflight passed and
a fresh stdio review was launched using
`reviews/order-leaf-review-r1.mjs`; no verdict is claimed until its result is
received, source hashes checked, and findings verified at their coordinates.

Corrected lint commands at `reviews/order-lints-corrected-20260916T060348/`
established fmt success and a real Clippy failure: `ordering_matches` exceeded
the 100-line limit (265 lines). No lint exemption was added. Root removed
redundant diagnostic assertions, strengthened the existing membership helper
to export schedule uniqueness, and extracted smallest-ready and stable-partition
runtime stages into private helpers with exact Boolean-equivalence contracts.
Runs r37, r38 and r39 each verify the entire crate with exit 0; r39 reports
25 verified, 0 errors. The public ordering contract is unchanged. The partition
helper repeats the same length guard for its standalone proof contract; accepted
and rejected public inputs remain governed by the full equivalence theorem.
Fresh lint/runtime verification against these refactored source bytes is pending.

Isolated proof sensitivity control completed under
`probes/order-controls-root-v1/`. Only the leaf source was copied, with a minimal
standalone manifest preserving edition, package identity, exact featureless vstd
pin and Verus metadata. The initial `positive` launch refused lock pruning under
`--locked` and remains retained. Offline lock generation selected 15 registry
packages; every version/checksum matched the original workspace lock, preserved
as `original-workspace.lock`.

- `positive2`: fresh writable proof cache, exit 0, 23 verified / 0 errors.
- `negative`: exact one-line change at copied `src/lib.rs:502`, replacing final
  `applied_index == applied.len()` with `false`; exit 101, 22 verified / 1 error,
  explicitly at the unchanged ordering postcondition.
- `restored`: reverse that exact patch, byte-compare source against the checkout,
  exit 0, 23 verified / 0 errors. Its input hash manifest equals `positive2`.

Each command records source, manifest, lock and verifier/solver hashes before
and after, with equality checked. Systemd bounds: 2 CPU quota, 4 GiB, no swap,
256 tasks, 600 seconds. The source manifest is deliberately standalone rather
than a complete reconstructed workspace; tools reuse the pinned installed
distribution. This proves bounded false-proof sensitivity, not independent
cold qualification, host integration, mutation coverage or any phase approval.

Fresh standalone oracle at
`reviews/order-oracle-scratch-20260916T060133/` completed with exit 0:
147,456 cases, 122 accepted. Its 15 registry dependency versions/checksums
match the authoritative workspace lock, and leaf hashes match before/after.
This newly bound development receipt does not overwrite the historical oracle.

System-tool lint attempt `reviews/order-lints-system-20260916T060108/`:
Clippy invocation refused argument forwarding before linting. Contrary to the
worker's summary, fmt stdout contains an actual formatting diff at
`crates/liminal-safety/tests/ordering.rs:139`; fmt ran and exited 1. Root inspected
the output and authorized formatting only that test file. No lint pass is
claimed from either attempt.

Corrected vendored runtime attempt
`reviews/order-runtime-config-20260916T055923/` passed all seven literal ordering
tests, exit 0, with source hashes unchanged. The exhaustive oracle still refused
before execution: its historical standalone lock pins proc-macro2 1.0.107 while
the workspace vendor closure contains 1.0.106. The historical lock is preserved;
a separately bound oracle run must verify its newly resolved dependencies against
the authoritative workspace lock before its output is used. This does not
invalidate or refresh the historical oracle receipt.

The second false-proof harness draft also failed read-only review: copied root
manifest references absent workspace members, named rustup entry remains absent,
and regex replacement plus a nonunique `false` check does not establish the
intended mutation. Neither draft was executed. Root took over harness design;
successful model prose is not accepted as source or mutation evidence.

Post-proof runtime launch attempt `reviews/order-runtime-20260916T055814/`
refused offline dependency resolution before tests: workspace `blake3` and
standalone `vstd` were unavailable because the launcher omitted the existing
vendor routing. Source hashes matched before/after. Lint attempt
`reviews/order-lints-20260916T055836/` selected the custom proof toolchain,
which has neither cargo-clippy nor cargo-fmt. Those are launch refusals, not
test or lint outcomes. Corrected commands must preserve these attempts and use
the pinned vendor closure and explicitly available lint tools.

The first external false-proof harness draft was rejected by source inspection
before execution: copy and working paths disagreed, workspace inheritance and
the named toolchain were missing, and a last-file-line substitution would not
mutate the ordering return. Its hash comparison also included unequal path
strings. No result from that draft is admissible. Corrected controls require
an exact one-site mutation and independently checked source binding.

Runtime and one false-proof control now cover the current source as recorded
above. Independent review, proof cleanup, host translation/integration,
and exact new source bindings remain pending; any subsequent source edits
require fresh verification. The development proof is of the
leaf equivalence contract, not the complete ILRP ordering obligation.
Previous full CI and cold replay receipts do not cover these new source bytes.
Do not commit or integrate this function as a proven capability on this evidence.
