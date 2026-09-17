# 0022. Maintain assurance without requalifying it

- **Status:** implementation authorized; activation requires human enrollment
- **Date:** 2026-09-16
- **Decider:** Brian (conversation: "Implement the plan.")
- **Related:** ADR-0004, ADR-0020, ADR-0021; AM-17.13, AM-17.14
- **Baseline:** `54b6450c15f161978ad63cc0f999d5090bc577f4`

## Context

Assurance must remain maintainable as implementation changes. Hosted Linux lacks
several local checks; standalone resource controls are not in either merge path.
More tests alone do not establish coverage, independence or affordable upkeep.

## Decision

Preserve the frozen HAQP instrument. Add one maintenance module to
`liminal-xtask`, with `just` as entrypoint. A family-level catalog references
existing requirements, threats and obligations, without replacing authority.
New tests inherit registered targets; new targets require classification.

Profiles: development (insufficient for merge), merge (complete Linux checks),
portability (existing macOS/Windows coverage), scheduled (advisories and
informational beta), qualification (existing HAQP producer). Registered command
IDs resolve to fixed argv, never metadata shell commands. Generate workflow
wiring while preserving job names/platforms. Drift checking never repairs drift.
An independent mandatory-command baseline catches mutually consistent omissions.

Merge retains nextest and threaded tests, adds resource harness run/lint, and
restores ADR-0004 local/hosted Linux equivalence including advisories. Reuse
existing resource enforcement: two CPUs, 4 GiB, no swap, 256 tasks, 600 seconds
per bounded resource invocation. Missing limits are infrastructure failure, never
permission to run unbounded. Hosted parity needs observed hosted execution.

Observe costs before setting budgets. Reports distinguish pass, failure,
infrastructure unavailable, deferred and unexecuted, with sample count,
environment, cache classification and observed resources (unknown if unavailable).
Change impact is advisory only. No automatic retirement, quarantine, retry,
timeout widening or assertion weakening.

### Narrow standing amendment authority

AM-17.13 coordinate moves may become automatic only under human-signed exact
policy bytes using OpenSSH namespace `liminal.assurance.policy.v1`. External human
trust configuration pins signer principal/public key, active policy digest,
trusted maintenance-tool revision and repository scope. Agents never sign, read
private keys, enroll trust or broaden policy. Missing/revoked/mismatched trust
disables apply, not tests. This is a trusted-host workflow, not isolation from a
hostile same-user process.

Proposals bind old/candidate commits, source hashes, exact targets, enclosing
implementation/declaration context, patch and checks. A real Rust parser using
already locked tooling dependencies establishes unchanged enclosing code bytes,
unchanged expression/operator and unique exact target. Unsupported syntax and
semantic changes are proposal-only. Independent target/mutation review remains
mandatory.

### Human-authorized batches and independent review

Brian approved this boundary in conversation on 2026-09-16 ("yep" in response
to the explicit coordinate-only boundary). Human approval defines each batch's
base revision, permitted targets, change class, and tool/policy versions. The
standing signed policy does not let the maintenance tool authorize or expand its
own batch. Activation still requires the external trust enrollment described
above; this conversation does not constitute a cryptographic signature.

Automatic maintenance is limited to the coordinate-only changes described here:
the target remains unique and the enclosing implementation remains unchanged.
Changes to behavior, mutation operators, expected results, thresholds, trust pins,
or qualification rules require human approval and are outside automatic apply.
Ambiguous eligibility and out-of-scope changes stop for human review.

The externally pinned maintenance tool obtains independent target/mutation review
directly and binds it to the exact proposed patch. Its record includes reviewer
identity, patch digest, verdict, and verified finding dispositions. LAMU supplies
review evidence, not authorization. Candidate-supplied review metadata is not
authentication; unavailable, inconclusive, or mismatched review refuses apply.
Routine changes inside a human-authorized batch do not require a separate human
review signature for every proposal. This decision defines authority, not proof
that the acceptance checks have been implemented or exercised.

Brian approved separate batch manifests in conversation ("approved 1."). Each
batch is human-signed over its exact bytes using OpenSSH namespace
`liminal.assurance.batch.v1`, distinct from the standing-policy namespace above.
It binds the standing-policy digest, base revision, exact permitted targets and
change class, and trusted tool revision and executable digest. External human
trust configuration selects the active batch digest. Missing, mismatched or
revoked activation refuses admission. A new batch does not require changing or
re-signing the standing policy. Agents do not sign or activate real policies or
batches; disposable test keys remain authorized for development controls.

Only exact registry coordinates, matching packet sources, producer-derived digest
mirrors and audit records are eligible. Apply uses a trusted tool revision in a
fresh isolated worktree, rechecks bindings, and never overwrites user work or
automatically commits, pushes or signs. Existing producers regenerate affected
evidence. Interrupted/failed work remains incomplete. Historical evidence stays
historical; maintenance success does not qualify a new tree.

## Delivery and limits

Follow `verification/assurance/PLAN.md`. Each code slice needs public CLI negative
controls and independent review. Tooling-only parser dependencies are authorized;
no production parser or Phase 1 implementation is authorized. Accepted SAS bytes
and `spec/v4/` remain unchanged. Phase GO, proof establishment and suite
ratification remain human acts.
