//! §7.4 denominator: Jurisdiction-sensitive-operation and session counting
//! over a parsed trace's event timeline (M11.2; Algorithm A + D11.1–D11.3).
//!
//! Pure trace-level accounting — no replay, no `ToyWorkspace`. Replaying
//! events through the M03/M05 runner to measure the D11.4 auto-resolution
//! NUMERATOR (checker output, diagnostics, reconciliation items) is
//! `pipeline.rs`'s job (M11.5); this module only computes the DENOMINATOR:
//! how many Jurisdiction-sensitive operations, semantic transactions,
//! sessions, and root-cause incidents the trace contains, full stop.
//!
//! Frozen by `denominator_counts_golden` (M11.3) BEFORE any profile tuning
//! (v4 §7.4; POLICY.md §6): "the operation and session denominators … are
//! fixed … *before* any profile tuning begins, so the metric cannot be
//! gamed by redefining what counts as an operation."

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::trace::{CauseCategory, TraceEvent};

/// D11.1 — the coalescing constant (§7.4 names the rule, not the constant;
/// frozen by the golden). Consecutive `buffer_edit` events on the SAME
/// buffer coalesce into one semantic transaction while the logical gap
/// between them is strictly less than this many ms AND no event of any
/// OTHER type intervenes anywhere in the trace between them. Boundary: a gap
/// of 1999 coalesces, a gap of 2000 splits, an interleaved `save` splits
/// regardless of gap.
pub const COALESCE_GAP_MS: u64 = 2_000;

/// D11.2 — the session-timeout constant, 30 minutes verbatim from §7.4.
/// Explicit `session_open`/`session_close` wins; otherwise first activity
/// opens a session, and a gap of at least this many ms in that session's own
/// activity closes it AT THE PREVIOUS EVENT.
pub const SESSION_TIMEOUT_MS: u64 = 1_800_000;

/// One resolved editing session (D11.2): either an explicit
/// `session_open`/`session_close` bracket, or a first-activity/timeout-
/// derived interval. `id` is the trace's own `session` field for a fully
/// explicit bracket, or `"<session>#<n>"` for the nth implicit sub-session
/// derived from one `session` field's activity stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpan {
    /// Span id (see type doc).
    pub id: String,
    /// First covered logical ms.
    pub start: u64,
    /// Last covered logical ms (inclusive).
    pub end: u64,
}

impl SessionSpan {
    /// Whether a logical timestamp falls inside this span (inclusive).
    #[must_use]
    pub fn contains(&self, at: u64) -> bool {
        at >= self.start && at <= self.end
    }
}

/// The frozen §7.4 counts for one trace (Algorithm A + D11.1–D11.3) — the
/// exact shape `denominator_counts_golden` (M11.3) snapshots per labeled
/// fixture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DenominatorCounts {
    /// Total Jurisdiction-sensitive operations (Algorithm A).
    pub ops: u64,
    /// Semantic editor transactions surviving D11.1 coalescing.
    pub txns: u64,
    /// Resolved editing sessions after D11.2 explicit/implicit derivation.
    pub sessions: u64,
    /// `<category>:<key>` (D11.3/D05.5 grammar) → visible-incident count:
    /// the number of DISTINCT `(session, key)` pairs that ever attributed to
    /// this cause. Workspace-stream events outside any session contribute to
    /// `ops` but never to this map (D11.2).
    pub incidents_by_cause: BTreeMap<String, u64>,
}

/// Count the §7.4 denominators over one trace's event timeline (everything
/// after the mandatory `trace_header`, i.e. [`crate::trace::Trace::events`]).
#[must_use]
pub fn count(events: &[TraceEvent]) -> DenominatorCounts {
    let txns = coalesce_transactions(events);
    let ops = count_ops(events, txns);
    let spans = resolve_sessions(events);
    let incidents_by_cause = incidents_by_cause(events, &spans);
    DenominatorCounts {
        ops,
        txns,
        sessions: spans.len() as u64,
        incidents_by_cause,
    }
}

/// D11.1: fold the trace's `buffer_edit` events into semantic transactions.
///
/// Per buffer, a `buffer_edit` continues the buffer's currently open
/// transaction iff (a) the logical gap since that buffer's last edit is
/// strictly less than [`COALESCE_GAP_MS`], AND (b) no event of a type other
/// than `buffer_edit` has occurred anywhere in the trace since that last
/// edit (tracked via a global "barrier" counter incremented by every
/// non-`buffer_edit` event — an edit on a DIFFERENT buffer does not
/// increment it, since it is still type `buffer_edit`, and two buffers'
/// transactions are independent). Otherwise the edit starts a new
/// transaction.
fn coalesce_transactions(events: &[TraceEvent]) -> u64 {
    transaction_starts(events).len() as u64
}

/// The event indices at which a new semantic transaction STARTS (D11.1) —
/// the per-event attribution [`per_event_ops`] uses (each transaction is one
/// op, carried by its opening `buffer_edit`). `len()` == the frozen `txns`
/// count.
#[must_use]
pub fn transaction_starts(events: &[TraceEvent]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut barrier: u64 = 0;
    // buffer -> (at of last edit, barrier value at that edit)
    let mut open: HashMap<&str, (u64, u64)> = HashMap::new();

    for (i, event) in events.iter().enumerate() {
        match event {
            TraceEvent::BufferEdit { at, buffer, .. } => {
                let starts_new = match open.get(buffer.as_str()) {
                    Some(&(last_at, last_barrier)) => {
                        last_barrier != barrier || at.saturating_sub(last_at) >= COALESCE_GAP_MS
                    }
                    None => true,
                };
                if starts_new {
                    starts.push(i);
                }
                open.insert(buffer.as_str(), (*at, barrier));
            }
            _ => barrier += 1,
        }
    }
    starts
}

/// Per-event Jurisdiction-sensitive-operation attribution (Algorithm A +
/// D11.1), index-aligned with `events`: entry `i` is the number of ops event
/// `i` contributes — Algorithm A's per-row count, plus one for each
/// `buffer_edit` that OPENS a semantic transaction. The sum equals the
/// frozen [`count`]`.ops` (unit-asserted below; the golden guards the total).
#[must_use]
pub fn per_event_ops(events: &[TraceEvent]) -> Vec<u64> {
    let mut per_event = base_event_ops(events);
    for i in transaction_starts(events) {
        per_event[i] += 1;
    }
    per_event
}

/// Algorithm A: every event type's Jurisdiction-sensitive-operation count.
/// `txns` is the D11.1-coalesced transaction count (each = 1 op).
fn count_ops(events: &[TraceEvent], txns: u64) -> u64 {
    base_event_ops(events).iter().sum::<u64>() + txns
}

/// Algorithm A's per-row op counts, index-aligned with `events` — every row
/// EXCEPT the transaction ops (which [`per_event_ops`] layers on top).
fn base_event_ops(events: &[TraceEvent]) -> Vec<u64> {
    // Every git_op's own (cause_category, cause_key) — a file_change_external
    // sharing one of these is the BYTE DELIVERY of that already-counted op,
    // not a new one.
    let git_causes: BTreeSet<(CauseCategory, &str)> = events
        .iter()
        .filter_map(|e| match e {
            TraceEvent::GitOp {
                cause_category,
                cause_key,
                ..
            } => Some((*cause_category, cause_key.as_str())),
            _ => None,
        })
        .collect();

    // holder_available: "1 per Overlay reconciliation attempted on Holder
    // return". Phase -1's only Holder is the file path (D05.1:
    // `SYS_UNAVAILABLE[path]`; runner::holder_available(store, path)), so the
    // trace's `holder` field IS the path. An offline `save` creates (or
    // D05.3-coalesces into) exactly ONE Overlay per subject; the count on
    // return is the number of DISTINCT paths that received a save while that
    // Holder was unavailable and have not yet been reconciled. Recorded as
    // DG-11.1 (M11.md Discovered gaps) — this is a trace-level reading, not
    // spelled out verbatim by the order; pipeline.rs's actual replay (M11.5)
    // is the authoritative cross-check before the golden freezes (M11.3).
    let mut buffer_path: HashMap<&str, &str> = HashMap::new();
    let mut unavailable: BTreeSet<&str> = BTreeSet::new();
    let mut pending_draft: BTreeSet<&str> = BTreeSet::new();

    let mut per_event = Vec::with_capacity(events.len());
    for event in events {
        per_event.push(match event {
            TraceEvent::TraceHeader { .. }
            | TraceEvent::SessionOpen { .. }
            | TraceEvent::SessionClose { .. }
            | TraceEvent::ClockAdvance { .. }
            | TraceEvent::BufferEdit { .. } => 0,
            TraceEvent::BufferOpen { buffer, path, .. } => {
                buffer_path.insert(buffer.as_str(), path.as_str());
                1
            }
            TraceEvent::Save { buffer, .. } => {
                if let Some(&path) = buffer_path.get(buffer.as_str())
                    && unavailable.contains(path)
                {
                    pending_draft.insert(path);
                }
                1
            }
            TraceEvent::FileChangeExternal {
                cause_category,
                cause_key,
                ..
            } => u64::from(!git_causes.contains(&(*cause_category, cause_key.as_str()))),
            TraceEvent::GitOp { files, .. } => files.len() as u64,
            TraceEvent::FormatterRewrite { .. } | TraceEvent::ResolverObserve { .. } => 1,
            TraceEvent::HolderUnavailable { holder, .. } => {
                unavailable.insert(holder.as_str());
                0
            }
            TraceEvent::HolderAvailable { holder, .. } => {
                unavailable.remove(holder.as_str());
                u64::from(pending_draft.remove(holder.as_str()))
            }
        });
    }
    per_event
}

/// D11.2: derive the trace's resolved sessions.
///
/// For each distinct `session` id referenced by a session/buffer-scoped
/// event: if BOTH an explicit `session_open` and a matching `session_close`
/// exist, that bracket wins outright — one session, with no internal
/// splitting even across a long internal gap. Otherwise the id is resolved
/// implicitly: first activity opens it, and any gap of at least
/// [`SESSION_TIMEOUT_MS`] between consecutive activity timestamps closes it
/// at the earlier one and opens a fresh implicit sub-session at the later
/// one. A present `session_open` (without a close) or `session_close`
/// (without an open) is used as a hard boundary on its respective end; this
/// partial-bracket case is not spelled out verbatim by the order (M11.md
/// Discovered gaps, DG-11.2).
///
/// Public as [`session_spans`] for the M11.5 pipeline's session-attributed
/// metrics; `len()` == the frozen `sessions` count.
#[must_use]
pub fn session_spans(events: &[TraceEvent]) -> Vec<SessionSpan> {
    resolve_sessions(events)
}

/// See [`session_spans`].
fn resolve_sessions(events: &[TraceEvent]) -> Vec<SessionSpan> {
    let mut open_at: HashMap<&str, u64> = HashMap::new();
    let mut close_at: HashMap<&str, u64> = HashMap::new();
    for event in events {
        match event {
            TraceEvent::SessionOpen { at, session, .. } => {
                open_at.insert(session.as_str(), *at);
            }
            TraceEvent::SessionClose { at, session } => {
                close_at.insert(session.as_str(), *at);
            }
            _ => {}
        }
    }

    // Per session-id activity: every session/buffer-scoped event carrying
    // that id, in trace order.
    let mut activity: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for event in events {
        if let (Some(session), Some(at)) = (event.session(), event.at()) {
            activity.entry(session).or_default().push(at);
        }
    }

    let mut spans = Vec::new();
    for (id, ats) in activity {
        let has_open = open_at.contains_key(id);
        let has_close = close_at.contains_key(id);

        if has_open && has_close {
            spans.push(SessionSpan {
                id: id.to_owned(),
                start: open_at[id],
                end: close_at[id],
            });
            continue;
        }

        let mut seq = 0u32;
        let mut run_start = if has_open { open_at[id] } else { ats[0] };
        let mut prev = ats[0];
        for &at in ats.iter().skip(1) {
            if at.saturating_sub(prev) >= SESSION_TIMEOUT_MS {
                spans.push(SessionSpan {
                    id: format!("{id}#{seq}"),
                    start: run_start,
                    end: prev,
                });
                seq += 1;
                run_start = at;
            }
            prev = at;
        }
        let end = if has_close { close_at[id] } else { prev };
        spans.push(SessionSpan {
            id: format!("{id}#{seq}"),
            start: run_start,
            end,
        });
    }
    spans
}

/// D11.3: root-cause coalescing. Every root-cause-capable event
/// ([`TraceEvent::cause`]) attaches to every session span active at its
/// `at` (D11.2: "workspace-stream events attach to every session active at
/// their `at`"); all such attributions sharing one `(session, key)` pair
/// coalesce into ONE visible incident. An event outside every span
/// contributes to the corpus-wide op denominator only (already folded into
/// [`count_ops`]) — never to this map.
fn incidents_by_cause(events: &[TraceEvent], spans: &[SessionSpan]) -> BTreeMap<String, u64> {
    let mut attributed: BTreeSet<(String, &str)> = BTreeSet::new();
    for event in events {
        let (Some((category, key)), Some(at)) = (event.cause(), event.at()) else {
            continue;
        };
        let full_key = format!("{category}:{key}");
        for span in spans {
            if span.contains(at) {
                attributed.insert((full_key.clone(), span.id.as_str()));
            }
        }
    }
    let mut out = BTreeMap::new();
    for (key, _session) in attributed {
        *out.entry(key).or_insert(0u64) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::Trace;
    use camino::Utf8PathBuf;

    fn labeled_dir() -> Utf8PathBuf {
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/traces/labeled")
    }

    /// The M11.5 per-event exposure is an attribution VIEW over the same
    /// frozen counting: `per_event_ops` sums to `count().ops` and
    /// `session_spans` has `count().sessions` entries, on every labeled
    /// fixture (whose totals the M11.3 golden freezes).
    #[test]
    fn per_event_attribution_sums_to_frozen_totals() {
        for name in [
            "coalesce-basic",
            "session-timeout",
            "git-rootcause",
            "offline-transient",
        ] {
            let path = labeled_dir().join(format!("{name}.trace.ndjson"));
            let ndjson = std::fs::read_to_string(&path).expect("fixture readable");
            let trace = Trace::parse(&ndjson).expect("fixture parses");
            let counts = count(&trace.events);
            assert_eq!(
                per_event_ops(&trace.events).iter().sum::<u64>(),
                counts.ops,
                "{name}: per-event ops must sum to the frozen total"
            );
            assert_eq!(
                session_spans(&trace.events).len() as u64,
                counts.sessions,
                "{name}: session spans must match the frozen session count"
            );
        }
    }

    /// Smoke test: all four hand-labeled denominator fixtures (Algorithm D)
    /// parse and produce a `DenominatorCounts` without panicking. The exact
    /// numbers are frozen by `denominator_counts_golden` at M11.3, not here.
    #[test]
    fn labeled_fixtures_all_parse_and_count() {
        for name in [
            "coalesce-basic",
            "session-timeout",
            "git-rootcause",
            "offline-transient",
        ] {
            let path = labeled_dir().join(format!("{name}.trace.ndjson"));
            let ndjson = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{path}: must exist and be readable: {e}"));
            let trace =
                Trace::parse(&ndjson).unwrap_or_else(|e| panic!("{path}: must parse cleanly: {e}"));
            // Every labeled fixture must exercise at least one event beyond
            // the header — an empty timeline can't label anything.
            assert!(
                !trace.events.is_empty(),
                "{path}: has no events past the header"
            );
            let _ = count(&trace.events);
        }
    }

    #[test]
    fn coalesce_gap_boundary_1999_coalesces_2000_splits() {
        let events = vec![
            TraceEvent::BufferOpen {
                at: 0,
                session: "s1".into(),
                client: "dev".into(),
                buffer: "b1".into(),
                path: "notes.md".into(),
            },
            TraceEvent::BufferEdit {
                at: 1_000,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 0, end: 0 },
                insert: "a".into(),
            },
            TraceEvent::BufferEdit {
                at: 2_999, // gap 1999 from prior edit -> coalesces
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 1, end: 1 },
                insert: "b".into(),
            },
            TraceEvent::BufferEdit {
                at: 4_999, // gap 2000 from prior edit -> splits
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 2, end: 2 },
                insert: "c".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(counts.txns, 2, "1999ms coalesces, 2000ms splits");
    }

    #[test]
    fn interleaved_save_splits_regardless_of_gap() {
        let events = vec![
            TraceEvent::BufferEdit {
                at: 0,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 0, end: 0 },
                insert: "a".into(),
            },
            TraceEvent::Save {
                at: 50,
                session: "s1".into(),
                buffer: "b1".into(),
            },
            TraceEvent::BufferEdit {
                at: 100, // gap 100ms would coalesce, but `save` intervened
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 1, end: 1 },
                insert: "b".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(
            counts.txns, 2,
            "an interleaved save splits regardless of gap"
        );
    }

    #[test]
    fn session_timeout_boundary_splits_at_exactly_30_min() {
        let events = vec![
            TraceEvent::BufferOpen {
                at: 0,
                session: "s1".into(),
                client: "dev".into(),
                buffer: "b1".into(),
                path: "notes.md".into(),
            },
            TraceEvent::BufferEdit {
                at: 100,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 0, end: 0 },
                insert: "a".into(),
            },
            TraceEvent::BufferEdit {
                at: 100 + SESSION_TIMEOUT_MS, // gap == 30min exactly -> splits
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 1, end: 1 },
                insert: "b".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(
            counts.sessions, 2,
            "a gap of exactly 30min splits (D11.2: >=30min)"
        );
    }

    #[test]
    fn session_timeout_just_under_30_min_does_not_split() {
        let events = vec![
            TraceEvent::BufferOpen {
                at: 0,
                session: "s1".into(),
                client: "dev".into(),
                buffer: "b1".into(),
                path: "notes.md".into(),
            },
            TraceEvent::BufferEdit {
                at: 100,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 0, end: 0 },
                insert: "a".into(),
            },
            TraceEvent::BufferEdit {
                at: 100 + SESSION_TIMEOUT_MS - 1,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 1, end: 1 },
                insert: "b".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(counts.sessions, 1, "a gap of 29:59.999 does not split");
    }

    #[test]
    fn explicit_session_bracket_survives_a_large_internal_gap() {
        let events = vec![
            TraceEvent::SessionOpen {
                at: 0,
                session: "s1".into(),
                client: "dev".into(),
            },
            TraceEvent::BufferOpen {
                at: 10,
                session: "s1".into(),
                client: "dev".into(),
                buffer: "b1".into(),
                path: "notes.md".into(),
            },
            TraceEvent::BufferEdit {
                at: 20,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 0, end: 0 },
                insert: "a".into(),
            },
            TraceEvent::BufferEdit {
                at: 20 + SESSION_TIMEOUT_MS * 4,
                session: "s1".into(),
                buffer: "b1".into(),
                range: crate::trace::EditRange { start: 1, end: 1 },
                insert: "b".into(),
            },
            TraceEvent::SessionClose {
                at: 20 + SESSION_TIMEOUT_MS * 5,
                session: "s1".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(
            counts.sessions, 1,
            "explicit open/close wins even across a >=30min internal gap"
        );
    }

    #[test]
    fn git_op_delivery_events_are_not_double_counted() {
        let events = vec![
            TraceEvent::GitOp {
                at: 60_000,
                op: crate::trace::GitOpKind::Commit,
                files: vec!["a.md".into(), "b.md".into()],
                cause_category: CauseCategory::Git,
                cause_key: "commit@60000".into(),
            },
            TraceEvent::FileChangeExternal {
                at: 60_001,
                path: "a.md".into(),
                contents: "a\n".into(),
                cause_category: CauseCategory::Git,
                cause_key: "commit@60000".into(),
            },
            TraceEvent::FileChangeExternal {
                at: 60_002,
                path: "b.md".into(),
                contents: "b\n".into(),
                cause_category: CauseCategory::Git,
                cause_key: "commit@60000".into(),
            },
        ];
        let counts = count(&events);
        assert_eq!(
            counts.ops, 2,
            "git_op counts len(files)=2 once; its delivery file_change_external \
             events must contribute 0 each, never 4 total"
        );
        assert!(
            counts.incidents_by_cause.is_empty(),
            "these events fall outside every session (none is open) — corpus-wide \
             op denominator only, no session metric (D11.2)"
        );
    }

    #[test]
    fn unmatched_file_change_external_counts_as_its_own_op() {
        let events = vec![TraceEvent::FileChangeExternal {
            at: 10,
            path: "a.md".into(),
            contents: "a\n".into(),
            cause_category: CauseCategory::Foreign,
            cause_key: "foreign@10".into(),
        }];
        let counts = count(&events);
        assert_eq!(counts.ops, 1);
    }
}
