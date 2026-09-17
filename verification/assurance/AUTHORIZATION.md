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
  --base FULL_COMMIT_ID --target EXACT_MUTANT_ID
```

Exit zero authenticates only the selected batch scope. Output explicitly says
`batch_authenticated: true`, `apply_authorized: false`, and
`independent_review: not-established`. The JSON is not signed, is not a reusable
capability, and must never be accepted as proof by a future apply implementation.
Without `--authorization-only`, this partial implementation refuses full admission.

The remaining checks include actual closed-registry membership, exact candidate
and patch binding, unchanged target/mutation behavior, independent review, and
isolated application. These are not established by authenticated scope. Admission
and revocation must be rechecked before effects; a historical check does not
authorize a later action. No automatic apply command exists.

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

Future apply must obtain independent review directly through its trusted adapter;
candidate-provided `reviewed` metadata or copied authentication output is never
authority. This document adds no qualification, Phase GO, or signing authority.
