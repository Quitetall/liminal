# HAQP reference-maintenance audit (AM-17.13)

Status: user-approved procedure, 2026-09-13. No reference updates have been
performed by the commit introducing this procedure. This is not qualification
evidence, a golden acceptance, or a phase decision.

## Allowed change

Only relocate an existing mutant's file/line coordinate to the same
implementation expression and mutation behavior, update its matching packet
`source`, and recompute the packet-digest mirror with the existing command.
The exception does not authorize changes to the production behavior itself;
those require their own applicable implementation authority.

Keep mutant identity, anchor expression, operator, family, requirement and
killing-test mappings, inventory size, coverage, assertions, thresholds, golden
values, qualification stage, and verifier algorithms unchanged. Finding
classification and AM-17.11's termination rule do not change. Never repair a
golden mismatch by copying the observed value into the expected value.

## Required evidence for each maintenance change

Before changing references, inspect both source versions and obtain an
independent review of the target and the mutant it produces. A matching line
or passing inventory check does not establish behavioral equivalence. Stop if
the target is missing, ambiguous, rewritten, or no longer supports the same
mutation. Do not use nearest-line selection, wildcards, or broader exclusions.

Commit a Markdown record under `docs/execution/reference-maintenance/` with the
reference changes. Use a unique descriptive filename; include these fields for
every affected mutant:

| Field | Required content |
|---|---|
| Authority | AM-17.13 and the exact separately authorized production commit, when one exists; otherwise explicitly state no production change or identify the pending authorized change without a self-hash |
| Mutant | Existing ID, family, operator, requirement and killing-test mappings |
| Old source | Full source commit ID, file:line, enclosing symbol, exact expression, and affected file SHA-256 |
| New source | File:line, enclosing symbol, exact expression, and affected file SHA-256; source revision is implicitly the commit containing this record, never its self-embedded hash |
| Behavior | Before/after mutation description and evidence that target identity and mutation behavior are unchanged |
| Independent review | Reviewer identity, reviewed source hashes, verdict, verified findings/dispositions, and durable review-artifact link/hash |
| Packet | Old/new packet digests, command used to derive the new digest, and exact changed fields |
| Verification | Commands, actual exits, log paths/hashes, preserved failures, and explicit unavailable/deferred tests |

Do not embed the containing commit's own hash in its contents: Git identifies
that revision, while the record binds the affected source bytes explicitly.
Where independent logs live outside Git, preserve them durably and bind their
hashes in the record. No approval or review is inferred from a blank field.

## Checks and evidence freshness

Inspect the complete diff for changes outside this exception. Run
`just haq-inventory`, the applicable already-active target tests, and `just ci`;
record each actual exit. Preserve any failure and resolve it only within its
authorized scope. Deferred or absent killing tests stay explicitly unmeasured;
do not un-ignore them or claim a measured kill from inventory validation.

Each commit still requires external `review_commit`; preserve its verdict and
verified disposition. Pre-change target review and post-commit review serve
different checks, and neither can be replaced with a self-issued PASS.

Retain previous packets, evidence, and review records in their original Git
history or durable hash-bound archives. New source or packet digests invalidate
old evidence as proof of the new candidate. Never rewrite old provenance to
look current. Generate any required replacement evidence through its existing
producer and qualify the new fixed candidate under the unchanged gates.
