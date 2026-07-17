# Silence fixtures (sound-state cases)

**Populated by:** M3 (interpretive Jurisdiction checker, two profiles,
silent checker). Empty until then by design.

**Spec:** v4 Law 3E (zero sound-state noise), v4 §7.9 ("sound workspace →
exit 0, no output, no badge, no notification"), v4 §7.4 (sound-session
definition), R4 §2.3 (session SLOs), R4 §10 ("sound states are silent").

## What lives here

Sound-workspace cases, each consisting of:

- A workspace whose inputs stay within the declared profile invariants:
  required Holders available, identities meeting their declared grades,
  no unresolved foreign corruption or merge dispute injected (v4 §7.4 —
  soundness is *declared*, so silence tests checker ergonomics rather
  than hiding real boundary conditions).
- The three-part expected outcome: exit code `0`, **byte-empty** stdout,
  **byte-empty** stderr, and zero unexpected reconciliation items.

The assertion is byte-emptiness, not "no errors". A progress spinner, a
summary line, a friendly notice, or an "all good" badge is a
conformance failure. Diagnostics for unsound states exist elsewhere;
this directory pins the shape of *normal*, which is nothing.

## Consuming tests

`tests/phase_minus_1.rs::sound_states_are_silent` and the M3 test
`sound_workspace_is_silent` (asserts empty byte strings), backed by the
harness `assert_silent()` helper. The end-to-end sound session is
`../scenarios/sound_session.scenario.toml`; fixtures here cover
additional sound configurations (graph-only workspaces, healthy
relations at each identity grade, idle multi-client workspaces) beyond
that single scripted session.
