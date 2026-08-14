#!/usr/bin/env bash
# HAQP-1 §4 fuzz lane: one sanitizer-enabled campaign per frozen target.
#
# Every crash, panic, timeout, OOM, or divergence is a FAILING result — the
# script records a nonzero exit per target and never swallows it. Minimized
# artifacts land in fuzz/artifacts/<target>/ and must become named regression
# fixtures before the qualification lane is rerun.
#
# Usage: scripts/haqp_fuzz_campaign.sh [seconds-per-target] [out.json]
set -uo pipefail

SECS="${1:-1860}"
OUT="${2:-target/haqp/fuzz.json}"
AUDIT_OUT="${HAQP_CORPUS_AUDIT_OUT:-conformance/haqp/evidence/corpus-access.json}"
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
AUDIT_ACCESS="${AUDIT_ACCESS:-1}"
AUDIT_DIR="conformance/haqp/evidence/access"
mkdir -p "$AUDIT_DIR"
audit_entries=""
audit_tracer="strace-open-paths"

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
  audit_raw="target/haqp/access-$t.strace"
  if [ "$AUDIT_ACCESS" = "1" ] && command -v strace >/dev/null 2>&1; then
    strace -f -qq -e trace=openat,openat2 -o "$audit_raw" \
      cargo +nightly fuzz run -s "$SANITIZER" "$t" -- \
        -max_total_time="$SECS" -seed="$SEED" -rss_limit_mb=4096 -print_final_stats=1 \
        >"$log" 2>&1
    code=$?
  else
    echo "corpus access audit unavailable: AUDIT_ACCESS=$AUDIT_ACCESS strace=$(command -v strace || echo missing)" >"$audit_raw"
    overall=1
    cargo +nightly fuzz run -s "$SANITIZER" "$t" -- \
      -max_total_time="$SECS" -seed="$SEED" -rss_limit_mb=4096 -print_final_stats=1 \
      >"$log" 2>&1
    code=$?
    audit_tracer="unavailable"
  fi
  elapsed=$(( $(date +%s) - started ))
  arts=$(ls "fuzz/artifacts/$t" 2>/dev/null | wc -l)
  execs=$(grep -oP 'stat::number_of_executed_units:\s*\K[0-9]+' "$log" | tail -1)
  execs=${execs:-0}
  [ "$code" -ne 0 ] && overall=1
  [ "$first" -eq 0 ] && echo "," >> "$OUT"
  first=0
  # F-17: release qualification retains each log in tracked evidence. A digest
  # alone cannot prove that later counts came from the campaign's output.
  loghash=$(cargo run -q -p liminal-xtask -- haq hash "$log" 2>/dev/null || echo "")
  if [ "$KEEP_LOGS" = "1" ]; then
    mkdir -p conformance/haqp/evidence/logs
    cp "$log" "conformance/haqp/evidence/logs/$t.log"
    evidence_log="conformance/haqp/evidence/logs/$t.log"
  else
    evidence_log="$log"
  fi
  audit_manifest="$AUDIT_DIR/$t.paths"
  if [ "$audit_tracer" = "strace-open-paths" ]; then
    sed -n -E 's/.*openat2?\([^,]+, "(([^"\\]|\\.)*)".*/\1/p' "$audit_raw" | sort -u >"$audit_manifest"
  else
    cp "$audit_raw" "$audit_manifest"
  fi
  audit_hash=$(cargo run -q -p liminal-xtask -- haq hash "$audit_manifest" 2>/dev/null || echo "")
  seed_manifest="target/haqp/seed-manifest-$t.txt"
  : > "$seed_manifest"
  seed_count=0
  for seed in "fuzz/corpus/$t"/*; do
    [ -f "$seed" ] || continue
    printf '%s %s\n' "${seed#fuzz/corpus/$t/}" "$(sha256sum "$seed" | cut -d' ' -f1)" >> "$seed_manifest"
    seed_count=$((seed_count + 1))
  done
  seed_manifest_blake3=$(cargo run -q -p liminal-xtask -- haq hash "$seed_manifest" 2>/dev/null || echo "")
  trace_command="cargo +nightly fuzz run -s $SANITIZER $t -- -max_total_time=$SECS -seed=$SEED -rss_limit_mb=4096 -print_final_stats=1"
  audit_row=$(printf '{"target":"%s","manifest":"%s","manifest_blake3":"%s","seed":%s,"sanitizer":"%s","exit_code":%s,"log_blake3":"%s","command":"%s"}' \
    "$t" "conformance/haqp/evidence/access/$t.paths" "$audit_hash" "$SEED" "$SANITIZER" "$code" "$loghash" "$trace_command")
  if [ -n "$audit_entries" ]; then audit_entries="$audit_entries,$audit_row"; else audit_entries="$audit_row"; fi
  printf '{"target":"%s","seconds":%s,"elapsed_s":%s,"exit_code":%s,"execs":%s,"artifacts":%s,"seed":%s,"seed_count":%s,"seed_manifest_blake3":"%s","sanitizer":"%s","log":"%s","log_blake3":"%s"}' \
    "$t" "$SECS" "$elapsed" "$code" "$execs" "$arts" "$SEED" "$seed_count" "$seed_manifest_blake3" "$SANITIZER" "$evidence_log" "$loghash" >> "$OUT"
  echo "--- $t: exit=$code execs=$execs artifacts=$arts elapsed=${elapsed}s"
done

echo "]" >> "$OUT"
mkdir -p "$(dirname "$AUDIT_OUT")"
printf '{"schema_version":"haqp-corpus-access-v1","tracer":"%s","targets":[%s]}\n' "$audit_tracer" "$audit_entries" >"$AUDIT_OUT"
echo "campaign complete; overall_exit=$overall; evidence=$OUT"
exit "$overall"
