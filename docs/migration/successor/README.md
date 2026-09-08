# SAS successor candidate

This directory is a non-authoritative successor workspace. The accepted
`0.1.0-proposed.1` SAS, its 777 requirement IDs, acceptance receipt, source map,
crosswalk, policy, historical sources and Warrants remain byte-for-byte intact.

`candidate.md` appends attributed source history to those accepted bytes. New or
changed passages are pending successor-incorporation disposition; that does not
revoke or re-request any decision already recorded in those passages. The
candidate is not registered, selected, accepted, a Phase GO, suite ratification,
or HAQP qualification.

The discarded duplicate proposal with digest `86e6cfa0e5b4e9a9f0e8f50264c3f17e85837129b764e2770a543a9e01cb8179`
and its alternate IDs are not adopted.

T1 successor incorporation remains open; AM-17.9 itself remains ratified and is
not being put back to a vote. The proposed scoped interpretation is that its
Class C treatment coexists with accepted SAS §3 only when every Class C finding
has a valid, exact-claim-scoped signed ruling. An unruled or invalidly ruled
Class C finding remains unresolved; Class C is not a blanket waiver. Class A and
B findings still block until fixed and followed by the required full rerun. This
interpretation needs successor acceptance and does not alter suite budgets, the
64-mutant floor, or full M19 breadth. AM-24.1 and
AM-24.2 match accepted GAP-002 sequencing and are source-history wording, not an
automatic new meaning. R-001 and R-002 retain their exact claim bounds, ruled
status, attribution, and signature condition. Phase 1 suite-review hashes and
counts remain evidence history, not a gate milestone. A machine pass cannot
dispose any of these impacts.

Run `python3 scripts/sas_successor.py generate` then
`python3 scripts/sas_successor.py check`. Generation reads committed bytes from
the pinned Git objects, never the dirty working tree.

The target `7e4ba39d6babb1e91a78aecd75bfa0262095c2c9` is currently local-only and is
not an ancestor of the accepted branch. A hosted or fresh-remote check must fail
closed until that exact retained target history is published and fetched. Do not
fabricate ancestry, merge with an `ours` workaround, or trust working-tree
copies. Current `origin/main` observation (`e67eda28`) is provenance only; no push
or hosted qualification is claimed here.

Because this bundle is an unselected external candidate, the active default CI
continues to enforce the accepted SAS and does not consume this side history.
`just sas-successor-ci` is the complete candidate-specific local lane: it runs
the unchanged active CI lane and then the successor checks. Mandatory hosted
integration remains blocked until the exact target history is published and a
successor-selection design is accepted; the candidate check fails closed when
its pinned commits are unavailable.
