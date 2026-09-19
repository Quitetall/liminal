#!/usr/bin/env bash
# Refuse any tracked file large enough for a remote to reject the push.
#
# M17.5 F-76: two corpus-access traces reached 198 MiB and 795 MiB and a push
# was refused by GitHub's 100 MB limit. By then the blobs were in history and
# clearing them needed a rewrite. The campaign's own mitigation -- zstd at a
# level chosen against one 2026-09-02 measurement -- had silently stopped
# clearing the ceiling as campaigns grew, because nothing measured it.
#
# This is deliberately about the WORKING TREE's tracked files, which is what a
# later commit would carry. History is not rescanned: a blob already committed
# is a rewrite, not a refusal, and this check exists to stop that happening.
set -uo pipefail

CEILING_BYTES="${TRACKED_FILE_CEILING_BYTES:-94371840}" # 90 MiB, under GitHub's 100 MB
cd "$(git rev-parse --show-toplevel)" || exit 1

status=0
while IFS= read -r -d '' path; do
  [ -f "$path" ] || continue
  size=$(stat -c '%s' "$path" 2>/dev/null) || continue
  if [ "$size" -gt "$CEILING_BYTES" ]; then
    printf 'ERROR: tracked file %s is %s bytes, over the %s-byte ceiling.\n' \
      "$path" "$size" "$CEILING_BYTES" >&2
    status=1
  fi
done < <(git ls-files -z)

if [ "$status" -ne 0 ]; then
  cat >&2 <<'MSG'
A remote will refuse this. Do not commit it and delete it afterwards -- the blob
stays in history and only a rewrite removes it.
For zstd evidence, store_trace in scripts/haqp_fuzz_campaign.sh and
scripts/haqp_scope_row.py compresses with `--long=27`, the largest match window
a default decoder accepts. Reduce what is traced; do not reduce a trace -- the
fd map in scan_scope_trace needs every line.
MSG
fi
exit "$status"
