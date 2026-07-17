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

# Stub until fuzz/ exists (activation: liminal-cst lands, Phase 1 — see docs/implementation-plan.md)
fuzz-smoke:
    @echo "deferred: fuzz/ is created when liminal-cst lands (Phase 1)" && exit 1

# Everything CI runs, locally, in CI order
ci: fmt-check lint
    cargo nextest run --workspace --all-features --profile ci
    cargo test --workspace --doc
    just doc
    just deny

bump-toolchain version:
    sed -i 's/^channel = ".*"/channel = "{{ version }}"/' rust-toolchain.toml
    @echo "Now update rust-version in Cargo.toml [workspace.package] and commit both."
