# Liminal task runner. `just` with no args lists recipes.
# An xtask crate replaces recipes only when one needs real logic (ADR-0004).

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# One-time machine setup
setup:
    cargo install cargo-binstall --locked
    cargo binstall -y cargo-nextest cargo-insta cargo-deny taplo-cli typos-cli

check:
    cargo check --workspace --all-targets --all-features

fmt:
    cargo fmt --all
    taplo fmt

fmt-check:
    cargo fmt --all --check
    taplo fmt --check
    typos

lint:
    cargo clippy --workspace --all-targets --all-features -- -Dwarnings

test *ARGS:
    cargo nextest run --workspace --all-features {{ ARGS }}
    cargo test --workspace --doc

# Serialized crash-injection group only (ILRP boundary-kill tests)
crash:
    cargo nextest run --workspace -E 'test(/^crash_/)'

# Spec-debt meter: per-phase passed/ignored counts — the project's live progress bar.
gates:
    cargo run -p liminal-conformance --bin gates

# HAQP packet checks and evidence lanes.
haq-inventory:
    cargo run -p liminal-xtask -- haq verify-inventory

haq-verify:
    cargo run -p liminal-xtask -- haq verify

haq-canaries:
    cargo run -p liminal-xtask -- haq run-canaries

haq-generated cases="100000":
    cargo run -p liminal-xtask -- haq generate --cases {{ cases }}

# Two isolated model-family HAQP reviews. Fails closed on dirty/unqualified base.
haq-crash:
    cargo run -p liminal-conformance --bin crash-evidence

haq-blind-review:
    python3 scripts/haqp_blind_review.py --run

# Review insta snapshot changes interactively
snap:
    cargo insta review

doc:
    RUSTDOCFLAGS="-Dwarnings" cargo doc --workspace --no-deps --all-features

deny:
    cargo deny check bans licenses sources advisories

# §116 benchmarks — implemented set only. Baselines, never gates, until Phase 1 (Law 14).
bench *ARGS:
    cargo bench -p liminal-benches {{ ARGS }}

# Run each frozen target against its committed development corpus. Full HAQP
# qualification still requires five separate 31-minute sanitizer runs.
fuzz-smoke:
    cargo +nightly fuzz run cst_parse --sanitizer address -- -runs=1000
    cargo +nightly fuzz run format_idempotent --sanitizer address -- -runs=1000
    cargo +nightly fuzz run canonical_round_trip --sanitizer address -- -runs=1000
    cargo +nightly fuzz run incremental_full_equivalence --sanitizer address -- -runs=1000
    cargo +nightly fuzz run html_render --sanitizer address -- -runs=1000

phase1-tests:
    cargo nextest run -p liminal-conformance --test laws --test classes -E 'test(/(formatter_idempotence_law_holds|canonical_round_trip_law_holds|incremental_equals_full_compile_law_holds|malformed_source_never_panics_and_round_trips|fuzz_regressions_stay_fixed|full_document_html_matches_golden|incremental_patch_equals_full_render)/)'

bench-sample count="30":
    cargo run -p liminal-xtask -- bench sample {{ count }}

bench-baseline-check:
    cargo run -p liminal-xtask -- bench baseline-check

bench-gate:
    cargo run -p liminal-xtask -- bench gate

# Threaded lane (M17.5 F-11). nextest gives every test its own PROCESS, so the
# suite's green status under `cargo test` — tests as THREADS in one process —
# was never exercised by CI. That is the runner cargo-mutants drives, and it is
# where the store-lock defect lived: a suite whose result depends on the harness
# is not qualified. Kept as its own recipe so the failure names the lane.
test-threaded:
    cargo test --workspace

# Everything CI runs, locally, in CI order
ci: fmt-check lint
    cargo nextest run --workspace --all-features --profile ci
    just test-threaded
    cargo test --workspace --doc
    just doc
    just deny

bump-toolchain version:
    sed -i 's/^channel = ".*"/channel = "{{ version }}"/' rust-toolchain.toml
    @echo "Now update rust-version in Cargo.toml [workspace.package] and commit both."
