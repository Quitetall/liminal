# SAS migration record

This directory contains migration inputs, traceability and review records. It
is not another governing specification. The only SAS candidate is
[`../sas/LIMINAL_Software_Architecture_Specification.md`](../sas/LIMINAL_Software_Architecture_Specification.md).
It remains proposed until a human accepts its exact revision and digest.

## Reproduce and inspect

```sh
python3 scripts/sas_migration.py check
python3 -m unittest discover -s scripts -p 'test_sas_migration.py'
just sas-check
```

The checker needs the historical Git baseline
`0d8c32a4ae1afda9808e55c3c291445aad4e3d60`; fetch history in a shallow clone.
It compares preserved source files with baseline bytes, independently reverses
the SAS's quote wrappers, checks complete block coverage and stable IDs, and
checks generated output drift. It never opens protected held-out contents.

`consolidation-policy.md` is the reviewed generator input for authority,
reconciliation, phases and first-delivery requirements. `source-map.json` is
the explicit source inventory and append-only requirement assignment registry.
`source-crosswalk.json` is generated: source paths/hashes/ranges, classification,
SAS anchors, HAQP requirement/test references, declared evidence categories,
Warrant links and explicit missing-evidence gaps. All HAQP links describe the
frozen baseline; they do not certify a later campaign result.

After reviewing an input change, regenerate with:

```sh
python3 scripts/sas_migration.py generate
just sas-check
```

Do not hand-edit the generated SAS or crosswalk, reset the assignment map, or
update a historical source/hash to conceal a disagreement. An accepted SAS
revision is immutable. A later change requires the revision/decision process,
retained prior bytes, and Warrant impact analysis. The initial migration's
baseline guard deliberately refuses changes to its preserved source set;
integration with campaign changes needs explicit source-history reconciliation,
not silent regeneration against whatever HEAD happens to contain.

## Coverage and semantic limits

The unit of incorporation is a structural Markdown section outside code fences.
Every source byte is retained, including whitespace, examples, code and original
headings. Each normative, mixed or open-gap section has one stable requirement;
its compound obligations are conjunctive. Context and historical status do not
become new obligations. Detailed classification reasons remain reviewable.

Lossless coverage proves no source text was dropped. It does not prove every
semantic conflict has been resolved or every requirement implemented. The SAS
reconciliation table defines scoped precedence; unsupported claims and missing
tests remain explicit gaps. Source-presence evidence is not runnable-test,
qualification, independent-verification or release evidence.

`warrant-index.json` records the twelve proposal dependencies. Human decisions,
independent dispositions and resolutions remain absent until actually received.
OpenWarrant's `controlled` adequacy/independence warnings are expected for work
whose independent acceptance evidence has not yet been produced.

## Integration and adoption

Development uses two separate worktrees:

- Liminal: `migration/sas-roadmap`, based on `0d8c32a`.
- OpenWarrant: `migration/program-sas-phases`, based on `8bc3978`, preserving
  the existing local integration packet work rather than resetting it to the
  fetched remote's earlier head.

The original Liminal campaign checkout remains untouched by this migration.
Preserve its eventual campaign and evidence commits before integrating. Then
reconcile all original-source changes, rerun repository checks and assess the
exact final candidate's HAQP source/parent/tree eligibility. A changed source
baseline requires qualification again under the existing rules. No run in this
migration claims HAQP completion.

See [production roadmap](../roadmap/PRODUCTION_ROADMAP.md) for all delivery
dependencies and [adoption decision](adoption-decision.md) for the proposed
authority change. The [verification record](VERIFICATION.md) retains check
results, review limits and unresolved adoption blockers.

The [tool-emitted acceptance request](sas-acceptance-request.toml) names revision
`0.1.0-proposed.1`, 777 requirements, and SHA-256
`53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`.
Its `eligible_acceptors` is empty: Liminal has no human-authored authority
register. OpenWarrant requires a human to establish
`docs/authority/roles.toml`; this migration does not invent that authorization.
The request's `architecture_changing=false` describes its lack of a prior
registered SAS to diff, not absence of a governance change from v4/R4.

The current OpenWarrant CLI records its fixed performer identifier `claude`
even when invoked by this Codex session. Tool-produced journals are preserved
verbatim; that identifier is a CLI role label, not evidence that Claude performed
an independent review. No independence claim, verifier or human role is derived
from it.
