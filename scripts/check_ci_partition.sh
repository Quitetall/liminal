#!/usr/bin/env bash
# Prove the CI split covers the suite exactly: no test in both halves, none in
# neither.
#
# M17.5 F-79: the hazard is not a red job. It is a test that falls out of both
# lists and stops running while both jobs stay green -- the same defect as F-78's
# stale binary, which reported green three times while exercising the wrong
# executable. Counting is the whole proof: |host| + |portable| == |all|, and
# because the portable side is literally `not (host)`, a test can never be in
# both.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

EXPR="$(grep -v '^[[:space:]]*#' .config/host-capability-tests.filter | grep -v '^[[:space:]]*$' | tr '\n' ' ')"
if [ -z "${EXPR// /}" ]; then
  echo "ERROR: .config/host-capability-tests.filter declares no tests." >&2
  exit 1
fi

count() { # count() <filter-expr|""> -> number of listed tests
  if [ -z "$1" ]; then
    cargo nextest list --workspace --all-features 2>/dev/null | grep -cE '\S'
  else
    cargo nextest list --workspace --all-features --filter-expr "$1" 2>/dev/null | grep -cE '\S'
  fi
}

all=$(count "")
host=$(count "$EXPR")
portable=$(count "not ($EXPR)")

printf 'all=%s host=%s portable=%s\n' "$all" "$host" "$portable"

declared=$(grep -c 'test(=' .config/host-capability-tests.filter)

status=0
if [ "$host" -eq 0 ]; then
  echo "ERROR: the host-capability expression matches no test; it is stale." >&2
  status=1
elif [ "$host" -ne "$declared" ]; then
  # The sum check below cannot see this: a name matching nothing leaves the two
  # halves summing correctly while the entry is dead. A renamed test then runs
  # in the portable job and fails there, which is visible -- but saying so here
  # names the cause instead of leaving it to be rediscovered.
  echo "ERROR: $declared names declared, $host matched." >&2
  echo "An entry in .config/host-capability-tests.filter names no test (renamed or" >&2
  echo "removed), or one name matches tests in two binaries." >&2
  status=1
fi
if [ "$(( host + portable ))" -ne "$all" ]; then
  echo "ERROR: host ($host) + portable ($portable) != all ($all)." >&2
  echo "A test is in both halves or in neither. Neither is acceptable: one runs it" >&2
  echo "twice, the other stops running it while both CI jobs stay green." >&2
  status=1
fi
exit "$status"
