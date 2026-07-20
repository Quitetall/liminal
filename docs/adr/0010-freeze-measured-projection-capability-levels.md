# 0010. Freeze measured projection capability levels

- **Status:** accepted
- **Date:** 2026-07-19
- **Deciders:** Brian
- **Related:** v4 §8.4, §124, Part XXIII; M09; M10;
  `conformance/golden/anchor_recovery.md`;
  `conformance/golden/pandoc_loss.md`

## Context

M09 and M10 were time-boxed measurements, not product implementations. Their
declared levels must equal frozen evidence even when a higher level was desired.
The annotated-source report is SHA-256
`84a987e96c12e65e7d7d5b5d6cc411323d4ed1db1a78a3e24f1976d38d84f484`;
the Pandoc loss report is SHA-256
`b83318659df74968aeb2609a9dd666129156ea60ae1875625d5885359167e8a0`.

## Decision

We will declare annotated-source capability **Level 2** and Pandoc conversion
capability **Level 1**.

Annotated-source demonstrated canonical round-trip on its supported subset but
missed Level 3 recovery thresholds and cannot claim Level 4 without a real merge
runtime. Pandoc demonstrated import/export with an explicit loss report, but not
canonical round-trip preservation.

**What would falsify or reverse this:** a new time-boxed, frozen report using
the production CST or adapter seam whose recomputed thresholds support a
different level. Report changes require a new measurement record, never editing
these declarations ahead of evidence.

## Consequences

- **Positive:** capability labels remain measurements rather than roadmap claims.
- **Negative:** richer authoring and Pandoc fidelity remain explicitly limited.
- **Follow-ups:** Phase 0 task 12 re-measures annotated-source recovery over the
  real CST before any Level 3 claim.

## Alternatives considered

- **Keep annotated-source at Level 3 aspirationally** — rejected because measured
  non-git recovery falls below the frozen threshold.
- **Call Pandoc Level 2 because many fixtures survive** — rejected because
  declared loss is not canonical round-trip.
