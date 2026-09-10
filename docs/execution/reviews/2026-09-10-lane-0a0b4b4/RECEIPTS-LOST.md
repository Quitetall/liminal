# Lane `0a0b4b4b` — provider receipts not retained

The two review records in this directory are lane seventeen's, and their
provider receipts are not. While clearing the working tree between fixes I
deleted `pass1-codex-gpt-5.6-sol.raw.txt`,
`pass1-codex-gpt-5.6-sol.session.jsonl` and
`pass2-mimo-direct-mimo-v2.5-pro.raw.txt` before copying them here, so the
codex rollout and both raw answers for this lane are gone.

What survives: both records, including each receipt's `session_id`,
`transcript_sha256` and `transcript_bytes`. What is lost: the bytes those
digests attest to, so the F-49 binding for this lane can no longer be checked —
only asserted.

What it does not affect: the lane was stopped at the reviews stage, it produced
no qualification claim, and all nine of its findings were verified at their
coordinates in the source rather than from the transcripts. Nothing in the
packet rests on this lane's receipts.

Recorded rather than quietly repaired: a regenerated transcript would hash to
something else and would be a different session, and writing one to make the
digest match is the forgery the receipts exist to prevent.
