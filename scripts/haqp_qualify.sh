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

cd "$(dirname "$0")/.."

if [ -n "$(git status --porcelain)" ]; then
  echo "refusing to start: the tree is dirty; the fixed base must be clean (ADR-0020 §1)" >&2
  exit 2
fi

BASE="$(git rev-parse HEAD)"
echo "=== HAQP-1a lane at fixed base ${BASE:0:12} ==="
echo "Nothing may commit source or gate code until this finishes and the packet"
echo "is flipped — any such commit invalidates every artifact below."
echo

# Cheap lanes first: a failure here should not cost 3.5 hours to discover.
echo "--- canaries ---";  just haq-canaries
echo "--- generated ---"; just haq-generated
echo "--- crash ---";     just haq-crash
echo "--- mutants (stage 1b machinery; recorded, not claimed) ---"
just haq-mutants || echo "mutant lane reported not-ready, which is expected for stage 1a"

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
