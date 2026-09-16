# Ordering leaf candidate source rebind

Candidate implementation commit: `ba9d9ae9c88ceae74307f440657d1ecaf7b81ab9`.
This is source-input maintenance, not qualification or a refreshed historical
receipt. Main checkout is not advanced by this isolated change.

The bounded source projection differs from inventory origin
`9c2279f5485e3533a67c0718c5df016dd935a8f6` in exactly two paths:

- `crates/liminal-safety/src/lib.rs`: ordering predicate and proof additions;
  new SHA-256 `9e0a62e2fd96ece112c19b9ed732a01f78e78d156735ff39e14ef18860dd9fad`.
- `crates/liminal-safety/tests/ordering.rs`: new runtime tests;
  SHA-256 `5f871e84aa38fb99625277933130e0add1eac3318630ee3d8b2f56e24e61689c`.

The complete candidate inventory now has 169 entries. Independent enumeration
using `git ls-tree` over the existing explicit source roots and `git cat-file`
verified every path, mode and SHA-256 against that commit, exit 0. No held-out
path was admitted or read. The parser origin and two fixture origins move to
the same commit. The selected-input library hash also moves; the acknowledgement
fragment label, non-discharge flag, other selected inputs and all tool pins stay
unchanged. This does not extend the acknowledgement fragment's declared scope.

Proof-support discovery: 142 tests passed, exit 0, 6.382 seconds. Receipts:
`/mnt/4tb/liminal-formal-evidence/reviews/proof-unittest-20260916T061524/`.
This tests support code, not the ordering theorem or whole-core obligations.

Remaining: exact-source staged replay, source-rebind commit review,
host integration and qualified evidence aggregation. Old cold replay
and HAQP outputs remain bound to their original source; no result is relabeled.

## Leaf commit review

Actual LAMU `review_commit` for `ba9d9ae9` completed with primary **PASS WITH
NITS** and critic **PASS**. Receipt:
`/mnt/4tb/liminal-formal-evidence/reviews/order-leaf-ba9d9ae9-review.stdout`.
Findings were inspected at current code coordinates. The definition-equivalence
lemma at `lib.rs:36` is intentionally definitional, not claimed as a substantive
ordering obligation. Renaming the Boolean graph predicate is optional style;
specification and executable helper remain separately proved representations.
The readiness bounds are retained because its standalone specification has no
prefix precondition. The suggestion to make the checker crate-private is not
applied: the user explicitly approved the public direct test seam; it grants
no authority. No verified correctness defect or required code change resulted.

## Candidate full CI

`just ci` completed with exit 0 under `liminal-order-full-ci-r1` on the
candidate source-rebind tree based on the implementation commit above. It ran
630 Rust tests successfully, with 47 skipped, plus 22 bootstrap controls and
142 proof-support tests. All 33 canaries were caught. The full command also
completed formatting, lint, doctest, documentation and dependency-policy gates.
This is development CI evidence, not a new HAQP qualification lane or proof
that the sixteen whole-core obligations are established.

Durable stdout and stderr are under
`/mnt/4tb/liminal-formal-evidence/reviews/order-full-ci-r1/`.
Systemd reported `Result=success`, `ExecMainStatus=0`, runtime 3 minutes
3.614 seconds, peak memory 4 GiB and swap 0 bytes. The unit was bounded to
two CPU equivalents, 4 GiB memory and no swap. The generated canary receipt
left no tracked diff; historical outputs were not hand-edited or relabeled.
Production source and candidate input metadata remained unchanged during CI.
