# Checked-repair precommit reviews

Baseline `f11e130c19f8717811e9c9396a6bc7f993ce7af5`; staged candidate, not a
post-commit or qualification review. Fresh LAMU stdio `review_diff`, `fast`
preset (single reviewer, not ensemble), automatically selected MiMo V2.5 Pro.
All three processes exited 0. Their complete JSON artifacts include the exact
reviewed diff, its SHA-256, source-file hashes, baseline, and model response.

Artifacts are durably retained under
`/mnt/4tb/liminal-formal-evidence/reviews/`.

| Group | Diff SHA-256 | Verdict | Artifact SHA-256 |
|---|---|---|---|
| graph | `94c1253b5d62bd6141e21be07c5d831c8c6536552d27b3cc986f3db1242bfa8f` | PASS WITH NITS | `3ed2564d7f1b4094792af81c07929e7b7f66bd9a13877ba467d905cd5c6713dc` |
| jurisdiction | `1ebd033bdb1c71b98d7d8cfb5187e7a90f60800b6e5b9cbaa8cb0c025083acec` | PASS WITH NITS | `e0cc2914fbb7d87aac7e85c109e4d968f2674b43d68f2b957f8d521dff89f923` |
| daemon | `05da376b668c4e33d8eb166b4029307565c0519877a3d894b2cb2b82b563aa4f` | PASS | `c1d6abd2dc56e74420c4afd81bc66937e339308d526a6bcf56f1e37eed609014` |

Artifact names are `checked-repair-<group>-review-2026-09-14.json`.
The client script is `checked-repair-review-client.mjs` in the same directory.

## Verified dispositions

### Graph

1. **Do not compare the entire aux map with durable replay.** Verified at
   `GraphStore::put_working_aux` and `committed_aux_history`: working buffers
   deliberately have no log record. The proposed whole-map comparison rejects
   valid saves. The method instead proves the requested key plus durable graph
   state; it does not claim to audit unrelated auxiliary keys. Positive and
   tamper tests exercise this distinction. No fix applied.
2. Owner `Deref` exposes read methods, while the owner already intentionally
   holds root write authority. Ordinary store handles still cannot construct
   writers. Future public API growth requires review; this is not a current
   bypass. No extra forwarding layer added.
3. Linear transaction lookup and genesis replay are explicit toy costs. No
   evidence establishes a release budget breach here. No unmeasured caching or
   indexing change was introduced; profile the full run before optimizing.
4. Macro/manual writer style and ASCII-byte-length comments are nonfunctional
   nits. The predicate rejects every non-ASCII byte. No changes required.

### Jurisdiction

1. **Keep pre-run and post-run history checks.** The first refuses forged
   progress before effects; the second creates a receipt for the newly durable
   finalization. Removing the latter is not a safe deduplication. An optimized
   incremental verifier would need an explicit invalidation/binding contract.
2. Observation serialization matches: `reactor.rs:61–62` hashes
   `serde_json::to_vec(&payload)`, the same encoding used by admission. The
   quoted-string concern does not reproduce for the actual producer.
3. `PanicAt` is an in-process fixture under the pinned ordinary test profile,
   which unwinds. It is explicitly not crash qualification. Real daemon crash
   tests still use SIGABRT. A hypothetical panic-abort test configuration is
   outside the observed command, not a reason to remove the fixture.
4. Reviewer language that validation is itself atomic is too broad. Validation
   reads may interleave; the non-retargetable expected-head guard is compared
   under the same mutex as validation/append. That guard, not reviewer prose,
   establishes refusal of a stale publication attempt.

### Daemon

No actionable defect found. The reviewer correctly checked same-store and exact
plan/evidence binding before accepted-repair bookkeeping. Its claim that all
scope restrictions are compile-time enforced is too broad: private construction
is a type boundary; allowed operations/namespaces are runtime checks that poison
the transaction on refusal. Neither this review nor single-threaded scenarios
prove arbitrary concurrency safety.

These dispositions do not replace the mandatory review of each eventual commit,
independent accumulated-delta review, full CI, or campaign evidence.
