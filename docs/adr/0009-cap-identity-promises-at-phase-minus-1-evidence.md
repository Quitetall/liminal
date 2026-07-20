# 0009. Cap identity promises at Phase -1 evidence

- **Status:** accepted
- **Date:** 2026-07-19
- **Deciders:** Brian
- **Related:** v4 §19, §19.1–19.3, Part XXIII §120; M03 D03.5; M07;
  `conformance/golden/identity_matrix.md`

## Context

Part XXIII §120 leaves identity-grade ceilings and profile defaults open until
the Phase -1 identity torture corpus measures them. M07 exercised six strategies
over eleven operations and eight seeds. Its frozen matrix is
`conformance/golden/identity_matrix.md` (SHA-256
`20294974b3c1c36857ab5eb9ea1ff9c57f1428364dd1242e698ba686f799bc4e`).

Managed identity survived every Holder-observed graph operation but degraded on
foreign operations, exactly matching its definition: the promise holds while
the Holder observes. Anonymous external text suffered ambiguous and lost cases;
claiming Explicit or ContentAddressed entity continuity would be dishonest.

## Decision

We will keep the external-file profile's required identity at **Anchored** and
the graph-native profile's required identity at **Managed**. Anonymous external
text is capped at Anchored unless explicit evidence raises a particular subject;
inline durable IDs compute Explicit, and exact resource bytes may compute
ContentAddressed without changing entity-continuity claims.

COMMENT Relations and EXTERNAL_VALUE Nodes use graph-native governance and
compute Managed. EXTERNAL_VALUE remains Managed only while reactor observation
maintains it; payload hashing identifies observed bytes, not continuing entity
identity.

**What would falsify or reverse this:** a new frozen matrix over the production
parser or a graph-native foreign-operation experiment demonstrating a weaker
worst outcome than these defaults, or stronger evidence sufficient to raise a
ceiling without hiding ambiguity or loss.

## Consequences

- **Positive:** Contract promises equal demonstrated continuity; foreign edits
  cannot inherit stronger identity from payload representation.
- **Negative:** anonymous text and unobserved external values surface uncertainty
  instead of claiming seamless continuity.
- **Follow-ups:** Phase 0 re-measures annotated-source identity over the real CST
  before raising any profile level or identity ceiling.

## Alternatives considered

- **Default external-file to Explicit** — rejected because copy, merge, and
  recreation produce ambiguity or loss without durable markers.
- **Treat EXTERNAL_VALUE object payload as ContentAddressed identity** — rejected
  because a content hash identifies one observation's bytes, not the external
  entity whose value changes.
