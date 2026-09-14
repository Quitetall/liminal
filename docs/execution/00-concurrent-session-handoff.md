# Working on Liminal alongside the M17.5 qualification

Written 2026-09-12 for a session doing other work while HAQP-1 qualification is
open. Read the four hazards before editing anything under `crates/`.

Updated 2026-09-13 under user-approved AM-17.13: reference maintenance is a
narrow, tracked exception to the freeze. It does not permit changed test
behavior or automatic golden refresh. The audit requirements are in
`haqp-reference-maintenance.md`.

## 1. Editing production code moves mutant anchors, and the gate will refuse

`conformance/haqp/packet.json` pins **65 mutants to `file:line` coordinates**
in 16 files across `liminal-cli`, `-cst`, `-format`, `-graph`, `-jurisdiction`,
`-revision` and `-source`. Inserting or deleting a single line above an anchor
moves it, and `just haq-inventory` then refuses:

```
P1-M013 mutant source coordinate crates/liminal-format/src/lib.rs:353
does not name family anchor ".all(|byte| byte.is_ascii_alphanumeric() ...)"
```

This is expected, it is RISK-003, and it is not your bug. The recipe:

1. The closed registry in `crates/liminal-xtask/src/haq.rs` —
   `fn mutant_source_coordinate` — stores each anchor's **exact line text**.
   That text is how a moved anchor is found again.
2. For each refused mutant, inspect the implementation at the old and new
   locations in their exact source revisions. Establish the same expression, enclosing symbol,
   and mutation behavior; record source hashes and independent verification.
   Duplicate text is not an identity proof. Never choose the nearest line;
   unresolved ambiguity or a changed target stops for a separate decision.
3. Update only the file/line coordinate in BOTH the registry tuple and the
   packet's `source` field. Preserve the anchor expression, mutant identity,
   operator, requirement/test mappings, and all qualification criteria.
4. `cargo run -p liminal-xtask -- haq packet-digest`, then paste the digest
   into the `| packet digest |` row of `docs/execution/phase1-suite-review.md`.
   The markdown carries the digest and the gate compares them.
5. Run `just haq-inventory`, then `just ci`, recording actual exits and retaining
   failures. A green check does not replace independent target verification or
   grant permission to change a golden. Commit the maintenance audit alongside
   the reference changes; new candidate qualification remains separate.

If an anchor's expression no longer exists, stop: re-anchoring is not covered by
AM-17.13's reference-only approval. See F-55, F-59, F-61 and F-64 in
`m17-5-adversarial-findings.md` for what a legitimate anchor has to support.
`anchor_supports_operator` will tell you: a `predicate-deletion` needs a real
negation, a `missing-enum-dispatch` needs the `match` and not one of its arms.

## 2. Do not run a qualification lane concurrently

`just haq-lane` runs as the systemd user unit `liminal-lane` and writes into
`conformance/haqp/evidence/`. Two lanes at once corrupt each other's evidence.
Check with `systemctl --user status liminal-lane` before starting one.

A lane also leaves untracked artefacts — `access/scopes/`, `lanes.json.parts`,
`scope-rows.ndjson`, and `reviews/*.raw.txt` / `*.session.jsonl`. The last two
are receipts and ARE tracked now; the others are not and should be removed, not
committed.

## 3. Two evidence files are regenerated, not edited

`conformance/haqp/evidence/generated.json` and `canaries.json` are outputs:

```
cargo run --release -p liminal-xtask -- haq generate --cases 100000
cargo run -p liminal-xtask -- haq run-canaries
```

If you change anything the generated oracles observe, the golden in
`haq.rs` (`generated_case_builders_match_their_recorded_goldens`) can fail with
the computed digest as `left` and the recorded one as `right`. Preserve the
failure and investigate against the specification. Do not re-record the golden
from `left`: AM-17.13 excludes expected-result changes. Any justified golden
change requires separate T1 approval before regeneration and qualification.
Generated outputs are not edited by hand, and historical evidence remains
bound to the source that produced it.

## 4. The freeze, and what is still open

AM-17.11 (ratified 2026-09-11) froze the qualifier's own implementation —
`haq.rs` and `scripts/haqp_*`. AM-17.13 permits only the tracked, behavior-preserving
reference maintenance described above; it does not change the finding or
termination policy. A finding against the qualifier is recorded as residual
risk rather than fixed; `cargo run -p liminal-xtask -- haq churn <file:line>`
decides which side of that line a coordinate falls on, from `git blame`.

**If you are hardening the production crates, your work is on the "fixed" side
of that freeze** — the freeze is about the measuring instrument, not the thing
being measured.

Open and belonging to Brian:

- whether AM-17.11's rule should count oracle COVERAGE as suite rather than
  instrument (F-73's A01, recorded as RISK-006)
- M17.6 GO/NO-GO on Phase 0
- M17.8's meter: AM-17.4 predicts 356 active / 43 backlog, `just gates` reads
  565 / 47
- M17.9, delivering the suite for adversarial review

Six residual risks are recorded in the packet. They are disclosures, not
dismissals: each names something known to be imperfect in the instrument.
