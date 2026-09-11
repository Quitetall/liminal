#!/usr/bin/env bash
# Record one complete HAQP qualification-lane wall-clock run.
#
# Usage: scripts/haqp_campaign_clock.sh <run-id> <output.json> -- <command> [args...]
# The command's stdout/stderr remain the caller's artifacts; this wrapper only
# records exit state, clean-tree state, and elapsed seconds for ADR-0020 §7.
set -uo pipefail

if [ "$#" -lt 4 ] || [ "$3" != "--" ]; then
  echo "usage: $0 <run-id> <output.json> -- <command> [args...]" >&2
  exit 2
fi

RUN_ID="$1"
OUT="$2"
shift 3
STARTED=$(date +%s)
COMMIT=$(git rev-parse HEAD)
TREE=$(git rev-parse "${COMMIT}^{tree}")
COMMAND=$(printf '%q ' "$@")
# The lane refuses to run unwrapped (M17.5 F-35): campaign.json and its receipt
# are required by verify_campaign_clock, nothing else writes them, and the
# orchestrator never called this wrapper — so a completed lane would have failed
# at `just haq-verify` on a missing artifact after four hours.
export HAQP_CAMPAIGN_CLOCK="$RUN_ID"
"$@"
CODE=$?
FINISHED=$(date +%s)
ELAPSED=$(( FINISHED - STARTED ))
# SOURCE-clean, not tree-clean: the lanes this wraps write evidence, and
# verify_campaign_clock requires run.clean — so a blanket `git status` check
# marked every legitimate campaign unclean. See scripts/haqp_paths.py.
CLEAN=1
if ! python3 scripts/haqp_paths.py >/dev/null; then CLEAN=0; fi
WRAPPER="scripts/haqp_campaign_clock.sh"
WRAPPER_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
WRAPPER_SHA256=$(sha256sum "$WRAPPER_FILE" | cut -d' ' -f1)
RECEIPT="conformance/haqp/evidence/campaign/$RUN_ID.receipt"
mkdir -p "$(dirname "$RECEIPT")"
printf 'run_id=%s\ncommit=%s\ntree=%s\ncommand=%s\nstarted_epoch=%s\nfinished_epoch=%s\nelapsed_s=%s\nclean=%s\nresult=%s\n' \
  "$RUN_ID" "$COMMIT" "$TREE" "$COMMAND" "$STARTED" "$FINISHED" "$ELAPSED" "$CLEAN" "$([ "$CODE" -eq 0 ] && echo pass || echo fail)" >"$RECEIPT"
if ! RECEIPT_BLAKE3=$(cargo run -q -p liminal-xtask -- haq hash "$RECEIPT"); then
  echo "ERROR: campaign receipt hash failed" >&2
  exit 1
fi

# M17.5 F-69: the clock was rewritten whatever the wrapped command did, so a
# lane that aborted after 52 seconds replaced a completed campaign's 4,623
# with its own. The receipt above records the failed run either way — that it
# happened is evidence — but the clock is the record of the campaign the packet
# would be qualified against, and an abort is not one. A failed run leaves the
# last completed campaign's clock where it is.
if [ "$CODE" -ne 0 ]; then
  echo "campaign run $RUN_ID failed (exit $CODE); clock left at the last completed campaign" >&2
  exit "$CODE"
fi

mkdir -p "$(dirname "$OUT")"
printf '{"schema_version":"haqp-campaign-clock-v1","reference_machine":"%s","runs":[{"id":"%s","commit":"%s","tree":"%s","command":"%s","started_epoch":%s,"finished_epoch":%s,"elapsed_s":%s,"clean":%s,"result":"%s","wrapper":"%s","wrapper_sha256":"%s","receipt":"%s","receipt_blake3":"%s"}]}\n' \
  "$(hostname -s)" "$RUN_ID" "$COMMIT" "$TREE" "$COMMAND" "$STARTED" "$FINISHED" "$ELAPSED" "$CLEAN" "$([ "$CODE" -eq 0 ] && echo pass || echo fail)" "$WRAPPER" "$WRAPPER_SHA256" "$RECEIPT" "$RECEIPT_BLAKE3" >"$OUT"
exit "$CODE"
