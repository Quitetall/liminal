#!/usr/bin/env bash
# HAQP-1 §4 fuzz lane: one sanitizer-enabled campaign per critical family.
#
# Every crash, panic, timeout, OOM, or divergence is a FAILING result — the
# script records a nonzero exit per target and never swallows it. Minimized
# artifacts land in fuzz/artifacts/<target>/ and must become named regression
# fixtures before the qualification lane is rerun.
#
# Usage: scripts/haqp_fuzz_campaign.sh [seconds-per-target] [out.json]
set -uo pipefail

SECS="${1:-1800}"
OUT="${2:-target/haqp/fuzz.json}"
TARGETS=(
  cst_parse
  format_idempotent
  canonical_round_trip
  incremental_full_equivalence
  html_render
  graph_interchange_codec
  ilrp_recovery
)
SEED=20260725   # fixed so the campaign is reproducible (ADR-0020 §1)

mkdir -p "$(dirname "$OUT")"
SANITIZER="${SANITIZER:-address}"
# Opt-in: committing every libFuzzer log bloats the repo permanently, and the
# digest is enough unless you actually want to read them.
KEEP_LOGS="${KEEP_LOGS:-0}"

echo "[" > "$OUT"
first=1
overall=0

for t in "${TARGETS[@]}"; do
  echo "=== fuzzing $t for ${SECS}s (ASan, seed=$SEED) ==="
  started=$(date +%s)
  log="target/haqp/fuzz-$t.log"
  # -s is explicit rather than relying on cargo-fuzz's default: ADR-0020 §4
  # requires a SANITIZER-ENABLED campaign, and a requirement satisfied by a
  # tool default is one a tool update can silently withdraw.
  cargo +nightly fuzz run -s "$SANITIZER" "$t" -- \
      -max_total_time="$SECS" -seed="$SEED" -rss_limit_mb=4096 -print_final_stats=1 \
      >"$log" 2>&1
  code=$?
  elapsed=$(( $(date +%s) - started ))
  arts=$(ls "fuzz/artifacts/$t" 2>/dev/null | wc -l)
  execs=$(grep -oP 'stat::number_of_executed_units:\s*\K[0-9]+' "$log" | tail -1)
  execs=${execs:-0}
  [ "$code" -ne 0 ] && overall=1
  [ "$first" -eq 0 ] && echo "," >> "$OUT"
  first=0
  # F-17: the log lives under target/ (gitignored), so its digest is what makes
  # the counts above checkable after the run. --keep-logs commits the log itself
  # for anyone who wants more than a hash.
  loghash=$(cargo run -q -p liminal-xtask -- haq hash "$log" 2>/dev/null || echo "")
  if [ "$KEEP_LOGS" = "1" ]; then
    mkdir -p conformance/haqp/evidence/logs
    cp "$log" "conformance/haqp/evidence/logs/$t.log"
  fi
  printf '{"target":"%s","seconds":%s,"elapsed_s":%s,"exit_code":%s,"execs":%s,"artifacts":%s,"seed":%s,"sanitizer":"%s","log":"%s","log_blake3":"%s"}' \
    "$t" "$SECS" "$elapsed" "$code" "$execs" "$arts" "$SEED" "$SANITIZER" "$log" "$loghash" >> "$OUT"
  echo "--- $t: exit=$code execs=$execs artifacts=$arts elapsed=${elapsed}s"
done

echo "]" >> "$OUT"
echo "campaign complete; overall_exit=$overall; evidence=$OUT"
exit "$overall"
