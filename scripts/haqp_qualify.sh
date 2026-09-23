#!/usr/bin/env bash
# Run every HAQP-1a evidence lane at ONE fixed base, then prepare the flip.
#
# ADR-0020 §1 wants one fixed clean tree. That is not a formality here: on
# 2026-08-15 the evidence had accumulated across six different commits, because
# each lane was rerun as the code around it hardened. Every artifact was
# individually honest and the set described no tree at all.
#
# So this script pins the base once, refuses to start if the tree is dirty, and
# runs every lane against it. It does NOT commit — the metadata-only child is
# the human's to make, because that commit is what §1 binds the qualification
# to.
#
# Wall clock is dominated by the fuzz campaign: 7 targets x 30 minutes = 3.5h.
# Everything else is minutes.
set -euo pipefail

# Kill the whole process group on interrupt. Killing this script alone leaves
# `strace` and `cargo-fuzz` children writing into committed evidence paths after
# the operator thinks the run has stopped — that is how the 2026-08-16 kill left
# six trace files dirty and `fuzz.json` as invalid JSON.
trap 'echo "haqp: interrupted, stopping campaign children" >&2; kill 0' INT TERM

cd "$(dirname "$0")/.."

# F-86: build into THIS checkout's own target directory, never a shared one.
# `build.target-dir` in $CARGO_HOME/config.toml points every checkout on this
# machine at /mnt/2tb/cargo-target. Cargo keeps hashed artifacts apart by
# package path, but it UPLIFTS final binaries to fixed names -- debug/lim,
# debug/lim-toy -- which every checkout shares. A build anywhere else could
# therefore replace the binary this lane is testing while it runs, and the
# campaign would qualify code that is not its fixed base. The 2026-09-23 lane
# ran while this very checkout's main tree was being built beside it.
export CARGO_TARGET_DIR="$PWD/target"

if [ -n "$(git status --porcelain)" ]; then
  echo "refusing to start: the tree is dirty; the fixed base must be clean (ADR-0020 §1)" >&2
  git status --short | sed 's/^/  /' >&2
  # A lane that died mid-way leaves its own outputs behind (receipt, row
  # files, campaign evidence). They are not restored here -- discarding
  # uncommitted evidence is the operator's call -- but the call is spelled out.
  # Fires when no dirty path lies outside the lane's output directories.
  if ! git status --short | awk '{print $2}' | grep -qvE '^conformance/haqp/evidence/|^fuzz/artifacts/'; then
    echo "every dirty path is lane output; to discard it (irreversible; preview with git clean -n):" >&2
    echo "  git checkout -- conformance/haqp/evidence && git clean -f conformance/haqp/evidence fuzz/artifacts" >&2
  fi
  exit 2
fi

# F-77: a campaign is 1.5-2.2 hours and dies whole. On 2026-09-19 two runs were
# SIGTERMed under host memory pressure -- 54 of 62 GB resident with an unrelated
# PTQ job and an editor session on the box -- after CI had already passed, which
# cost the run and proved nothing. Refuse in two seconds instead of dying at 90%.
#
# The check is available memory, not free: page cache is reclaimable and counting
# it as pressure would refuse a healthy idle host. Override for a deliberately
# tight run; there is no way to make the campaign itself smaller.
REQUIRED_AVAILABLE_MB="${HAQP_REQUIRED_AVAILABLE_MB:-12000}"
available_mb=$(awk '/^MemAvailable:/ { print int($2 / 1024) }' /proc/meminfo 2>/dev/null)
if [ -n "$available_mb" ] && [ "$available_mb" -lt "$REQUIRED_AVAILABLE_MB" ]; then
  echo "refusing to start: ${available_mb} MB available, ${REQUIRED_AVAILABLE_MB} MB required." >&2
  echo "a campaign is 1.5-2.2 hours and is lost whole if the host reclaims it." >&2
  echo "largest resident processes:" >&2
  ps -eo rss,comm --sort=-rss 2>/dev/null | awk 'NR>1 && NR<=6 { printf "  %6.1f GB  %s\n", $1/1048576, $2 }' >&2
  echo "wait for the host to quiet, or set HAQP_REQUIRED_AVAILABLE_MB to accept the risk." >&2
  exit 2
fi

if [ -z "${HAQP_CAMPAIGN_CLOCK:-}" ]; then
  echo "refusing to start: run this under the campaign clock, or ADR-0020 §7's" >&2
  echo "evidence is never written and the lane fails at haq-verify:" >&2
  echo "    just haq-lane" >&2
  exit 2
fi

BASE="$(git rev-parse HEAD)"
# §1 makes every verified fix invalidate eligibility until the lane reruns — so
# rerunning must be possible. The blind lane refuses unless the packet is
# `proposed`/`not-run`, so after a successful qualification the SECOND run died
# at the reviews with "packet is not proposed/not-run" (M17.5 F-35). Reset here,
# loudly, rather than leaving a hand-edit as an undocumented prerequisite.
if ! python3 - <<'RESET'
import json, pathlib, sys
p = pathlib.Path("conformance/haqp/packet.json")
d = json.loads(p.read_text())
if d["qualification_state"] == "not-run" and d.get("provenance") is None:
    sys.exit(0)
print("re-qualifying: resetting packet to not-run and dropping stale provenance")
d["qualification_state"] = "not-run"
d.pop("provenance", None)
p.write_text(json.dumps(d, indent=1) + "\n")
RESET
then
  echo "could not reset the packet for re-qualification" >&2
  exit 2
fi

echo "=== HAQP-1a lane at fixed base ${BASE:0:12} ==="
echo "Nothing may commit source or gate code until this finishes and the packet"
echo "is flipped — any such commit invalidates every artifact below."
echo

# ADR-0020 requires the report to carry per-command exit statuses and raw
# artifact hashes. No lane recorded them, so those cells could only ever be
# filled by inventing values (M17.5 pre-flight). Every stage now runs through
# `stage`, which records command, exit code, elapsed seconds and the BLAKE3 of
# the artifact it produced into a manifest the flip renders from.
LANES="conformance/haqp/evidence/lanes.json"
: > "${LANES}.parts"

# Corpus-scope capture (ruling 2026-09-04). verify_corpus_scope_replays used
# to RE-EXECUTE every scope's command inside the gate -- a second fuzz campaign
# and a second pair of paid reviews per haq-verify. Now each stage runs under
# strace exactly once, here, and the gate verifies the recorded bytes.
#
# The declared command string is what scope_lane_command names for that scope,
# verbatim, and it is what actually runs: the gate compares the recorded string
# against its closed registry, so the two cannot be allowed to differ.
SCOPE_ROWS="conformance/haqp/evidence/scope-rows.ndjson"
: > "$SCOPE_ROWS"

# stage <label> <artifact-or-empty> <scope-or-empty> <declared command string>
stage() {
  local label="$1" artifact="$2" scope="$3" declared="$4"
  echo "--- ${label} ---"
  local started exit_code elapsed digest trace
  trace="target/haqp/scope-${label}.trace"
  mkdir -p target/haqp
  started=$(date +%s)
  set +e
  # HAQP_STAGE_TRACE: the fuzz campaign cannot trace its binaries under a stage
  # that is already traced (one ptrace slot, F-43); it carves them from this
  # trace by pid instead. Harmless to every other stage.
  HAQP_STAGE_TRACE="$PWD/$trace" strace -f -q -e trace=%file -o "$trace" sh -c "$declared"
  exit_code=$?
  set -e
  if [ -n "$scope" ]; then
    python3 scripts/haqp_scope_row.py "$scope" "$declared" "$trace" "$exit_code" >> "$SCOPE_ROWS"
  else
    rm -f "$trace"
  fi
  elapsed=$(( $(date +%s) - started ))
  digest="absent"
  if [ -n "$artifact" ] && [ -f "$artifact" ]; then
    digest=$(cargo run -q -p liminal-xtask -- haq hash "$artifact")
  fi
  printf '{"stage":"%s","command":"%s","exit_code":%s,"elapsed_s":%s,"artifact":"%s","artifact_blake3":"%s"}\n' \
    "$label" "$(printf '%s' "$declared" | sed 's/"/\\"/g')" "$exit_code" "$elapsed" "${artifact:-none}" "$digest" \
    >> "${LANES}.parts"
  return "$exit_code"
}

# ADR-0020 §6: two blinded reviews, distinct model families. FIRST, not last.
#
# The reviewers are handed the ADR, the standing rulings, the review markdown,
# the packet and four source files — never an evidence artifact — so nothing
# they read is produced by the stages below, and running them first changes no
# input, no command and no fixed base. What it changes is the cost of a
# refusal: six lanes in a row have passed every stage and then refused on a
# review finding, each after ~110 minutes. First, that verdict arrives in ten.
stage reviews "" reviews "just haq-blind-review"

# Then the cheap lanes: a failure here should not cost 3.5 hours to discover.
# `ci` and `replay` are scopes the closed registry names and the lane never
# ran; they go first so the canaries stage below writes the final canaries.json.
stage ci          ""                                          ci          "just ci"
stage replay      ""                                          replay      "cargo test -q --workspace"
stage canaries    conformance/haqp/evidence/canaries.json    canaries    "just haq-canaries"
stage generated   conformance/haqp/evidence/generated.json   generated   "just haq-generated"
stage crash       conformance/haqp/evidence/crash.json       crash       "just haq-crash"
stage concurrency conformance/haqp/evidence/concurrency.json ""          "just haq-concurrency"
echo "--- mutants (stage 1b machinery; recorded, not claimed) ---"
# `|| echo` swallowed every failure, not only the expected not-ready one — the
# F-19 family, masking an exit code (M17.5 F-35). Only not-ready is tolerated.
# The tolerance must judge THIS run's output. It used to grep the existing
# mutants.json for "not-ready", so a stage that died before writing anything --
# which is what happened on 2026-09-01, when the runner's blanket clean check
# refused a tree the earlier lanes had legitimately dirtied -- was excused by a
# file from three weeks earlier, and the stale source_commit rode through to the
# flip. Requiring the file to be REWRITTEN at this base closes that.
mutants_before=$(cargo run -q -p liminal-xtask -- haq hash conformance/haqp/evidence/mutants.json 2>/dev/null || echo none)
if ! stage mutation conformance/haqp/evidence/mutants.json mutation "just haq-mutants"; then
  mutants_after=$(cargo run -q -p liminal-xtask -- haq hash conformance/haqp/evidence/mutants.json 2>/dev/null || echo none)
  if [ "$mutants_after" = "$mutants_before" ]; then
    echo "mutant lane failed WITHOUT writing evidence; the not-ready tolerance may" >&2
    echo "not be granted by a pre-existing file (M17.5)" >&2
    exit 2
  fi
  if ! grep -q '"status": *"not-ready"' conformance/haqp/evidence/mutants.json 2>/dev/null; then
    echo "mutant lane failed for a reason other than not-ready" >&2
    exit 2
  fi
  echo "mutant lane reported not-ready, which is expected for stage 1a"
fi

# The long one. Writes fuzz.json and corpus-access.json.
stage fuzz conformance/haqp/evidence/fuzz.json fuzz \
  "scripts/haqp_fuzz_campaign.sh 1800 conformance/haqp/evidence/fuzz.json"

# Fold the eight scope rows into the audit the fuzz campaign just wrote.
python3 - "$SCOPE_ROWS" <<'MERGE'
import json, sys, pathlib
rows = [json.loads(l) for l in pathlib.Path(sys.argv[1]).read_text().splitlines() if l.strip()]
audit = pathlib.Path("conformance/haqp/evidence/corpus-access.json")
d = json.loads(audit.read_text())
d["scope_traces"] = rows
audit.write_text(json.dumps(d, indent=1) + "\n")
pathlib.Path(sys.argv[1]).unlink()
print(f"merged {len(rows)} scope rows into corpus-access.json")
MERGE

# One manifest, written once, so a partial lane cannot leave half a file behind.
python3 - "$LANES" <<'MANIFEST'
import json, sys, pathlib
out = pathlib.Path(sys.argv[1])
parts = out.with_suffix(".json.parts")
rows = [json.loads(line) for line in parts.read_text().splitlines() if line.strip()]
out.write_text(json.dumps({"schema_version": "haqp-lane-manifest-v1", "stages": rows}, indent=1) + "\n")
parts.unlink()
MANIFEST

echo
echo "=== every lane ran at ${BASE:0:12} ==="
if [ "$(git rev-parse HEAD)" != "$BASE" ]; then
  echo "HEAD moved during the run; the artifacts describe more than one tree." >&2
  echo "Rerun from a fixed base." >&2
  exit 2
fi

python3 scripts/haqp_flip_packet.py

# The lane used to END at the flip, so it reported success without ever
# checking the packet it had just written. Every gate downstream of
# `qualification_state == "complete"` is unreachable until this point, which
# means the first run of a dozen verifiers happened after the lane had already
# said it was done, and a failure surfaced later at the un-ignored gate test
# with the flipped packet already committed.
#
# The commit is made here rather than by hand: leaving five commands for a
# human after a four-hour lane is where transcription errors enter, and the
# flip already prints the exact commit it wants.
echo
echo "--- metadata child ---"
git add conformance/haqp/packet.json conformance/haqp/evidence \
        docs/execution/phase1-suite-review.md \
        docs/execution/m17-5-adversarial-findings.md \
        conformance/tests/phase0.rs
git commit --quiet -m 'record HAQP-1a qualification evidence'

echo "--- verifying the packet this lane just wrote ---"
if ! just haq-verify; then
  # Reversible: --soft keeps every artifact staged, so nothing the campaign
  # produced is lost and the failure can be inspected in place.
  git reset --soft HEAD^
  echo >&2
  echo "the qualified gate refused the packet this lane produced." >&2
  echo "the metadata commit was rolled back (--soft); artifacts are staged." >&2
  exit 2
fi
echo
echo "=== HAQP-1a qualified at ${BASE:0:12} ==="
