# 0011. Freeze Jurisdiction measurement denominators

- **Status:** accepted
- **Date:** 2026-07-19
- **Deciders:** Brian
- **Related:** v4 §7.4, §-1.5; R4 §2; M11 D11.1–D11.4;
  `conformance/golden/denominator_counts.md`

## Context

Jurisdiction ergonomics can be gamed if operation or session denominators change
after profile results are known. M11 froze arithmetic before held-out scoring.
The denominator golden is SHA-256
`876752fe89a38d484e8a1dbf180d3e5b169bd4007559850d1483cef3e7d2f52e`.

## Decision

We will freeze these constants and counting rules:

- `COALESCE_GAP_MS = 2_000`: same-buffer edits with gap `< 2000 ms` and no
  intervening non-edit event coalesce; `2000 ms` splits.
- `SESSION_TIMEOUT_MS = 1_800_000`: explicit boundaries win; otherwise a client
  activity gap of at least 30 minutes closes the prior session.
- Root-cause key grammar is `<category>:<key>`. One visible incident is counted
  per distinct `(session, key)`; the maximum must remain at most one.

| Trace event | Jurisdiction-sensitive operations |
|---|---:|
| `trace_header`, `session_open`, `session_close`, `clock_advance` | 0 |
| `buffer_open` | 1 |
| `buffer_edit` | 0 individually; 1 per coalesced semantic transaction |
| `save` | 1 |
| `file_change_external` | 1, except 0 when its cause pair matches a prior `git_op` |
| `git_op` | number of affected files |
| `formatter_rewrite` | 1 |
| `holder_unavailable` | 0 |
| `holder_available` | 1 per distinct pending Overlay reconciliation attempted |
| `resolver_observe` | 1 |

Profile-declared transient drafts are excluded from both auto-resolution
numerator and denominator and remain visible in the separate transient report.

**What would falsify or reverse this:** discovery that an event row double-counts
or omits a real Jurisdiction-sensitive operation, or a new trace schema version.
Either requires a new corpus version, regenerated denominator golden, and a
superseding ADR before rescoring profiles.

## Consequences

- **Positive:** profile improvements cannot redefine success after seeing scores.
- **Negative:** correcting a denominator bug requires corpus versioning and full
  scorecard regeneration.
- **Follow-ups:** every future trace event type must receive an explicit row
  before entering a locked corpus.

## Alternatives considered

- **Count raw edits** — rejected because keystrokes inflate denominators without
  representing independent user operations.
- **Use aggregate incidents only** — rejected because one noisy root cause could
  be hidden across otherwise quiet sessions.
