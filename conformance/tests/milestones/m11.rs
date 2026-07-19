//! M11 exit-gate born-passing tests.
//!
//! M11.1: the NDJSON trace parser rejects unknown fields (D11.5).

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
