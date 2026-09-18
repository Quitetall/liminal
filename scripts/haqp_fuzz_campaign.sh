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
# The resolver writes one transient lexical/canonical pair file per target.
# Keep this scratch directory explicit: a clean checkout may have no prior
# target/haqp output, and failing to create it turns a clean fuzz run into a
# harness failure during evidence materialization.
mkdir -p target/haqp
SANITIZER="${SANITIZER:-address}"
# Opt-in: committing every libFuzzer log bloats the repo permanently, and the
# digest is enough unless you actually want to read them.
KEEP_LOGS="${KEEP_LOGS:-0}"
AUDIT_ACCESS="${AUDIT_ACCESS:-1}"
AUDIT_DIR="conformance/haqp/evidence/access"
mkdir -p "$AUDIT_DIR"
mkdir -p conformance/haqp/evidence/binaries conformance/haqp/evidence/probes
mkdir -p conformance/haqp/evidence/build-logs

hash_file() {
  local path="$1"
  local digest
  if ! digest=$(cargo run -q -p liminal-xtask -- haq hash "$path"); then
    echo "hashing evidence file failed: $path" >&2
    exit 1
  fi
  printf '%s' "$digest"
}

# Hash lexical/canonical pairs for fuzz corpus opens. Keep canonical paths
# repository-relative and never print a forbidden target: a symlink alias into
# held-out data must fail without leaking its pathname into retained evidence.
# F-43: copy one pid's lines out of the lane's stage trace. The stage wrapper's
# `strace -f` holds the one ptrace slot this process tree may have -- a second
# strace fails with "PTRACE_TRACEME: Operation not permitted" and would leave
# this audit empty -- so under the lane the binary runs untraced by us and its
# lines are carved from the stage trace once its exit line is present. strace
# writes each line as it completes and the exit line last, but reports the exit
# after the parent has reaped the child, so the wait is bounded, not assumed.
# Store a raw trace compressed, losslessly, small enough to push.
#
# `--long=27` sets a 128 MiB match window -- the largest a default zstd
# decoder accepts. `zstd::decode_all` in read_evidence_bytes uses that default,
# and `--long=31` (2 GiB window, 3.5 MiB output here) is refused outright:
# "Window size larger than maximum : 2147483648 > 134217728". Measured on the
# 2026-09-18 graph_interchange_codec trace: 5.43 GB raw, 795 MiB at `-3`,
# 26.3 MiB here, decompressing to the same bytes (sha256 verified), so
# trace_blake3 is unchanged by the storage format.
#
# The ceiling is checked rather than assumed. `-3` silently stopped clearing
# GitHub's limit as campaigns grew, and nothing failed until a push was
# refused -- by which point the blob was already in history.
TRACE_STORE_CEILING_BYTES=94371840 # 90 MiB, under GitHub's 100 MB refusal
store_trace() {
  local raw="$1" stored="$2" size
  if ! zstd -q -15 --long=27 --rm -f "$raw" -o "$stored"; then
    echo "ERROR: compressing trace failed: $raw" >&2
    exit 1
  fi
  size=$(stat -c '%s' "$stored")
  if [ "$size" -gt "$TRACE_STORE_CEILING_BYTES" ]; then
    echo "ERROR: stored trace $stored is $size bytes, over the ${TRACE_STORE_CEILING_BYTES}-byte ceiling." >&2
    echo "A larger window is not available: 128 MiB is the default decoder limit." >&2
    echo "Reduce what is traced; do not reduce the trace -- the fd map needs every line." >&2
    exit 1
  fi
}

carve_stage_trace() {
  # 60 s: libFuzzer has already exited (we waited on it); this only covers the
  # tracer reporting that exit after the parent reaped the child, which is
  # milliseconds. A missing exit line after a minute is the tracer's failure.
  local stage_trace="$1" pid="$2" out="$3" waited=0
  until grep -q -E "^${pid} \+\+\+ (exited with [0-9]+|killed by [A-Z0-9]+( \(core dumped\))?) \+\+\+$" "$stage_trace" 2>/dev/null; do
    [ "$waited" -ge 60 ] && return 1
    sleep 1
    waited=$((waited + 1))
  done
  grep -E "^${pid} " "$stage_trace" >"$out"
  [ -s "$out" ]
}

resolve_corpus_paths() {
  local raw_trace="$1"
  local target="$2"
  local resolved_input="target/haqp/resolved-$target.input"
  local raw canonical relative
  : > "$resolved_input"
  while IFS= read -r raw; do
    [ -n "$raw" ] || continue
    case "$raw" in
      *"/fuzz/corpus/$target"*) ;;
      *) continue ;;
    esac
    if [[ "$raw" = /* ]]; then
      case "$raw" in
        "$trace_root"/*) relative="${raw#"$trace_root"/}" ;;
        */fuzz/*) relative="fuzz/${raw#*/fuzz/}" ;;
        *) return 1 ;;
      esac
    else
      relative="$raw"
    fi
    # Corpus entries can be deleted by libFuzzer after they were opened. Keep
    # canonicalizing existing symlink prefixes while allowing missing leaves;
    # the trace_root case check below remains the root-boundary guard.
    canonical=$(realpath -m -- "$trace_root/$relative" 2>/dev/null) || return 1
    case "$canonical" in
      "$trace_root"/*) relative="${canonical#"$trace_root"/}" ;;
      *) return 1 ;;
    esac
    case "${relative,,}" in
      *heldout*|*conformance/corpora*) return 1 ;;
    esac
    printf '%s\0%s\n' "$raw" "$relative" >> "$resolved_input"
  done < <(sed -n -E 's/.*(open|openat|openat2|creat|stat|statx|lstat|fstatat|newfstatat|readlink|readlinkat|access|faccessat|faccessat2|execve|execveat|name_to_handle_at|truncate|utimensat|unlink|unlinkat|rename|renameat|mkdir|chdir)\([^,]*,? "(([^"\\]|\\.)*)".*/\2/p' "$raw_trace" | sort -u)
  [ -s "$resolved_input" ] || return 1
  hash_file "$resolved_input"
}
audit_entries=""
audit_tracer="strace-open-paths"
source_commit=$(git rev-parse HEAD)
source_tree=$(git rev-parse HEAD^{tree})
trace_root=$(pwd -P)
if command -v strace >/dev/null 2>&1; then
  mkdir -p conformance/haqp/evidence/access
  strace -V >"conformance/haqp/evidence/access/strace.version" 2>&1
  tracer_version_blake3=$(hash_file conformance/haqp/evidence/access/strace.version)
else
  echo "ERROR: strace is required for HAQP corpus-access evidence" >&2
  exit 1
fi

# LeakSanitizer aborts under ptrace even when target code is clean. Keep ASan
# memory checks enabled while disabling only leak detection for traced runs;
# otherwise tracer itself manufactures a false fuzz failure and empty artifact.
asan_options="${ASAN_OPTIONS:-}"
if [ "$SANITIZER" = "address" ] && [[ "$asan_options" != *detect_leaks=* ]]; then
  asan_options="${asan_options:+$asan_options:}detect_leaks=0"
fi

echo "[" > "$OUT"
first=1
overall=0

# Sanitizer binaries are built at a CANONICAL absolute path, not in-tree
# (M17.5 F-42). cargo derives each crate's `-C metadata` hash from the package
# id, which for a path dependency includes the manifest's ABSOLUTE path; ASan
# then embeds the codegen-unit name (`<target>.<hash>-cgu.0`) in .rodata. So the
# same commit built at two paths yields two binaries, and the replay -- which
# rebuilds in a scratch worktree -- could never match the committed digest.
# Measured: source-path remapping and trim-paths do not touch it; two clean
# builds at one fixed path are byte-identical. Campaign and replay therefore
# both build at exactly this path, keyed by commit so concurrent commits cannot
# clobber each other. /var/tmp is persistent disk; older commits' trees are
# pruned so the directory cannot grow without bound.
BUILD_ROOT_BASE=/var/tmp/liminal-haqp-build
BUILD_ROOT="$BUILD_ROOT_BASE/$source_commit"
BUILD_RUSTFLAGS="-Zremap-cwd-prefix=/liminal --remap-path-prefix=$BUILD_ROOT=/liminal --remap-path-prefix=$HOME/.cargo=/cargo"
mkdir -p "$BUILD_ROOT_BASE"
# Pruning sibling commits' trees is destructive, and a replay (`haq-verify`)
# may be mid-build in one of them -- it holds a SHARED lock on .lock for the
# duration. Take the EXCLUSIVE lock here so pruning waits for any replay to
# finish, and a replay started later waits for pruning (HEAD review).
exec 9>"$BUILD_ROOT_BASE/.lock"
flock -x 9
for stale in "$BUILD_ROOT_BASE"/*/; do
  [ "${stale%/}" = "$BUILD_ROOT" ] && continue
  git worktree remove --force "${stale%/}" 2>/dev/null || rm -rf "${stale%/}"
done
git worktree prune
if [ ! -d "$BUILD_ROOT/.git" ] && [ ! -f "$BUILD_ROOT/.git" ]; then
  rm -rf "$BUILD_ROOT"
  # F-44: the build root never materializes the locked corpus. The sanitizer
  # build does not reference it, and a tree without it has nothing of the
  # locked data to write, prune or alias. The verifier's replay adds its
  # worktree the same way (add_sparse_worktree), so both build the same tree.
  git worktree add --quiet --detach --no-checkout "$BUILD_ROOT" "$source_commit"
  git -C "$BUILD_ROOT" sparse-checkout set --no-cone '/*' '!/conformance/corpora/'
  git -C "$BUILD_ROOT" checkout --quiet --detach "$source_commit"
  if [ -e "$BUILD_ROOT/conformance/corpora" ]; then
    echo "ERROR: sparse worktree still materialized conformance/corpora" >&2
    exit 1
  fi
fi
flock -u 9
mkdir -p "$BUILD_ROOT/fuzz/.cargo"
printf '[unstable]\ntrim-paths = true\n\n[profile.release]\ntrim-paths = "all"\n' > "$BUILD_ROOT/fuzz/.cargo/config.toml"

for t in "${TARGETS[@]}"; do
  echo "=== building $t (${SANITIZER}) at $BUILD_ROOT ==="
  log="target/haqp/fuzz-$t.log"
  build_log="target/haqp/build-$t.log"
  build_log_evidence="conformance/haqp/evidence/build-logs/$t.log"
  build_command="cargo +nightly fuzz build -s $SANITIZER $t"
  # Pin the target directory to the one the binary lookup below reads.
  # `build.target-dir` in $CARGO_HOME/config.toml is outside this repository
  # and redirected the build out of $BUILD_ROOT on 2026-09-18. The value is
  # cargo-fuzz's own default, so the binaries and their digests are unchanged.
  (cd "$BUILD_ROOT" && CARGO_TARGET_DIR="$BUILD_ROOT/fuzz/target" \
    RUSTFLAGS="$BUILD_RUSTFLAGS" $build_command) >"$build_log" 2>&1
  build_code=$?
  cp "$build_log" "$build_log_evidence"
  binary_candidate=$(find "$BUILD_ROOT/fuzz/target" -type f -perm -111 -name "$t" -print -quit)
  binary="conformance/haqp/evidence/binaries/$t"
  probe="conformance/haqp/evidence/probes/$t.log"
  probe_code=1
  if [ "$build_code" -eq 0 ] && [ -n "$binary_candidate" ]; then
    cp "$binary_candidate" "$binary"
    # The runtime probe asks the SANITIZER RUNTIME to identify itself (F-42):
    # `-help=1` alone prints libFuzzer's usage banner, which never names the
    # sanitizer, so the old probe recorded nothing the verifier could check.
    # This is exactly the probe verify_sanitizer_build_replay re-runs.
    case "$SANITIZER" in
      address) san_env=ASAN_OPTIONS ;; memory) san_env=MSAN_OPTIONS ;;
      thread)  san_env=TSAN_OPTIONS ;; leak)   san_env=LSAN_OPTIONS ;;
      *) echo "ERROR: no runtime witness for sanitizer $SANITIZER" >&2; exit 1 ;;
    esac
    env "$san_env=help=1:detect_leaks=0" "$binary_candidate" -runs=0 -seed=1 >"$probe" 2>&1
    probe_code=$?
  else
    echo "build failed: code=$build_code binary=${binary_candidate:-missing}" >"$probe"
  fi
  binary_hash=$(hash_file "$binary")
  probe_hash=$(hash_file "$probe")
  build_log_hash=$(hash_file "$build_log_evidence")
  # -s is explicit rather than relying on cargo-fuzz's default: ADR-0020 §4
  # requires a SANITIZER-ENABLED campaign, and a requirement satisfied by a
  printf '%s\0%s\0%s\0%s\0%s\0%s\0' \
    "$build_code" "$probe_code" "$binary_hash" "$probe_hash" "$build_log_hash" "$build_command" \
    >"target/haqp/buildmeta-$t"
done

# ── fuzz runs, in parallel ────────────────────────────────────────────────
#
# Each libFuzzer instance is single-threaded and its targets are independent:
# separate corpus, artifacts, log and strace file. Running them one at a time
# left 13 of this machine's 14 cores idle for three and a half hours.
#
# What does NOT change: every target still gets its own full `-max_total_time`
# budget on its own core, so ADR-0020 §4's floor (30 min per family, 150
# target-minutes) is satisfied exactly as before. What DOES change is
# executions per target -- concurrent ASan runs share memory bandwidth and pull
# all-core clocks down. §4's requirement is denominated in time, but exec depth
# is the substance of the evidence, so the default is deliberately conservative
# rather than "all seven at once".
#
# Builds above are serial on purpose: they share one `fuzz/target`, so cargo
# would serialize them on its package lock regardless.
# One core per target where the machine has them, so the seven 30-minute
# budgets overlap and the stage costs ~31 minutes instead of ~70. Capped at the
# core count rather than fixed at 7 (review of `7e4ba39`): on a 2-4 core CI
# runner, seven concurrent ASan fuzzers contend instead of overlapping.
# ADR-0020 §4 is denominated in TIME per family, which is unchanged; what
# concurrency costs is exec DEPTH, and the recorded execs make any loss visible.
default_jobs=$(nproc 2>/dev/null || echo 4)
[ "$default_jobs" -gt 7 ] && default_jobs=7
JOBS="${HAQP_FUZZ_JOBS:-$default_jobs}"
echo "=== fuzzing ${#TARGETS[@]} targets for ${SECS}s each, ${JOBS} at a time ==="
running=0
for t in "${TARGETS[@]}"; do
  (
    started=$(date +%s)
    log="target/haqp/fuzz-$t.log"
    audit_raw="$AUDIT_DIR/$t.trace"
    # The BUILT BINARY is run directly rather than through `cargo fuzz run`.
    #
    # cargo rebuilds and takes a global package-cache/build-directory lock, so
    # four concurrent `cargo fuzz run` invocations serialize on it: measured,
    # one target spent 220 of its 228 seconds printing "Blocking waiting for
    # file lock" against a FIVE second budget, and that wait landed inside
    # elapsed_s. Running the binary is what cargo fuzz run does anyway once the
    # build phase above is done -- 6s instead of 228s, and 90k executions
    # instead of 62k on the same budget.
    #
    # Two things cargo fuzz run supplied that must now be supplied explicitly,
    # or the evidence quietly changes meaning:
    #   -artifact_prefix, or a crash lands in the cwd and `arts` counts zero;
    #   the echoed command line, which verify_fuzz_log_metrics binds the log to
    #   via `-max_total_time=` and `-seed=`. libFuzzer prints "INFO: Seed:" but
    #   never its own flags, so the campaign records the invocation itself. The
    #   line is true by construction: it is the command being run on the line
    #   below it.
    # The evidence copy IS the canonical build; there is no in-tree binary now.
    binary_run="conformance/haqp/evidence/binaries/$t"
    mkdir -p "fuzz/artifacts/$t"
    # ABSOLUTE, as cargo fuzz run passed it. resolve_corpus_paths filters
    # traced paths on "/fuzz/corpus/$target" -- with a leading slash -- so a
    # relative corpus argument makes every one of the ~20k traced opens fail
    # the filter and the resolver return empty. trace_root is recorded in the
    # audit row and stripped back off, so absolute here is the shape the
    # corpus-access evidence already expects.
    fuzz_argv=(
      "$trace_root/fuzz/corpus/$t"
      -artifact_prefix="fuzz/artifacts/$t/"
      -max_total_time="$SECS" -seed="$SEED" -rss_limit_mb=4096 -print_final_stats=1
    )
    printf 'Running: %s %s\n' "$binary_run" "${fuzz_argv[*]}" >"$log"
    if [ -n "${HAQP_STAGE_TRACE:-}" ]; then
      # F-43: under the lane the stage tracer is the tracer; see carve_stage_trace.
      ASAN_OPTIONS="$asan_options" "$binary_run" "${fuzz_argv[@]}" >>"$log" 2>&1 &
      fuzz_pid=$!
      wait "$fuzz_pid"
      code=$?
      if carve_stage_trace "$HAQP_STAGE_TRACE" "$fuzz_pid" "$audit_raw"; then
        tracer="strace-open-paths"
      else
        echo "corpus access audit unavailable: pid $fuzz_pid has no exit line in stage trace $HAQP_STAGE_TRACE" >"$audit_raw"
        tracer="unavailable"
      fi
    elif [ "$AUDIT_ACCESS" = "1" ] && command -v strace >/dev/null 2>&1; then
      ASAN_OPTIONS="$asan_options" strace -f -q -e trace=%file -o "$audit_raw" \
        "$binary_run" "${fuzz_argv[@]}" >>"$log" 2>&1 &
      fuzz_pid=$!
      wait "$fuzz_pid"
      code=$?
      tracer="strace-open-paths"
    else
      echo "corpus access audit unavailable: AUDIT_ACCESS=$AUDIT_ACCESS strace=$(command -v strace || echo missing)" >"$audit_raw"
      ASAN_OPTIONS="$asan_options" "$binary_run" "${fuzz_argv[@]}" >>"$log" 2>&1
      code=$?
      fuzz_pid=0
      tracer="unavailable"
    fi
    # elapsed now measures ONLY the fuzz run. It used to start before
    # `cargo fuzz build`, so a cold cache pushed elapsed_s past
    # verify_fuzz_log_metrics' `done_elapsed + 60` tolerance and failed the
    # campaign after 3.5 hours. Warm builds hid it: the committed evidence
    # shows 2-3s of slack against a 60s allowance.
    printf '%s\0%s\0%s\0%s\0' \
      "$code" "$(( $(date +%s) - started ))" "$fuzz_pid" "$tracer" \
      >"target/haqp/runmeta-$t"
    echo "--- $t finished: exit=$code"
  ) &
  running=$((running + 1))
  if [ "$running" -ge "$JOBS" ]; then
    wait -n
    running=$((running - 1))
  fi
done
wait

for t in "${TARGETS[@]}"; do
  # Serial and in TARGETS order: the JSON rows below are appended with manual
  # comma handling, and the evidence must not depend on which target finished
  # first.
  log="target/haqp/fuzz-$t.log"
  build_log_evidence="conformance/haqp/evidence/build-logs/$t.log"
  binary="conformance/haqp/evidence/binaries/$t"
  probe="conformance/haqp/evidence/probes/$t.log"
  audit_raw="$AUDIT_DIR/$t.trace"
  mapfile -d '' -t bm <"target/haqp/buildmeta-$t"
  build_code="${bm[0]}"; probe_code="${bm[1]}"; binary_hash="${bm[2]}"
  probe_hash="${bm[3]}"; build_log_hash="${bm[4]}"; build_command="${bm[5]}"
  mapfile -d '' -t rm <"target/haqp/runmeta-$t"
  code="${rm[0]}"; elapsed="${rm[1]}"; fuzz_pid="${rm[2]}"; audit_tracer="${rm[3]}"
  [ "$audit_tracer" = "unavailable" ] && overall=1
  trace_pid=${fuzz_pid:-0}
  if [ -s "$audit_raw" ]; then
    # Capture tracer identity even on failing runs; verifier then rejects the
    # nonzero trace_exit_code without losing which process emitted the trace.
    traced_pid=$(awk '/\+\+\+ exited with [0-9]+ \+\+\+$/ { pid=$1 } END { print pid + 0 }' "$audit_raw")
    if [ "${traced_pid:-0}" -gt 0 ]; then
      trace_pid=$traced_pid
    fi
  fi
  trace_exit_code=$code
  trace_complete=false
  if [ -s "$audit_raw" ] && grep -q '+++ exited with 0 +++' "$audit_raw"; then
    trace_complete=true
  fi
  # A 30-minute ASan run emits gigabytes of strace output -- 1.56 GB for
  # graph_interchange_codec on 2026-09-02, which GitHub refuses outright at its
  # 100 MB ceiling. zstd takes that to ~55 MB. The DIGEST stays over the raw
  # bytes so trace_blake3 keeps meaning "the trace this campaign produced",
  # unchanged by the storage format (read_evidence_bytes decompresses to check).
  #
  # F-76: `-3` stopped clearing that ceiling. The 2026-09-18 campaign traced
  # 5.43 GB for this target and stored 795 MiB, and the push was refused. The
  # content is extremely repetitive -- 5.43 GB holds 15.6 MiB of distinct lines
  # -- so a larger match window, not a lossy reduction, is the fix. See
  # store_trace below. Reducing the trace to distinct lines would shrink it
  # further and is NOT safe: scan_scope_trace is stateful, mapping (pid, fd) to
  # a path, so a repeated `openat(7, "x", O_WRONLY)` line after fd 7 is reopened
  # on another directory is a different access wearing identical text. Dropping
  # it as a duplicate would hide exactly the locked-corpus write that blind
  # pass 1 at f360e90 (A07) added the fd map to catch.
  trace_hash=$(hash_file "$audit_raw")
  arts=$(ls "fuzz/artifacts/$t" 2>/dev/null | wc -l)
  execs=$(grep -oP 'stat::number_of_executed_units:\s*\K[0-9]+' "$log" | tail -1)
  execs=${execs:-0}
  [ "$code" -ne 0 ] && overall=1
  [ "$first" -eq 0 ] && echo "," >> "$OUT"
  first=0
  # F-17: release qualification retains each log in tracked evidence. A digest
  # alone cannot prove that later counts came from the campaign's output.
  loghash=$(hash_file "$log")
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
  audit_hash=$(hash_file "$audit_manifest")
  resolved_paths_blake3=$(resolve_corpus_paths "$audit_raw" "$t") || {
    echo "ERROR: traced corpus path resolution failed for $t" >&2
    exit 1
  }
  # Compressed only AFTER the last reader: the paths manifest and
  # resolve_corpus_paths both consume the raw trace, and an earlier `--rm` left
  # them reading a file that no longer existed.
  if ! command -v zstd >/dev/null 2>&1; then
    echo "ERROR: zstd is required to store corpus-access traces" >&2
    exit 1
  fi
  store_trace "$audit_raw" "$audit_raw.zst"
  seed_manifest="target/haqp/seed-manifest-$t.bin"
  : > "$seed_manifest"
  seed_count=0
  # TRACKED seeds only, matching the verifier (M17.5 F-32).
  mapfile -t seed_paths < <(git ls-files -z -- "fuzz/corpus/$t" | tr '\0' '\n' | LC_ALL=C sort)
  for seed in "${seed_paths[@]}"; do
    # Match verifier seed_manifest_digest exactly: lexical filename, NUL,
    # SHA-256 hex, NUL. Human-readable separators would bind a different
    # byte stream and make an otherwise valid campaign fail closed.
    # Absolute: under the lane the campaign's own reads are in the stage trace,
    # and a relative fuzz/corpus path there is unresolvable evidence (F-44).
    printf '%s\0%s\0' "${seed#fuzz/corpus/$t/}" "$(sha256sum "$trace_root/$seed" | cut -d' ' -f1)" >> "$seed_manifest"
    seed_count=$((seed_count + 1))
  done
  seed_manifest_blake3=$(hash_file "$seed_manifest")
  trace_prefix=""
  if [ -n "$asan_options" ]; then
    printf -v trace_prefix 'ASAN_OPTIONS=%q ' "$asan_options"
  fi
  # Must describe what actually ran: the build, then the traced BINARY.
  binary_run="conformance/haqp/evidence/binaries/$t"
  if [ -n "${HAQP_STAGE_TRACE:-}" ]; then
    # F-43: what ran is the binary under the lane's stage tracer, carved by pid.
    trace_command="$build_command && ${trace_prefix}$binary_run $trace_root/fuzz/corpus/$t -artifact_prefix=fuzz/artifacts/$t/ -max_total_time=$SECS -seed=$SEED -rss_limit_mb=4096 -print_final_stats=1 (traced by the stage tracer: strace -f -q -e trace=%file -o $HAQP_STAGE_TRACE; pid $trace_pid carved to $audit_raw)"
  else
    trace_command="$build_command && ${trace_prefix}strace -f -q -e trace=%file -o $audit_raw $binary_run $trace_root/fuzz/corpus/$t -artifact_prefix=fuzz/artifacts/$t/ -max_total_time=$SECS -seed=$SEED -rss_limit_mb=4096 -print_final_stats=1"
  fi
  binding_input="target/haqp/process-binding-$t.txt"
  printf '%s\0%s\0%s\0%s' "$trace_command" "$trace_pid" "$trace_exit_code" "$trace_hash" >"$binding_input"
  process_binding=$(hash_file "$binding_input")
  audit_row=$(printf '{"target":"%s","manifest":"%s","manifest_blake3":"%s","seed":%s,"sanitizer":"%s","exit_code":%s,"log_blake3":"%s","command":"%s","trace":"%s","trace_blake3":"%s","trace_pid":%s,"trace_exit_code":%s,"trace_complete":%s,"process_binding":"%s","tracer_binary":"strace","tracer_version":"conformance/haqp/evidence/access/strace.version","tracer_version_blake3":"%s","trace_root":"%s","resolved_paths_blake3":"%s"}' \
    "$t" "conformance/haqp/evidence/access/$t.paths" "$audit_hash" "$SEED" "$SANITIZER" "$code" "$loghash" "$trace_command" "conformance/haqp/evidence/access/$t.trace.zst" "$trace_hash" "$trace_pid" "$trace_exit_code" "$trace_complete" "$process_binding" "$tracer_version_blake3" "$trace_root" "$resolved_paths_blake3")
  if [ -n "$audit_entries" ]; then audit_entries="$audit_entries,$audit_row"; else audit_entries="$audit_row"; fi
  printf '{"target":"%s","seconds":%s,"elapsed_s":%s,"exit_code":%s,"execs":%s,"artifacts":%s,"seed":%s,"seed_count":%s,"seed_manifest_blake3":"%s","sanitizer":"%s","log":"%s","log_blake3":"%s","sanitizer_proof":{"build_command":"%s","build_root":"%s","build_rustflags":"%s","binary":"%s","binary_blake3":"%s","runtime_probe":"%s","runtime_probe_blake3":"%s","runtime_probe_exit_code":%s,"instrumentation_flags":["-fsanitize=%s"],"build_log":"%s","build_log_blake3":"%s","source_commit":"%s","source_tree":"%s","build_result":"%s"}}' \
    "$t" "$SECS" "$elapsed" "$code" "$execs" "$arts" "$SEED" "$seed_count" "$seed_manifest_blake3" "$SANITIZER" "$evidence_log" "$loghash" "$build_command" "$BUILD_ROOT" "$BUILD_RUSTFLAGS" "$binary" "$binary_hash" "$probe" "$probe_hash" "$probe_code" "$SANITIZER" "$build_log_evidence" "$build_log_hash" "$source_commit" "$source_tree" "$([ "$build_code" -eq 0 ] && echo pass || echo fail)" >> "$OUT"
  echo "--- $t: exit=$code execs=$execs artifacts=$arts elapsed=${elapsed}s"
done

echo "]" >> "$OUT"
mkdir -p "$(dirname "$AUDIT_OUT")"
printf '{"schema_version":"haqp-corpus-access-v2","tracer":"%s","source_commit":"%s","source_tree":"%s","targets":[%s]}\n' "$audit_tracer" "$source_commit" "$source_tree" "$audit_entries" >"$AUDIT_OUT"
echo "campaign complete; overall_exit=$overall; evidence=$OUT"
exit "$overall"
