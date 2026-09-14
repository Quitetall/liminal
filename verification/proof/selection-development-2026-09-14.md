# Compiler-selection and strict-forwarding development record

Status: development evidence only. Every retained receipt reports
`qualification: false`. This closes the observed compiler-selection and strict
argument-forwarding uncertainty only for the reconstructed sandbox. It does not
qualify a proof, discharge an obligation or authorize a phase.

## Selection sequence

All attempts used staged source commit
`425cc2de8838fc35de5d9683c5437a6f3e273059`, the constructed Rust 1.97.1
payload, pinned Verus distribution and reconstructed vendor tree, without
network or ambient user Cargo/Rust homes. The command service requested CPU
quota 200%, MemoryHigh 2 GiB, MemoryMax 4 GiB, zero swap, 256 tasks, a 64 MiB
file limit and ten-minute runtime limit.

- V1 stopped before compiler execution because `--disable-userns` required
  `--unshare-user`. Preparation SHA-256:
  `59c30d18e8b5e0e627e01746b553f8f2a6ff56689b177969b01b5e54b3625f43`;
  `rust-selection-v1/command-evidence/receipt.json`:
  `860a59f388db4f029834a3aaa0dc252b3957aca42e676e02034e9b87475a2123`.
- V2 reached the tool path, then fresh-home rustup refused the absent named
  `1.97.1-x86_64-unknown-linux-gnu` registration. Preparation:
  `89e48c3b2d335c7f7c0741ac901c1f23b25a1e5b9d288aa2fa20ae4e93862b16`;
  receipt:
  `ce44fb018e1993a5f21b2b667312ef2e033b80415a81b4a7cc9298c7a0088d51`;
  `rust-selection-v2/logs/trace.log`:
  `23259e371e83481320982a098859033465755342bdb03ec0e8fc454536d338e4`.
- V3 read-only-bound the same constructed payload at rustup's required name,
  without installation or ambient registration. The child exited 0. Complete
  output separates vstd `1862 verified, 0 errors` from `liminal-safety`'s
  entire-crate JSON `4 verified, 0 errors`. Preparation:
  `4ecbf74250b8d788a64f499c20ecad2bf41b5dff74bfbf83a5fd2c471b01f89b`;
  `rust-selection-v3/command-evidence/receipt.json`:
  `6b24066aa69aeaa279fb41c1bffb0be185bdfa200d5089f4ca18d30e0686b2d8`;
  stdout:
  `adff75e952ad4d2b4238f5a2bb97949c54cfc16b37ac7d8df6bcb2b4199e0099`;
  stderr:
  `c4e0927a698ba4177fb9c1afa14a83217b734408c70229ee71a7cabda5b3a0a2`;
  `rust-selection-v3/logs/trace.log`:
  `788d8565db0640f92965aa78f50b5038faecd7a5b7ee3976b2fd8dd5937f3beb`.
  Peak memory was 2,318,127,104 bytes and cleanup observed the unit absent.

These paths are below `/mnt/4tb/liminal-formal-evidence/probes/`. V3 traces
constructed `/rust/bin/cargo` and `/rust/bin/rustc`, named rustup selection,
pinned `/verus/rust_verify` and bundled Z3. Binary/hash and behavioral selection
evidence is not verifier-source correctness evidence; the pinned binary's source
was not reconstructed and reviewed by this probe.

## Strict-forwarding inverse control

Both variants retained V3's sandbox and read-only overlaid only
`crates/liminal-safety/src/lib.rs` with a scratch proof containing
`assume(false)`. The immutable base file SHA-256 was
`a96cf145ef072b23916d7bc92f6c90cde82102a8d0451c2b361f591f3795c5ba`;
the exact 120-byte overlay was
`9cdc2a9cab7e7a07f0dfa3779ea7fd101c9fc147d46bca90f2f3e71d8b2ac2c9`.
After normalizing independent fresh writable paths, commands differed only by
the strict command's final `--no-cheating`; both retained
`--fwd-verus-args-to roots` and `--output-json`.

Loose exited 0 with entire-crate JSON `1 verified, 0 errors`. Strict exited 101
and reported `assume/admit not allowed with --no-cheating` at
`crates/liminal-safety/src/lib.rs:7:5`. Both preflight and postflight bindings
were byte-identical, SHA-256
`5fe27c6a26bce44be770bd3a7c9eb07507bdf8728f88f84fefe704a089d09a3b`,
and both units were absent after cleanup. Receipts were
`strict-forwarding-v1/loose/command-evidence/receipt.json` SHA-256
`c199987ecd0634a67a6a280fca50048179b2e4dc22cc93c2da6dc6d4f916f062`
and `strict-forwarding-v1/strict/command-evidence/receipt.json` SHA-256
`4722fd8fa35531fd0b584981d5d33fc47521e35ede4594ac9c51810189e4d926`.
Complete raw outputs and traces are in each variant's `command-evidence/` and
`logs/` directories.

This deliberately scratch-overlaid inverse control establishes forwarding
behavior, not production-source proof or whole-proof coverage. The bounded,
same-source witness pair is now complete. Ordinary V1 and verified V2 build and
execute commands all exited 0 and cleanup observed their units absent. Both
executions produced the exact 26 bytes `acknowledgement-witness:5\n` on stdout
and empty stderr. The ordinary binary SHA-256
`7a23935ff99dc14176ea4184a05f4b0fb579adee1ef5faab1022ca38871a8eb9`
and verified binary SHA-256
`8c97643e59ba521db14a393faa436c3d84b071230c1cd664a23960cf9a0fde68`
each matched its own build, preflight and postflight hashes; the two binaries
are not byte-identical. Verified V2's root entire-crate JSON reports `4 verified,
0 errors`, including `contains_identity` and `acknowledgement_matches`. Its
second example JSON reports `0 verified, 0 errors` with empty `func-details` and
is not counted as a proof. Trace lines 23469 and 23846 bind the library and
example compilations. Verified build CPU usage was 70.594364 seconds with peak
memory 2,148,315,136 bytes; execution used 0.012192 seconds and peaked at
3,571,712 bytes. Build and execution receipt SHA-256 values were respectively
`15ebb19e262d4f4e7cc6ad08cda164f9177ccc914e409d1397ef71ff79e1d00a`
and `142412ada97d137244f6509c80761bc7411a8dbf0ace2a16f144b4a218eb810c`
under `ack-sandbox-witness-v2/verified/`.

The preserved verified V1 child exit 1 was solely a cargo-verus CLI-ordering
error: `--config` could not follow `--example`; no compiler started. V2 corrected
the ordering without weakening the verified command. This is development
witness binding, not a qualified executable, cold replay, or global-core proof.
Full orchestration, independent cold construction cycles, and a negative runtime
fixture in this sandbox remain pending. All sixteen obligations remain pending.

An earlier operator statement attributed missing producer-receipt paths to a
concurrent rename. It is retracted: the problem was an incorrect filename
assumption, and there is no evidence another process renamed those receipts.
