# Batch authentication (partial S5)

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md).
This is a trusted-host check, not hostile-same-user isolation or patch admission.
No real policy, batch, key or trust configuration is enrolled by this change.

## Interface and limits

```text
liminal-xtask assurance amend check --authorization-only \
  --trust-root ABSOLUTE_EXTERNAL_DIRECTORY \
  --policy POLICY_JSON --policy-signature POLICY_JSON.sig \
  --batch BATCH_JSON --batch-signature BATCH_JSON.sig \
  --base FULL_COMMIT_ID --target EXACT_MUTANT_ID \
  [--candidate CANDIDATE_COMMIT --source RELATIVE.rs --line OLD_LINE --anchor EXACT_LINE \
   --review-receipt ABSOLUTE_EXTERNAL_REVIEW_JSON]

liminal-xtask assurance amend apply --authorization-only \
  [same authorization and candidate/review arguments] \
  --output ABSOLUTE_NEW_EXTERNAL_WORKTREE
```

Exit zero authenticates only the selected batch scope. Output explicitly says
`batch_authenticated: true`, `apply_authorized: false`, and
`independent_review: not-established`. The JSON is not signed, is not a reusable
capability, and must never be accepted as proof by a future apply implementation.
Without `--authorization-only`, this partial implementation refuses full admission.

When all four optional candidate flags are present, the check additionally emits
`registry_binding: verified`. It binds the exact target to the compiled 65-entry
closed mutant registry, verifies the base packet coordinate, runs the read-only
coordinate proposal against base/candidate commits, and permits only two modified
files: the Rust source (blank-line relocation with unchanged enclosing code) and
`conformance/haqp/packet.json` (one exact source-coordinate line replacement).
Supplying only part of this optional group refuses. Omitting the group reports
`registry_binding: not-requested` and remains authentication-only. The compiled
registry snapshot is part of the pinned executable; changing it requires a new
tool enrollment, not a mutable runtime registry edit.

`--review-receipt` is accepted only with a complete candidate group. Its regular
external file must lie below the canonical trust root and contain strict
`schema_version: 1` JSON with `reviewer`, `backend`, exact `base_commit`,
`candidate_commit`, `target`, lowercase `patch_sha256`, `verdict`,
`unresolved_verified_findings`, and `findings`. The check recomputes the exact
raw Git patch over the requested source and packet paths, compares its SHA-256,
requires `verdict: "pass"` and zero unresolved verified findings, and requires
each finding to have a unique identifier, known classification and resolved
status. A `verified_defect` also needs `independently_reproduced: true`. Success
reports `independent_review: receipt-verified` and the receipt digest; omission
reports `not-established`. This is a read-only binding check, not provider
authentication or apply authority. Future apply must obtain review directly from
its trusted adapter and recheck every binding immediately before effects; copied
or candidate-provided review metadata remains insufficient.

The optional registry check establishes only coordinate and packet-diff binding;
the optional receipt check establishes only exact-patch review-record binding. It
does not establish mutation behavior or provider authenticity. Remaining checks
include exact mutation patch binding and unchanged target/mutation behavior.
Authenticated scope or a historical receipt does not establish either. The
explicit `amend apply` command is isolated and refuses in-place destinations;
admission and revocation are rechecked before effects. No automatic in-place
apply exists.

## Exact-byte formats

All three JSON documents use `schema_version: 1`. Unknown, missing and duplicate
fields are refused. Digests are lowercase SHA-256 hexadecimal strings (64 digits),
commit/revision pins are full lowercase SHA-1 Git IDs (40 digits). Identifiers are
nonempty ASCII alphanumeric strings with hyphens or underscores; wildcards are
not supported. JSON whitespace remains part of the signed and digest-bound bytes.
Inputs must be regular files, not symlinks, and each is limited to 1 MiB.

The external directory contains `trust.json` and `allowed_signers`. Its canonical
path must lie outside the Git common directory and every registered worktree,
including the primary checkout. Unresolvable worktree scope fails closed.
`repository_git_dir` is the canonical absolute Git common-directory path, allowing
legitimate linked worktrees without confusing them with separate repositories.

| Document | Required fields besides schema version |
|---|---|
| External `trust.json` | `enabled` (boolean), `principal`, `repository_git_dir`, `policy_sha256`, `batch_sha256`, `tool_revision`, `tool_sha256`, `allowed_signers_sha256` |
| Signed policy | `repository_git_dir`, `change_class`, `tool_revision`, `tool_sha256` |
| Signed batch | `id`, `policy_sha256`, `base_commit`, `change_class`, `tool_revision`, `tool_sha256`, `targets` (nonempty array of unique exact identifiers) |
| External review receipt | `reviewer`, `backend`, `base_commit`, `candidate_commit`, `target`, `patch_sha256`, `verdict`, `unresolved_verified_findings`, `findings` |

Both signed documents must say `change_class: "coordinate-only"`. Their tool pins
must match external trust. The batch binds the active policy digest, and its base
must match the requested base and name an actual commit in the selected repository.
The requested target must appear exactly in the signed batch's targets.

## Signatures, activation and tool identity

OpenSSH verifies the policy under `liminal.assurance.policy.v1`, and the separate
batch under `liminal.assurance.batch.v1`. Both use the externally selected principal
and digest-pinned `allowed_signers` bytes. Verification snapshots public signature
and signer inputs in external scratch; exact payload bytes are supplied on stdin.
Missing OpenSSH or any failed verification refuses the check. The implementation
does not read private keys or invoke signing. Tests generate and sign only with
their own disposable fixture keys.

Humans maintain external trust, sign policy/batch bytes, and select active digests.
`enabled: false` revokes admission; changing the active batch digest makes the old
batch inactive without changing the standing policy. There is no implicit expiry,
fallback policy, automatic activation, or signature-based auto-enrollment.

`tool_sha256` pins the running executable's bytes, streamed through SHA-256.
The externally approved pair `(tool_revision, tool_sha256)` is an enrollment
attestation connecting source revision to binary; the check does not manufacture
that provenance from its current Git directory or claim reproducible builds.
Human enrollment must establish the pair. The host OS, Git, OpenSSH, executable
search path and external trust administration remain trusted platform inputs.
This check is not a defense against malicious processes running as the same user.

The isolated apply path requires a verified receipt from the trusted adapter;
candidate-provided `reviewed` metadata or copied authentication output is never
authority. It stages the exact candidate tree in a new external worktree and
reports the producer-derived packet digest, but never commits, pushes or signs.
This document adds no qualification, Phase GO, or signing authority.
