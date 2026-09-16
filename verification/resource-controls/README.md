# Ordering resource controls

This is a development-only, standalone test binary. It is not formal
qualification evidence and does not establish a whole-system out-of-memory
claim. Its unsafe `GlobalAlloc` is an external test-only forwarding boundary;
the production unsafe-code lint and production allocator remain unchanged.

The platform layout control is bounded to this target: it asserts the observed
`(u128, u128, bool)` size/alignment before arming the allocator. The binary
consolidates human-admission, automatic-admission, malformed-input, Prepare and
all four nonterminal recovery-state controls
without inventing production interfaces. All fixtures, actors, and plans are
created before each fault window; the allocator disarms immediately after each
call and forwards all non-target operations to `System`.

Reproduce from the repository root with dependencies available offline:

```sh
cargo run --locked --offline --manifest-path verification/resource-controls/Cargo.toml
```

The caller must provide bounded resources (2 CPUs, 4 GiB memory, no swap,
256 tasks, and a 600-second runtime). This package intentionally does not claim
those caps or this probe qualify the implementation. This standalone package is
not yet part of `just ci`; its run must be reported separately. It currently
covers admission, Prepare and each nonterminal recovery state, not mixed-effect
plans, post-effect finalization allocation timing or every allocation
site. Its lockfile is checked against the authoritative workspace registry pins.

Applying and Finalizing fixtures use real driver cutoffs with both a reached
flag and exact panic-payload checks. ExternalApplied uses the existing trusted
fixture-assembly pattern: after a real Applying acknowledgement, append only the
legal ExternalApplied transition with its exact provenance. No runtime crash
hook leaves that state durable before Finalizing, so this case is not claimed
as a crash witness. Expected caught cutoff panics remain visible in stderr;
the command must still exit zero with every assertion satisfied.
