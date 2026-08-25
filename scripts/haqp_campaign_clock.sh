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

mkdir -p "$(dirname "$OUT")"
printf '{"schema_version":"haqp-campaign-clock-v1","reference_machine":"%s","runs":[{"id":"%s","commit":"%s","tree":"%s","command":"%s","started_epoch":%s,"finished_epoch":%s,"elapsed_s":%s,"clean":%s,"result":"%s","wrapper":"%s","wrapper_sha256":"%s","receipt":"%s","receipt_blake3":"%s"}]}\n' \
  "$(hostname -s)" "$RUN_ID" "$COMMIT" "$TREE" "$COMMAND" "$STARTED" "$FINISHED" "$ELAPSED" "$CLEAN" "$([ "$CODE" -eq 0 ] && echo pass || echo fail)" "$WRAPPER" "$WRAPPER_SHA256" "$RECEIPT" "$RECEIPT_BLAKE3" >"$OUT"
exit "$CODE"
