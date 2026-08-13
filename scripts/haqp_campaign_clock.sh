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
ELAPSED=$(( $(date +%s) - STARTED ))
CLEAN=1
if [ -n "$(git status --porcelain=v1)" ]; then CLEAN=0; fi

mkdir -p "$(dirname "$OUT")"
printf '{"schema_version":"haqp-campaign-clock-v1","reference_machine":"%s","runs":[{"id":"%s","commit":"%s","tree":"%s","command":"%s","elapsed_s":%s,"clean":%s,"result":"%s"]}\n' \
  "$(hostname -s)" "$RUN_ID" "$COMMIT" "$TREE" "$COMMAND" "$ELAPSED" "$CLEAN" "$([ "$CODE" -eq 0 ] && echo pass || echo fail)" >"$OUT"
exit "$CODE"
