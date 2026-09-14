# Baseline consolidation — 2026-09-13

Status: integration record, not Phase 0 closure, formal adoption, HAQP
qualification, suite ratification or release approval. Final verification
receipts are retained outside the source tree under
`/mnt/4tb/liminal-formal-evidence/baseline-2026-09-13/`.

## Authority and selected sequence

Brian confirmed the other agent had stopped and instructed:

> consolidate baseline. The other agent is now stopped. We harden the current core by construction, improve codebase architecture if we can, close phase 0 honestly and all M17 honestly, then we can start phase 1.

This clears the concurrent-writer hold and authorizes integration and the
previously selected current-core hardening work. It does not supply a Phase 0
GO, signatures for pending rulings, formal-companion adoption or release authority.

The central [production roadmap](../roadmap/PRODUCTION_ROADMAP.md) still defines
the full program and first qualified OpenWarrant compiler distribution. Its
baseline/campaign status paragraphs describe its migration-time snapshot, not
this checkout. Current execution order is:

1. Consolidate and independently review this baseline.
2. Implement the [current-core safety contract](../../verification/PLAN.md).
   Prefer small checked interfaces and one interpretive implementation; no
   generic rewrite, new policy interpreter or removal of retained tests.
3. Establish applicable production proofs and adapter evidence, then close
   every M17 obligation and obtain its separate human decisions.
4. Only after M17 closure and explicit Phase 1 authorization, execute M18–M24
   with full M19 breadth. Later compiler integration/parity/distribution remains
   the roadmap's separate delivery chain.

## Exact integration inputs

| Input | Revision and handling |
|---|---|
| Campaign main | `9478f4b57f8bace18313953f18b1a267853037e1`; all later campaign source, rulings, findings and evidence retained |
| Reference maintenance | `503fe7f0`; separately reviewed AM-17.13, docs only |
| Accepted SAS archive | `9e76fc99027c55ded5bd0cc61da43b0f2b68b049`; restore exact selected artifacts, not an old checkout over current source |
| Formal foundation | `b08b8ea05aa4fe6f8b5af258c73fabb22550b727`; history-preserving merge of its three commits, with no production-core implementation claim |
| Unaccepted successor | `e78c1fee`; not restored or adopted |
| User-owned directory | `liminal-5.3-spark/`; remains outside the tracked baseline and is not modified or deleted |

The containing merge commit identifies the resulting revision; no self-hash is
embedded here. Parent history is preserved, including the prior SAS merge and
revert. No push or signature is performed by this consolidation.

## Preserved artifact and source evidence

All **129** restored files matched their Git blobs at the accepted archive.
Exact restored roots/files are `docs/migration/`, `docs/roadmap/`,
`docs/sas/`, `docs/warrants/`, `docs/adr/generated/`,
`scripts/sas_migration.py`, `scripts/test_sas_migration.py` and
`openwarrant.toml`. This new record is not part of that historical set.
Generated records are restored, not regenerated or hand-edited.

The accepted SAS retains **777 unique requirement rows** and SHA-256
`53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73`.
Its proposal labels and historical source crosswalk are unchanged. Human
acceptance remains established by the existing receipt; OpenWarrant registry
acceptance remains pending.

Of **65** incorporated source paths, **55** still match the archive and the
following **10** differ. Old SHA-256 refers to the accepted archive; current
SHA-256 refers to the retained source bytes in this consolidation.

| Source | Accepted-archive SHA-256 | Current SHA-256 |
|---|---|---|
| `docs/execution/M17.md` | `8b96866c9a8dadde408b185aad1f874b130706709504f0010b25b1ac39056b94` | `beb8f8816b8ca7e172e7ace628d175c7a0b26625c243897d9c8203e7d412884c` |
| `docs/execution/M18.md` | `c42c82d6a40d2c8ed61f018aea4330079daae02d1525d006dc1b8399c7ed4e05` | `4ba30a144c42b7c437bfaf2210caf84fd5a9c3cfa2af16e0625b215c41399c10` |
| `docs/execution/M19.md` | `cd82bb0f5046f54e64a4e5adcc139157ddb5bae27b3e898f24d491933a3d2a16` | `f222177419f838290c9923ef9bfb9cbd6edfe395ef1cb7c05ebd887d6e5e48b2` |
| `docs/execution/M20.md` | `af09127762193fa819e675c6c2080f53a70a3e2c54242574843c2fd11b67c6be` | `8e5b979270c4f0914eb187ba312a523e5ef2879562e1da51618d522d8a89eee4` |
| `docs/execution/M21.md` | `627690ab6d9a6aba0d04a69859a200bd2e60554ff65a7b3287b8d14b72c860a0` | `85e5d1b5a5cbc53e5a6a975387d131447ddef9ff804a70ef366a8391a5524749` |
| `docs/execution/M22.md` | `b9c9ee2cd345a3b2fab7824018fff13d608be94e9e4625aae1cc598933975eb3` | `451b722736becf213efb2e8b7174491852860183053b132431f1a30a4fdc5977` |
| `docs/execution/M23.md` | `98d2400d8a6cd4dba75e204f6d9bbbbcb3481b0b72541384d235cc9cbd75bc54` | `f6d9820f79ab3da553d8cc2b8b3c1ef58cde9f073347cec7597b7ae34f56bb76` |
| `docs/execution/M24.md` | `6650233bbf781bfc7d270ce315234281fdbea69030f3cf89a11d597b84437359` | `ce427db214b740cdeb0b9e4b32955ab38848a53d1509c46a2a950c025857cc57` |
| `docs/execution/phase0-amendments.md` | `2ec7ac5a8748fbdbe6fcf8a36326664b9a1bf46ed5e9eb1680009723db914cd6` | `2d690c4c2bd5ce4f646192b5406277646140b028344d100c17e3b88b1390e1aa` |
| `docs/execution/phase1-suite-review.md` | `741ebde386f3a13e10d0620efaa83b15f8a47139a2d216ecd83b5ac913f5b4c9` | `d1257e4cdeafdf1a4bb89900f42695721860e3a3047faaec18b2fb50213c9f79` |

### Meaning of the differences

- M17 retains campaign amendments through AM-17.11 and approved AM-17.13.
  F-73/RISK-006 and the relation between the accepted SAS and later instrument
  termination rules still require explicit disposition before GO. This
  integration records their coexistence, not an invented precedence decision.
- M18–M23 retain AM-24.2's opening/exit prerequisite reconciliation; M24 retains
  AM-24.1. These preserve accepted ADR-0021's HAQP-1a → M17.6 authorization →
  HAQP-1b at M24 → separate suite ratification sequence, not test activation.
- The phase ledger retains those entries and AM-17.13.
- The proposed suite packet document retains current digest mirrors and
  corrected table shapes. Its NOT_RUN fields are not rewritten from old logs.
- All other incorporated sources, including canonical v4/R4 and ADR-0020/0021,
  retain exact bytes. Historical source bytes remain in Git and in the SAS.

This inventory is not semantic adoption of every later amendment. M17 closeout
must disposition the active ledger and any authority conflict explicitly.
No historical source, packet, golden or verifier algorithm was rolled back to
make the migration checker pass.

## Verification performed before integration commit

- Exact restored-file byte comparison: exit 0; 129 compared, zero mismatches.
- SAS digest/requirement inventory: expected digest and 777 unique rows.
- At the clean accepted archive worktree, original
  `python3 scripts/sas_migration.py check`: exit 0.
- At that same archive, original migration unittest suite: **40 tests passed**,
  exit 0. This is archive evidence, not current-source conformance.
- On the combined candidate, `just formal-check`: exit 0, explicitly
  **structural only / NOT QUALIFICATION**.
- On the combined candidate, `just formal-bootstrap-self-test`: **22 passed**,
  exit 0. Production obligations remain not established.
- `just formal-gate 0`: expected refusal, exit 1, because adoption/evidence
  remains missing. This is not a successful formal gate.
- AM-17.13's `just haq-inventory`: exit 0.
- AM-17.13 actual LAMU `review_commit`: **PASS WITH NITS**, critic PASS.
  Wording nits were verified as optional; the user quote remains verbatim.
  Review record: `/mnt/4tb/liminal-formal-evidence/reviews/503fe7f0-review.jsonl`.
- Main-checkout `just fmt-check`: exit 1 from unrelated recursive
  `liminal-5.3-spark/` TOML files. Preserve that result and directory; use an
  isolated checkout of the resulting commit for full CI. Do not narrow check
  scope or format user-owned files to obtain green output.
- Current-main migration check: exit 1 at `docs/execution/M17.md`, as required
  by the archive checker's unchanged historical-source guard.
- Formal Rust registry tests: **12 passed**, exit 0.
- Whole-restoration `git diff --cached --check`: exit 2 from whitespace in
  the exact accepted SAS transcription and twelve archived milestone YAML
  files. Those bytes remain unchanged. The integration-code/navigation-only
  diff check exits 0; it is not presented as a whole-restoration pass.
- Fresh integration `review_diff`: **PASS WITH NITS**. The critic's macOS-CI
  failure claim was checked at `test_run.py:279–289` and workflow lines 43–58:
  missing `/proc` is caught, and the hosted matrix invokes Cargo directly,
  not `just ci`. The timeout-bound suggestion is not accepted: widening
  1.5 seconds to 3 seconds could admit the deliberately two-second surviving
  child. No timing failure was observed. The Linux-specific process-state
  witness establishes no non-Linux process-death guarantee. Optional naming
  and historical-wording nits are retained without changing the imported code.

Full CI, exact meter output, current HAQP inventory and external integration
review must be recorded against the resulting commit before this integration
is considered verified. Archived reviews of the formal foundation are not a
fresh review of this merge.

The restored migration checker is intentionally an archive checker: it
requires incorporated live source files to equal its old baseline. It must
refuse on current main's ten changed sources. Do not wire it into current CI as
though that precondition held, weaken its guard, or call its refusal SAS-byte
corruption. Current-source qualification and successor adoption are distinct
remaining work.

## Remaining hardening and M17 work

The formal registry contains fourteen current-core obligations and two future
extension rows. Bootstrap does not discharge any of them. Implement checked
store/repair authority, Basis selection, ILRP intent/apply/ack/finalize/recover/
revert, capture retention, exact durability receipts and evidence admission.
Test actual Linux adapters and declared resource limits; keep production and
verification decision bodies identical.

M17 remains open. Confirm M17.5's exact candidate eligibility and campaign
receipts; settle the oracle-coverage/termination question; disposition unsigned
rulings without inventing signatures; reconcile every named meter delta and
amendment; obtain M17.6's explicit human GO; rerun M17.7; deliver M17.9 for
human adversarial review. Applicable formal-companion adoption and human
authority-register work remain separately recorded decisions.

M18–M24 gate names, frozen criteria, full M19 breadth, locked corpus handling,
HAQP-1b mutation requirements and final human suite ratification are unchanged.
