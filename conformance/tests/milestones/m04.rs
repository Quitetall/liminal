//! M04 exit-gate tests: the two-step DAG crash matrix and the `lim repairs`
//! golden.
//!
//! ── STUB (M04.9, T3). Born-passing tests to be written once the M04 repair
//! engine (evaluate_repair conjunction, save-as-Promotion, InsertSourceId
//! executor, accept_repair) lands. See docs/execution/M04.md exit gate. ──
//!
//! These are `#[ignore]`d with a phase-tagged reason so the tree compiles and
//! the debt meter counts them. When the engine is real, DELETE the `#[ignore]`
//! and implement the body per the spec in each doc comment. Do NOT create these
//! born-passing until the engine exists — an ignore-flip here is the M04 close.

/// `crash_matrix_two_step_dag` (born-passing at M04): `ToyRun::crash_matrix`
/// over the `dag_accept` scenario — 5 boundary points with per-step
/// occurrences (two `after_external_apply`? no: one file step + one graph step,
/// so `after_external_apply:1` for the InsertSourceId, the RetargetRelation is a
/// Graph step landing at Finalize). Double-recovery world-digest equality holds
/// at every point. Requires `runnable_crash_scenarios()` RUNNABLE to include
/// `"dag_accept"`.
///
/// SPEC: mirror `crash_matrix_promote_single_step` in conformance/tests/crash.rs
/// but load the `dag_accept` scenario. Name it `crash_matrix_two_step_dag` and
/// place it in conformance/tests/crash.rs (crash_ prefix → serialized group),
/// NOT here — this module holds the non-crash born-passing test only.
#[test]
#[ignore = "Phase -1 M4: two-step DAG crash matrix (needs repair engine + accept_repair)"]
fn crash_matrix_two_step_dag_placeholder() {
    unimplemented!("see doc comment; real test is crash_matrix_two_step_dag in tests/crash.rs")
}

/// `repairs_output_golden` (born-passing at M04): drive a workspace to one
/// accepted disjoint save + one NeedsReview DAG proposal, then snapshot
/// `lim repairs` stdout (insta, UUID-redacted).
///
/// SPEC (M04 Algorithm E output format, D04.8 — no timestamps):
///   repair:<uuid>  <selected_rule>  <evidence-kind>  <n> steps  undo available|undone|no inverse
///   repair:<uuid>  needs review: <first ReviewReason>
///   repair:<uuid>  interrupted (<state kebab>) — run recovery
/// Sorted by repair id (UUIDv7 ⇒ chronological). Redact UUIDs with an insta
/// filter (`[uuid]`) so the golden is stable. Accept via `cargo insta review`
/// — that acceptance is a T1 act (a golden is spec).
#[test]
#[ignore = "Phase -1 M4: lim repairs golden (needs repair records + CLI)"]
fn repairs_output_golden() {
    unimplemented!("drive save + DAG proposal; insta snapshot `lim repairs` per Algorithm E")
}
