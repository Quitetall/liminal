#!/usr/bin/env bash
# Run the suite as its two declared halves, in sequence.
#
# M17.5 F-82: `sanitizer_replay_reproduces_the_canonical_build_and_refuses_the_rest`
# performs a full ASan release build INSIDE a test. Measured at 73.9s on a fresh
# build root, saturating every core. Running beside it, the crash tests -- which
# spawn a subprocess per durable boundary and are judged on wall clock -- were
# starved into a 240s timeout; the same test passes in 7.9s alone.
#
# The halves are the ones .config/host-capability-tests.filter already declares
# for CI, so this changes no membership: it runs them one after the other instead
# of together, and local `just ci` now runs exactly what the workflow runs.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

EXPR="$(grep -v '^[[:space:]]*#' .config/host-capability-tests.filter \
        | grep -v '^[[:space:]]*$' | tr '\n' ' ')"
if [ -z "${EXPR// /}" ]; then
  echo "ERROR: .config/host-capability-tests.filter declares no tests." >&2
  exit 1
fi

# F-84: build every workspace binary first, with the same flags as the tests.
# Cargo builds a package's executables only for that package's own integration
# tests, and liminal-cli has none -- so `lim` did not exist on a clean runner
# and ten conformance tests that shell out to it failed. Building here makes the
# harness's once-per-run fallback a no-op.
echo "=== workspace binaries ==="
cargo build --workspace --all-features --bins || exit $?

echo "=== portable half ==="
cargo nextest run --workspace --all-features --profile ci --filter-expr "not ($EXPR)" || exit $?

echo "=== host-capability half ==="
cargo nextest run --workspace --all-features --profile ci --filter-expr "$EXPR" || exit $?
