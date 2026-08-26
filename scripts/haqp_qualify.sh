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

if [ -n "$(git status --porcelain)" ]; then
  echo "refusing to start: the tree is dirty; the fixed base must be clean (ADR-0020 §1)" >&2
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

# Cheap lanes first: a failure here should not cost 3.5 hours to discover.
echo "--- canaries ---";  just haq-canaries
echo "--- generated ---"; just haq-generated
echo "--- crash ---";     just haq-crash
echo "--- mutants (stage 1b machinery; recorded, not claimed) ---"
# `|| echo` swallowed every failure, not only the expected not-ready one — the
# F-19 family, masking an exit code (M17.5 F-35). Only not-ready is tolerated.
if ! just haq-mutants; then
  if ! grep -q '"status": *"not-ready"' conformance/haqp/evidence/mutants.json 2>/dev/null; then
    echo "mutant lane failed for a reason other than not-ready" >&2
    exit 2
  fi
  echo "mutant lane reported not-ready, which is expected for stage 1a"
fi

# The long one. Writes fuzz.json and corpus-access.json.
echo "--- fuzz campaign: 7 targets x 1800s (~3.5h) ---"
scripts/haqp_fuzz_campaign.sh 1800 conformance/haqp/evidence/fuzz.json

# ADR-0020 §6: two blinded reviews, distinct model families. Last, because it
# reads the tree the other lanes just described.
echo "--- blind reviews ---"; just haq-blind-review

echo
echo "=== every lane ran at ${BASE:0:12} ==="
if [ "$(git rev-parse HEAD)" != "$BASE" ]; then
  echo "HEAD moved during the run; the artifacts describe more than one tree." >&2
  echo "Rerun from a fixed base." >&2
  exit 2
fi

python3 scripts/haqp_flip_packet.py
