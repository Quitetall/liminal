# Human acceptance of the Liminal SAS

Decision: **accepted by Brian Lam** in the conversation following migration
commit `d984f66ab53ebdb4ae658afdcb46665b11878303`.

User statement, verbatim:

> I accept the SAS revision.

The statement answers the immediately preceding request to accept
`0.1.0-proposed.1` as Liminal's sole governing specification, with SHA-256
`53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`.
The request is retained in [sas-acceptance-request.toml](sas-acceptance-request.toml).
The accepted document is
[LIMINAL_Software_Architecture_Specification.md](../sas/LIMINAL_Software_Architecture_Specification.md).

Recorded by Codex at `2026-09-06T02:42:50Z`. This timestamp records transcription,
not an independently supplied timestamp for the user's message. This receipt
transcribes the received human decision; it is not an agent's acceptance or an
independent verification disposition.

## Meaning and unchanged boundaries

Under SAS section 1, this exact revision is now the sole governing specification.
Its incorporation and reconciliation clauses apply. The original sources and
all accepted SAS bytes remain unchanged. In particular, the original proposal
header, version suffix, source map and crosswalk remain immutable proposal-time
artifacts; their wording is not a revocation of this later human decision.

This acceptance does not close Phase 0, authorize M18 implementation, give
M17.6 GO, ratify the Phase 1 suite, establish any Warrant obligation, resolve a
Warrant, approve a compiler release, or dispose OpenWarrant's ten controlled
deliverable digest mismatches. Those decisions and evidence remain separate.

## OpenWarrant registration remains pending

The controlled revision record still has `state = "proposed"`. Liminal has no
`docs/authority/roles.toml`, so the tool-emitted request has no eligible
acceptors. This receipt is not a successful `war sas accept --response` import
and does not change the tool's authority projection to accepted.

OpenWarrant's `docs/authority/roles.toml.example` says: "this file is written by
a human in an editor, and by nothing else." Its `authorizer` role grants more
than acceptance of this one SAS. The received acceptance does not supply a
repository-wide role assignment, so none is inferred or created here. No
signature, acting role, grant or acceptance response is manufactured.

Remaining registration work: the repository owner establishes the human
authority register and supplies the acting-role response for the already
received acceptance. Then ingest that response through
`war sas accept 0.1.0-proposed.1 --response <file>`, recompile and verify the
generated views. The exact accepted digest must still match. This is recording
the decision already received, not a request to reconsider or repeat it.

Until then, OpenWarrant status remains explicitly draft; no authoritative phase
completion is inferred from this receipt. Campaign preservation and final
baseline eligibility checks still precede integration.
