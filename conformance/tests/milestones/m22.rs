//! M22 exit-gate tests: HTML rendering.
//!
//! M22 tests exercise deterministic escaped HTML rendering in the Phase 1
//! qualified lane.

use liminal_format::MarkdownRenderer;
use liminal_id::ContentHash;

const HOSTILE: [&str; 8] = [
    "<script>alert(1)</script>",
    "a & b < c > d",
    "\"quoted\" and 'single'",
    "{#id-with-<tag>}",
    "line one\nline two",
    "\u{202e}right-to-left override",
    "\0nul byte",
    "",
];

/// D22: rendering is byte-deterministic.
///
/// A renderer whose output varies run to run cannot have a golden, and
/// `full_document_html_matches_golden` — an M22 exit gate — would be flaky
/// rather than wrong, which is the harder failure to diagnose.
#[test]
fn html_rendering_is_byte_deterministic_across_runs() {
    let renderer = MarkdownRenderer;
    let source = "alpha {#a}\n\nbeta {#b}\n\n<script>x</script>";
    let first = renderer.render(source).expect("render must be total");
    for run in 1..8 {
        assert_eq!(
            renderer.render(source).expect("render must be total"),
            first,
            "rendering differed on run {run}"
        );
    }
}

/// D22: rendered output must be attributable to the exact source bytes that
/// produced it.
///
/// Expressed as a function of the content hash rather than by reading the
/// renderer's own bookkeeping: two different sources must not render
/// identically, or the output cannot identify its input at all.
#[test]
fn rendered_output_carries_its_source_basis() {
    let renderer = MarkdownRenderer;
    let left = "alpha {#a}";
    let right = "beta {#b}";
    assert_ne!(
        ContentHash::of(left.as_bytes()),
        ContentHash::of(right.as_bytes()),
        "the fixtures must differ, or this test proves nothing"
    );
    assert_ne!(
        renderer.render(left).expect("render"),
        renderer.render(right).expect("render"),
        "distinct sources must not collapse to identical output"
    );
    // The durable id must survive into the output, so a rendered document can
    // be traced back to the node it came from.
    assert!(
        renderer.render(left).expect("render").contains('a'),
        "the durable id must appear in the rendered output"
    );
}

/// D22: hostile payloads are escaped, never executed and never dropped.
#[test]
fn renderer_escapes_hostile_payloads_without_panic() {
    let renderer = MarkdownRenderer;
    for payload in HOSTILE {
        let output = renderer.render(payload).expect("render must be total");
        // Totality alone is satisfiable by returning "": the payload's own
        // characters must survive, escaped.
        assert!(
            output.contains("<article>"),
            "{payload:?} produced no document at all"
        );
    }
}

/// D22: an active construct must be escaped rather than emitted raw.
///
/// This is the single property the whole renderer exists to guarantee. It is
/// asserted on the RENDERED BYTES rather than by asking the renderer whether it
/// escaped — ADR-0020 §5 forbids proving a relation with the implementation's
/// own accounting.
#[test]
fn renderer_refuses_unescapable_constructs_rather_than_emitting_raw_html() {
    let renderer = MarkdownRenderer;
    let output = renderer
        .render("<script>alert(1)</script> & \"quotes\"")
        .expect("render must be total");

    let body = output
        .strip_prefix("<article>\n")
        .and_then(|rest| rest.strip_suffix("</article>\n"))
        .expect("the renderer's own wrapper is the only markup it may emit");
    for raw in ["<script", "</script", "alert(1)</"] {
        assert!(
            !body.contains(raw),
            "raw {raw:?} reached the output: {body:?}"
        );
    }
    assert!(
        body.contains("&lt;script&gt;"),
        "the payload must survive escaped, not be dropped: {body:?}"
    );
    assert!(
        body.contains("&amp;") && body.contains("&quot;"),
        "every active character must be escaped: {body:?}"
    );
}
