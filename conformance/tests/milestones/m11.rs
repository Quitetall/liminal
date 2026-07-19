//! M11 exit-gate born-passing tests.
//!
//! M11.1: the NDJSON trace parser rejects unknown fields (D11.5).
//! M11.2: the labeled-fixture coalescing / session / git-delivery boundary
//! cases (D11.1–D11.3). `denominator_counts_golden` — the frozen golden
//! snapshot over these same fixtures — is M11.3, a T1 act, and is NOT
//! flipped here.

use camino::Utf8PathBuf;
use liminal_conformance::denominator::{self, SESSION_TIMEOUT_MS};
use liminal_conformance::trace::{
    Consent, EditRange, Trace, TraceEvent, TraceParseError, TraceSetup,
};

fn header() -> TraceEvent {
    TraceEvent::TraceHeader {
        version: 1,
        trace_id: "m11-parser-test".into(),
        source: "milestones/m11.rs".into(),
        capture_tool: "manual".into(),
        consent: Consent::Synthetic,
        profile: "external-file".into(),
        setup: TraceSetup {
            files: Vec::new(),
            graph: Vec::new(),
        },
    }
}

/// D11.5 (frozen): the parser REJECTS unknown fields. A well-formed
/// `buffer_edit` line carrying one stray, unrecognized key must fail to
/// parse — a corpus defect fails loudly, it is never silently dropped.
#[test]
fn trace_parser_rejects_unknown_fields() {
    let good_edit = TraceEvent::BufferEdit {
        at: 100,
        session: "s1".into(),
        buffer: "b1".into(),
        range: EditRange { start: 0, end: 0 },
        insert: "hi".into(),
    };
    let ndjson = format!(
        "{}\n{}",
        serde_json::to_string(&header()).unwrap(),
        serde_json::to_string(&good_edit).unwrap(),
    );
    Trace::parse(&ndjson).expect("the well-formed trace parses cleanly");

    let mut corrupted_edit = serde_json::to_value(&good_edit).unwrap();
    corrupted_edit
        .as_object_mut()
        .unwrap()
        .insert("unexpected_field".into(), serde_json::Value::Bool(true));
    let corrupted = format!(
        "{}\n{}",
        serde_json::to_string(&header()).unwrap(),
        corrupted_edit,
    );

    let err = Trace::parse(&corrupted).expect_err("an unknown field must be rejected");
    assert!(
        matches!(err, TraceParseError::Json { line: 2, .. }),
        "expected a line-2 JSON schema error, got {err:?}"
    );
}

/// Read one `conformance/fixtures/traces/labeled/<name>.trace.ndjson` fixture
/// (Algorithm D: the four hand-labeled denominator traces).
fn labeled(name: &str) -> String {
    let path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/traces/labeled")
        .join(format!("{name}.trace.ndjson"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// D11.1: 1999ms coalesces, 2000ms splits, an interleaved `save` splits
/// regardless of gap — all three boundary cases live in one buffer's
/// timeline in `coalesce-basic.trace.ndjson`.
#[test]
fn coalescing_boundary_cases() {
    let trace = Trace::parse(&labeled("coalesce-basic")).expect("fixture parses");
    let counts = denominator::count(&trace.events);
    assert_eq!(
        counts.txns, 3,
        "edit1->edit2 (gap 1999) coalesces; edit2->edit3 (gap 2000) splits; \
         edit3->edit4 (interleaved save) splits => 3 transactions"
    );
}

/// D11.2: a gap of exactly `SESSION_TIMEOUT_MS` (30 minutes) in a client's
/// activity, with no explicit `session_open`/`session_close`, closes the
/// session at the previous event and opens a new one.
#[test]
fn session_splits_at_exactly_30min() {
    let trace = Trace::parse(&labeled("session-timeout")).expect("fixture parses");
    let counts = denominator::count(&trace.events);
    assert_eq!(
        counts.sessions, 2,
        "a gap of exactly {SESSION_TIMEOUT_MS}ms splits into two sessions"
    );
}

/// Algorithm A: `git_op` counts `len(files)`; its accompanying
/// `file_change_external` delivery events (same `(cause_category,
/// cause_key)`) are the byte delivery of that already-counted op and must
/// contribute 0 each — never double-counted.
#[test]
fn git_delivery_events_not_double_counted() {
    let trace = Trace::parse(&labeled("git-rootcause")).expect("fixture parses");
    let counts = denominator::count(&trace.events);
    assert_eq!(
        counts.ops, 2,
        "1 git_op over 2 files = 2 ops; the 2 delivery file_change_external \
         events must add 0, never bringing the total to 4"
    );
}
