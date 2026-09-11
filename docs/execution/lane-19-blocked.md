# Lane 19 blocked: the review prompt crossed a hard 1 MiB limit

**2026-09-11.** `just haq-lane` refused at its first stage. The cause is not a
finding and not a credential; it is a size limit the campaign grew into.

## What happens

`scripts/haqp_blind_review.py` builds one prompt per pass and sends it to
`codex exec`. The provider refuses:

```
turn/start failed: Input exceeds the maximum length of 1048576 characters.
(code -32602) input_error_code=input_too_large max_chars=1048576 actual_chars=1126460
```

The error surfaced first as `codex wrote no final message (exit 1)` with a
`Bearer error="invalid_request"` tail, which is a separate MCP transport
message that happened to occupy the last 400 characters the runner captures.
Codex authentication is fine: `codex exec --model gpt-5.6-sol` answers a small
prompt, and the same model answers a 500 KB one.

## The measurement

| file | bytes | share |
|---|---:|---:|
| `crates/liminal-xtask/src/haq.rs` | 890,630 | 84.4% |
| `conformance/haqp/packet.json` | 57,285 | 5.4% |
| `crates/liminal-format/src/lib.rs` | 51,835 | 4.9% |
| `docs/execution/phase1-suite-review.md` | 22,681 | 2.2% |
| `crates/liminal-query/src/lib.rs` | 13,685 | 1.3% |
| rulings (7 files) | 13,139 | 1.2% |
| `crates/liminal-cst/src/lib.rs` | 5,519 | 0.5% |
| **total** | **1,054,774** | |

Over the cap by about 6 KiB — 0.6%. The gate's own source is 84% of the prompt,
and it has grown by roughly 200 KB over this campaign because every finding
fixed adds code, its reasoning, and its test.

## Why this is Brian's call

ADR-0020 §6 is about what an independent reviewer is given. Every option
changes that, and Protocol §3 says not to improvise qualification semantics:

1. **Drop the gate's test modules from the prompt.** Frees ~65 KB. The reviewer
   has used tests productively — F-55 found six mutants anchored inside
   `mod tests` because it could see them.
2. **Drop a production surface** (`liminal-format`, 52 KB). Removes a file the
   reviewer currently attacks.
3. **Deliver the files in the reviewer's scratch directory instead of inline,**
   with `--cd` pointed at it. Same document set, no size limit, blinding
   preserved because only those files are present. The difference is that the
   material becomes *available* rather than *guaranteed in context* — a real
   weakening of "the reviewer saw this", and the reason I did not just do it.
4. **Split each pass into two calls.** Breaks §6's one isolated session per
   pass.

**Recommendation: 3, with 1 as the fallback.** The document set is what §6
constrains, and 3 keeps it exactly while removing the ceiling permanently; the
cost is that a reviewer could skip a file, which the record would show as a
thinner review rather than as a false pass. If that cost is unacceptable, 1 is
the smallest real reduction, and it should be recorded as a narrowing of what
the reviewer sees rather than as plumbing.

Either way the fix is an amendment, not a patch: the prompt will cross the cap
again as the gate grows, so whatever is chosen needs a rule that keeps it under
the limit by construction.
