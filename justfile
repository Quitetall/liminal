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
    # Scope TOML formatting to repository inputs; nested untracked projects are
    # outside this workspace's contract.
    if test -n "$(git ls-files -- '*.toml')"; then git ls-files -z -- '*.toml' | xargs -0 taplo fmt; fi

fmt-check:
    cargo fmt --all --check
    if test -n "$(git ls-files -- '*.toml')"; then git ls-files -z -- '*.toml' | xargs -0 taplo fmt --check; fi
    # Explicit paths bypass typos excludes unless --force-exclude is present.
    # Heldout corpus is intentionally absent from sparse qualification
    # worktrees; never pass those locked files to a checker.
    git ls-files -z -- ':!conformance/corpora/heldout/**' | xargs -0 typos --force-exclude

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

# Re-attest the not-applicable concurrency declaration at the current base.
# Nothing produced conformance/haqp/evidence/concurrency.json before this, so
# it carried an August source_commit into every later lane (M17.5).
haq-concurrency:
    cargo run -p liminal-xtask -- haq concurrency

haq-mutants *ARGS:
    cargo run -p liminal-xtask -- haq mutants {{ ARGS }}

# Two isolated model-family HAQP reviews. Fails closed on dirty/unqualified base.
haq-crash:
    cargo run -p liminal-conformance --bin crash-evidence

haq-blind-review:
    python3 scripts/haqp_blind_review.py --self-test
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
# qualification requires seven separate 30-minute sanitizer runs plus traced
# corpus-access manifests.
fuzz-smoke:
    cargo +nightly fuzz run cst_parse --sanitizer address -- -runs=1000
    cargo +nightly fuzz run format_idempotent --sanitizer address -- -runs=1000
    cargo +nightly fuzz run canonical_round_trip --sanitizer address -- -runs=1000
    cargo +nightly fuzz run incremental_full_equivalence --sanitizer address -- -runs=1000
    cargo +nightly fuzz run html_render --sanitizer address -- -runs=1000
    cargo +nightly fuzz run graph_interchange_codec --sanitizer address -- -runs=1000
    cargo +nightly fuzz run ilrp_recovery --sanitizer address -- -runs=1000

phase1-tests:
    cargo nextest run -p liminal-conformance --test laws --test classes --run-ignored ignored-only -E 'test(/(formatter_idempotence_law_holds|canonical_round_trip_law_holds|incremental_equals_full_compile_law_holds|malformed_source_never_panics_and_round_trips|fuzz_regressions_stay_fixed|full_document_html_matches_golden|incremental_patch_equals_full_render)/)'

bench-sample count="30":
    cargo run -p liminal-xtask -- bench sample {{ count }}

bench-baseline-check:
    cargo run -p liminal-xtask -- bench baseline-check

bench-gate:
    cargo run -p liminal-xtask -- bench gate

formal-check:
    cargo run -p liminal-xtask -- formal check

formal-bootstrap verus_root tlc_jar output:
    python3 -B verification/bootstrap/run.py --verus-root {{ quote(verus_root) }} --tlc-jar {{ quote(tlc_jar) }} --output {{ quote(output) }}

formal-bootstrap-self-test:
    python3 -B -m unittest -v verification/bootstrap/test_run.py

# Fast proof-support unit controls only: no verifier, model, network, or heavy proof execution.
formal-proof-self-test:
    python3 -B -m unittest discover -v -s verification/proof -p 'test_*.py'

formal-proof:
    cargo run -p liminal-xtask -- formal proof

formal-model:
    cargo run -p liminal-xtask -- formal model

formal-adapters:
    cargo run -p liminal-xtask -- formal adapters

formal-gate phase:
    cargo run -p liminal-xtask -- formal gate {{ phase }}

# Threaded lane (M17.5 F-11). nextest gives every test its own PROCESS, so the
# suite's green status under `cargo test` — tests as THREADS in one process —
# was never exercised by CI. That is the runner cargo-mutants drives, and it is
# where the store-lock defect lived: a suite whose result depends on the harness
# is not qualified. Kept as its own recipe so the failure names the lane.
#
# `--all-targets` deliberately excludes doc tests: `ci` already runs them once
# via `cargo test --workspace --doc`, and a bare `cargo test --workspace` would
# run all 27 doc-test sections a second time.
test-threaded:
    cargo test --workspace --all-targets

# Mutation lane (M17.5 F-24). cargo-mutants copies the source tree per worker.
# `.gitignore` excludes `/target`, which it honors, but `fuzz/target` is a
# SECOND build directory (~656 MB) that it copies in full, once per worker. On
# a machine whose /tmp is a tmpfs — this one is 32 GB — `-j 6` is ~4 GB of
# RAM-backed copies of a build cache no mutant reads, and the campaign dies on
# ENOSPC. The full ~2915-mutant campaign is that same copy, repeated.
#
# TMPDIR is pinned here rather than left to a doc note so a fresh checkout gets
# it without reading the findings ledger.
#
# It must live OUTSIDE the repo. Pointing it at `.cache/` inside the tree makes
# cargo-mutants copy its own worker copies into each new worker copy, and the
# run dies on `File name too long` after nesting the path ~80 times deep.
mutants-dir := env('HOME') / ".cache/liminal-mutants"

mutants *ARGS:
    mkdir -p {{ mutants-dir }}
    TMPDIR={{ mutants-dir }} cargo mutants {{ ARGS }}

# The full HAQP-1a lane, wall-clocked. ADR-0020 §7 bounds the campaign, and
# verify_campaign_clock requires conformance/haqp/evidence/campaign.json — which
# only this wrapper writes. haqp_qualify.sh refuses to run outside it (F-35).
haq-lane run="run-1":
    scripts/haqp_campaign_clock.sh {{ run }} conformance/haqp/evidence/campaign.json -- ./scripts/haqp_qualify.sh

# Everything CI runs, locally, in CI order
ci: fmt-check lint
    just formal-bootstrap-self-test
    just formal-proof-self-test
    cargo nextest run --workspace --all-features --profile ci
    just test-threaded
    cargo test --workspace --doc
    just doc
    just deny
    # M17.5 F-14: these two gates were outside CI, so the packet digest sat
    # broken for three commits without anything going red. Both are seconds.
    just haq-inventory
    just haq-canaries

bump-toolchain version:
    sed -i 's/^channel = ".*"/channel = "{{ version }}"/' rust-toolchain.toml
    @echo "Now update rust-version in Cargo.toml [workspace.package] and commit both."
