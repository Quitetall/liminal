#!/usr/bin/env bash
# Run the suite's declared halves: `portable`, `host`, `doctest`, or `all`
# (portable then host, the default).
#
# M17.5 F-82: `sanitizer_replay_reproduces_the_canonical_build_and_refuses_the_rest`
# performs a full ASan release build INSIDE a test -- 73.9s on a fresh build root,
# saturating every core. Beside it, the crash tests, which spawn a subprocess per
# durable boundary and are judged on wall clock, were starved into a 240s
# timeout; the same test passes in 7.9s alone. So the halves never run together.
#
# F-84: every mode first builds the workspace binaries with the tests' own flags.
# Cargo builds a package's executables only for that package's own integration
# tests, and liminal-cli has none, so `lim` did not exist on a clean runner.
#
# The halves are declared once, in .config/host-capability-tests.filter. The
# assurance merge profile registers `portable`, `host` and `doctest` as separate
# gates, so the generated CI workflow and local `just ci` run the same sets.
#
# EXCLUDE_UNIX_TOOLING (set by the CI matrix on Windows only, F-90) excludes the
# Unix-only tooling crates; every test in them still runs on Linux and macOS.
# `${EXCLUDE[@]+...}` because macOS bash 3.2 treats an empty array as unbound
# under `set -u`.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

MODE="${1:-all}"
# shellcheck disable=SC2206 # deliberate word-splitting of a flag list
EXCLUDE=(${EXCLUDE_UNIX_TOOLING:-})

EXPR="$(grep -v '^[[:space:]]*#' .config/host-capability-tests.filter \
        | grep -v '^[[:space:]]*$' | tr '\n' ' ')"
if [ -z "${EXPR// /}" ]; then
  echo "ERROR: .config/host-capability-tests.filter declares no tests." >&2
  exit 1
fi

build_binaries() {
  echo "=== workspace binaries ==="
  cargo build --workspace ${EXCLUDE[@]+"${EXCLUDE[@]}"} --all-features --bins
}

portable() {
  echo "=== portable half ==="
  cargo nextest run --workspace ${EXCLUDE[@]+"${EXCLUDE[@]}"} --all-features --profile ci \
    --filter-expr "not ($EXPR)"
}

host() {
  echo "=== host-capability half ==="
  cargo nextest run --workspace --all-features --profile ci --filter-expr "$EXPR"
}

case "$MODE" in
  portable) build_binaries && portable ;;
  host) build_binaries && host ;;
  doctest) cargo test --workspace ${EXCLUDE[@]+"${EXCLUDE[@]}"} --doc ;;
  all) build_binaries && portable && host ;;
  *) echo "usage: $0 [portable|host|doctest|all]" >&2; exit 2 ;;
esac
