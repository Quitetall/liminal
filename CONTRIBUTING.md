# Contributing

Thanks for looking at Liminal. Some honesty up front: this is currently a
one-person research project in its riskiest phase, and the contribution
surface is unusual.

## The architecture is being falsified, not extended

Liminal is in Phase -1, the falsification laboratory (v4 Law 14, Part XXII).
The point of everything in this repository right now is to *disprove* the
architecture cheaply — to find out whether identity round-trips, mutation-local
repair, crash-recoverable cross-Holder mutation (ILRP), and concurrent-buffer
Basis selection actually work before anything is built on top of them. A
failed Phase -1 gate that forces a constitutional revision is a good outcome.

Consequences:

- **Unsolicited feature PRs will likely be declined until Phase 0 exits.**
  Not because they are bad, but because Law 14 forbids building on unretired
  risk, and v4 §125 forbids optimizing semantics that have not been frozen.
  This includes parsers, editors, CRDTs, performance work, new crates, and
  compiled Jurisdiction plans.
- **Bug reports against the conformance fixtures are gold.** If you can make
  a Phase -1 gate test fail — a crash-matrix case that loses data, an
  identity-torture case where a strategy claims more continuity than it
  demonstrates, a scenario where the checker speaks on a sound workspace or
  two dirty buffers leak into one Basis — that is exactly the falsification
  this phase exists for. File it with the fixture or a reproducible trace.
- Reports of spec ambiguity, contradictions between v4 and R4, or code that
  diverges from its cited section are equally welcome.

## Ground rules for any change

- `just ci` must pass (fmt, taplo, typos, clippy with `-Dwarnings`, nextest,
  doctests, rustdoc with `-Dwarnings`, cargo-deny).
- **Every public Rust item carries a doc comment citing its spec section**
  ("v4 §N" / "R4 §N"). If you cannot cite a section, the item probably should
  not exist yet.
- Never tune profiles or heuristics against the held-out corpora
  (`conformance/corpora/heldout/`, R4 §2.2). Their manifests are locked and
  CI-enforced; scores-seen means a new corpus version.
- A flaky crash-recovery test is a finding, never retried away
  (`.config/nextest.toml` sets retries = 0).

## Decision records

- **ADRs** (`docs/adr/`) record repository and implementation decisions:
  toolchain, layout, test stack, the hand-rolled toy store, and so on.
- **RFCs** (`spec/rfc/`) record anything spec-observable: changes to
  semantics, vocabulary, crate topology (see RFC-0001, which added
  `liminal-jurisdiction` to the v4 §117 layout), or conformance obligations.
  The canonical documents in `spec/v4/` are never edited; they are amended.

If a change needs neither, it is probably routine; if you are unsure, open an
issue first — it is cheaper for everyone.

## Licensing

Liminal is Apache-2.0. Unless you explicitly state otherwise, any
contribution intentionally submitted for inclusion is licensed as
Apache-2.0, without any additional terms or conditions. There is no CLA.
