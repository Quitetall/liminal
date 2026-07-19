//! The §-1.5 scorecard pipeline (M11 Algorithm C; D11.4).
//!
//! `run(corpus_dir, profile)` replays every matching `*.trace.ndjson`
//! through the ONE M03/M05 runner (`liminal_daemon::runner::StepRunner`,
//! AM-11.4 — never a second implementation of save/foreign-edit semantics),
//! collects observables PER EVENT (checker findings + rendered bytes,
//! reconciliation items expected-vs-unexpected, overlay dispositions), and
//! applies Algorithm A + D11.1–D11.4 to produce the nine frozen
//! [`Scorecard`] fields plus the side [`TransientDraftReport`].
//!
//! Event → runner mapping (Algorithm C step 1, verbatim):
//! `session_open`→(accounting only), `buffer_open`→`open_buffer`,
//! `buffer_edit`→`buffer_edit` step, `save`→`save` step,
//! `file_change_external`|`formatter_rewrite`→`foreign_edit` step,
//! `holder_*`→`holder_(un)available` steps (set/clear + sweep),
//! `resolver_observe`→`ToyWorkspace::inject_observation`,
//! `clock_advance`→`advance_clock` step (delta from the previous event's
//! logical `at`).
//!
//! Soundness is INPUT-based (v4 §7.4: "inputs remain within the declared
//! profile invariants"), projected mechanically onto the toy: a session (or
//! the whole workspace) is unsound once (a) a Holder-unavailable interval
//! overlaps it, (b) a foreign change lands on a path some open buffer has
//! edited without saving (an injected merge dispute), or (c) a foreign
//! event's ingestion surfaces new identity findings (injected foreign
//! corruption, detected by the checker on the post-ingest world). Checker
//! findings in sessions that stay sound count as sound-session diagnostics;
//! rendered checker bytes accumulate into `sound_checker_output_bytes`
//! while the workspace prefix is still entirely sound (Law 3E is asserted
//! at every observation point).

use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use liminal_daemon::reactor::Observation;
use liminal_daemon::runner::{StepRunner, sweep_overlays};
use liminal_daemon::scenario::{Setup, SetupFile, Step};
use liminal_daemon::{FsExecutor, ToyWorkspace};
use liminal_graph::ns;
use liminal_id::{SourceId, Timestamp};
use liminal_jurisdiction::{NoCrash, Overlay, OverlayState, ReconciliationQueue};
use liminal_revision::{BasisPerspective, WorkspaceBasis};
use serde::Serialize;

use crate::denominator;
use crate::slo::Scorecard;
use crate::trace::{Trace, TraceEvent};

/// D11.4's side report: profile-declared transient drafts (saves during
/// `holder_unavailable`) are EXCLUDED from the auto-resolution numerator
/// and denominator and accounted here instead — they must reconcile within
/// their policy window or become debt, never silently count as resolved
/// (R4 §2.3). A SIDE struct by decree: the nine `Scorecard` fields are
/// frozen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TransientDraftReport {
    /// Declared-transient drafts created (distinct Overlays; D05.3-coalesced
    /// re-saves stay ONE draft).
    pub declared: u64,
    /// Drafts reconciled on Holder return WITHOUT ever aging past the
    /// policy window.
    pub reconciled_within_window: u64,
    /// Drafts that aged past their window into visible debt (the coalesced
    /// `offline:` reconciliation item).
    pub aged_into_debt: u64,
}

/// Everything one trace's replay contributes to the aggregate scorecard.
#[derive(Debug, Clone, Default)]
pub struct TraceReplay {
    /// Total Algorithm A ops (== the frozen denominator).
    pub ops: u64,
    /// Ops excluded as declared transient drafts (D11.4).
    pub excluded_ops: u64,
    /// Ops whose replay surfaced a failure (unexpected item, new finding,
    /// or undeclared Overlay) — not automatically resolved.
    pub failed_ops: u64,
    /// Session count (D11.2).
    pub sessions: u64,
    /// Sessions with no intervention surface.
    pub intervention_free_sessions: u64,
    /// New findings observed during events inside sessions that stayed
    /// sound.
    pub sound_session_diagnostics: u64,
    /// Unexpected reconciliation items created during events inside sound
    /// sessions.
    pub unexpected_items_in_sound_sessions: u64,
    /// Rendered checker bytes observed while the workspace prefix was
    /// entirely sound.
    pub sound_checker_output_bytes: u64,
    /// Worst visible-incident count for one `(session, root cause)` pair
    /// (D11.3).
    pub max_visible_incidents_per_root_cause: u64,
    /// The side transient-draft accounting.
    pub drafts: TransientDraftReport,
    /// Per holder_available event index: Overlay reconciliations actually
    /// attempted on Holder return (observed from replay — the DG-11.1
    /// cross-check reads this).
    pub reconciliation_attempts_by_event: BTreeMap<usize, u64>,
}

/// One immutable observable snapshot between events.
#[derive(Debug, Clone, Default)]
struct Snapshot {
    /// item id → root_cause.
    items: BTreeMap<String, String>,
    /// overlay aux key → (state, declared_transient).
    overlays: BTreeMap<String, (OverlayState, bool)>,
    /// Rendered checker lines (the `lim check` byte format).
    findings: Vec<String>,
}

fn snapshot(ws: &ToyWorkspace) -> anyhow::Result<Snapshot> {
    let store = ws.store();
    let mut items = BTreeMap::new();
    for item in (ReconciliationQueue { store }).items()? {
        items.insert(item.id.to_string(), item.root_cause);
    }
    let mut overlays = BTreeMap::new();
    for (key, value) in store.scan_aux(ns::JUR_OVERLAY)? {
        let overlay: Overlay = serde_json::from_value(value)?;
        overlays.insert(key, (overlay.state, overlay.lifecycle.declared_transient));
    }
    // The same check `lim check` runs (DurableOnly, empty components) with
    // the same one-line-per-finding byte format.
    let basis = WorkspaceBasis {
        transaction: liminal_id::TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    let report = ws
        .checker()
        .check_workspace(&basis)
        .map_err(|e| anyhow::anyhow!("checker: {e}"))?;
    let findings = report
        .findings
        .iter()
        .map(|f| {
            let subject = ws
                .alias_for(f.subject)
                .unwrap_or_else(|| f.subject.to_string());
            format!(
                "{}: {subject} [{}]\n",
                liminal_jurisdiction::everyday_rendering(f.code),
                f.code
            )
        })
        .collect();
    Ok(Snapshot {
        items,
        overlays,
        findings,
    })
}

/// Build a one-entry `Step` table.
fn step(kind: &str, fields: &[(&str, toml::Value)]) -> Step {
    let mut extra = toml::Table::new();
    for (k, v) in fields {
        extra.insert((*k).to_owned(), v.clone());
    }
    Step {
        kind: kind.to_owned(),
        extra,
    }
}

fn s(v: &str) -> toml::Value {
    toml::Value::String(v.to_owned())
}

/// A fresh scratch workspace root for one trace replay.
fn scratch_root(trace_id: &str) -> anyhow::Result<Utf8PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = Utf8PathBuf::from_path_buf(std::env::temp_dir())
        .map_err(|p| anyhow::anyhow!("non-UTF-8 temp dir: {}", p.display()))?;
    let clean: String = trace_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dir = base.join(format!(
        "liminal-pipeline/{}-{}-{clean}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Holder-unavailable intervals `[from, to]` per the trace's own timeline
/// (an interval left open runs to `u64::MAX`).
fn holder_intervals(events: &[TraceEvent]) -> Vec<(u64, u64)> {
    let mut intervals = Vec::new();
    let mut open: BTreeMap<&str, u64> = BTreeMap::new();
    for event in events {
        match event {
            TraceEvent::HolderUnavailable { at, holder, .. } => {
                open.entry(holder.as_str()).or_insert(*at);
            }
            TraceEvent::HolderAvailable { at, holder } => {
                if let Some(from) = open.remove(holder.as_str()) {
                    intervals.push((from, *at));
                }
            }
            _ => {}
        }
    }
    for (_, from) in open {
        intervals.push((from, u64::MAX));
    }
    intervals
}

/// Replay ONE parsed trace through the runner, collecting per-event
/// observables (Algorithm C steps 1–2) and folding them into the D11.4
/// accounting (step 3).
///
/// # Errors
/// Replay/setup/IO failures, or a structurally invalid trace (e.g. an edit
/// on an unopened buffer, an out-of-bounds splice) — corpus defects fail
/// loudly (D11.5 spirit).
#[allow(clippy::too_many_lines)] // one linear pass per event kind; splitting would scatter the accounting state
pub fn replay_trace(trace: &Trace) -> anyhow::Result<TraceReplay> {
    let TraceEvent::TraceHeader {
        trace_id, setup, ..
    } = &trace.header
    else {
        anyhow::bail!("trace has no header");
    };
    let events = &trace.events;

    // ── Runner setup from the header (Algorithm C step 1). ──
    let root = scratch_root(trace_id)?;
    let runner_setup = Setup {
        files: setup
            .files
            .iter()
            .map(|f| SetupFile {
                path: f.path.clone(),
                text: f.contents.clone(),
            })
            .collect(),
        graph: setup.graph.clone(),
        buffers: Vec::new(),
    };
    let mut runner =
        StepRunner::open(&root, &runner_setup, FsExecutor::new(root.clone()), NoCrash)?;

    // ── Trace-level accounting state. ──
    let per_event_ops = denominator::per_event_ops(events);
    let spans = denominator::session_spans(events);
    let mut span_unsound = vec![false; spans.len()];
    for (from, to) in holder_intervals(events) {
        for (i, span) in spans.iter().enumerate() {
            if from <= span.end && to >= span.start {
                span_unsound[i] = true;
            }
        }
    }

    let mut buffer_info: BTreeMap<String, (String, String)> = BTreeMap::new(); // buffer → (client, path)
    let mut buffer_text: BTreeMap<String, String> = BTreeMap::new();
    let mut dirty_paths: BTreeMap<String, u64> = BTreeMap::new(); // path → dirty-buffer count
    let mut unavailable: BTreeSet<String> = BTreeSet::new();
    let mut declared_paths: BTreeSet<String> = BTreeSet::new();
    let mut prev_at = 0u64;
    let mut prefix_sound = true;

    let mut out = TraceReplay {
        ops: per_event_ops.iter().sum(),
        sessions: spans.len() as u64,
        ..TraceReplay::default()
    };
    // (span index, root cause) → distinct visible surfaces (item ids).
    let mut incidents: BTreeMap<(usize, String), BTreeSet<String>> = BTreeMap::new();
    // span index → an intervention surface appeared.
    let mut span_intervention = vec![false; spans.len()];
    // span index → diagnostics observed (summed only if the span stays sound).
    let mut span_diags = vec![0u64; spans.len()];
    let mut span_unexpected_items = vec![0u64; spans.len()];
    // overlay key → (declared_transient, aged into debt).
    let mut draft_state: BTreeMap<String, bool> = BTreeMap::new();

    let mut prev = snapshot(runner.workspace())?;

    for (idx, event) in events.iter().enumerate() {
        let at = event.at().unwrap_or(prev_at);
        let mut excluded_here = 0u64;
        let mut foreign_paths: Vec<String> = Vec::new();

        // ── Step 1: apply the event through the runner. ──
        match event {
            TraceEvent::TraceHeader { .. } => anyhow::bail!("header inside the timeline"),
            TraceEvent::SessionOpen { .. } | TraceEvent::SessionClose { .. } => {}
            TraceEvent::BufferOpen {
                client,
                buffer,
                path,
                ..
            } => {
                runner.open_buffer(client, path)?;
                let bytes = std::fs::read_to_string(root.join(path))?;
                buffer_info.insert(buffer.clone(), (client.clone(), path.clone()));
                buffer_text.insert(buffer.clone(), bytes);
            }
            TraceEvent::BufferEdit {
                buffer,
                range,
                insert,
                ..
            } => {
                let (client, path) = buffer_info
                    .get(buffer)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("buffer_edit on unopened buffer {buffer}"))?;
                let old = buffer_text
                    .get(buffer)
                    .cloned()
                    .expect("info and text maps agree");
                let (start, end) = (usize::try_from(range.start)?, usize::try_from(range.end)?);
                anyhow::ensure!(
                    start <= end && end <= old.len(),
                    "buffer_edit splice out of bounds ({start}..{end} of {})",
                    old.len()
                );
                let new = format!("{}{}{}", &old[..start], insert, &old[end..]);
                runner.step(&step(
                    "buffer_edit",
                    &[
                        ("client", s(&client)),
                        ("path", s(&path)),
                        ("find", s(&old)),
                        ("replace", s(&new)),
                    ],
                ))?;
                buffer_text.insert(buffer.clone(), new);
                *dirty_paths.entry(path).or_insert(0) += 1;
            }
            TraceEvent::Save { buffer, .. } => {
                let (client, path) = buffer_info
                    .get(buffer)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("save on unopened buffer {buffer}"))?;
                if unavailable.contains(&path) {
                    // D11.4: a profile-declared transient draft — excluded
                    // from numerator AND denominator.
                    excluded_here = per_event_ops[idx];
                    declared_paths.insert(path.clone());
                }
                runner.step(&step("save", &[("client", s(&client)), ("path", s(&path))]))?;
                dirty_paths.remove(&path);
            }
            TraceEvent::FileChangeExternal { path, contents, .. }
            | TraceEvent::FormatterRewrite { path, contents, .. } => {
                foreign_paths.push(path.clone());
                // DG-11.3: an imported real history can ADD a file (in a
                // directory that never existed at setup). A foreign add is
                // an edit from nothing: create parents + an empty file, and
                // the `find:""` whole-content replace below lands the bytes.
                //
                // DG-11.4: real histories also transition a path dir→file
                // and file→dir. Deletions are never replayed, so a stale
                // counterpart can block the write: a directory sitting where
                // the file must land, or a file sitting where an ancestor
                // directory must exist. The history says the path is a file
                // NOW — clear the stale shape and proceed.
                let abs = root.join(path);
                if abs.is_dir() {
                    std::fs::remove_dir_all(&abs)?;
                }
                let mut ancestor = abs.parent();
                while let Some(a) = ancestor {
                    if a == root {
                        break;
                    }
                    if a.is_file() {
                        std::fs::remove_file(a)?;
                    }
                    ancestor = a.parent();
                }
                if !abs.exists() {
                    if let Some(parent) = abs.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&abs, b"")?;
                }
                let current = std::fs::read_to_string(&abs)?;
                runner.step(&step(
                    "foreign_edit",
                    &[
                        ("path", s(path)),
                        ("find", s(&current)),
                        ("replace", s(contents)),
                    ],
                ))?;
            }
            TraceEvent::GitOp { files, .. } => {
                // The op is counted here; its byte deliveries follow as
                // file_change_external events with the same cause pair.
                foreign_paths.extend(files.iter().cloned());
            }
            TraceEvent::HolderUnavailable { holder, .. } => {
                unavailable.insert(holder.clone());
                runner.step(&step("holder_unavailable", &[("path", s(holder))]))?;
            }
            TraceEvent::HolderAvailable { holder, .. } => {
                unavailable.remove(holder);
                runner.step(&step("holder_available", &[("path", s(holder))]))?;
            }
            TraceEvent::ClockAdvance { at } => {
                let by_secs = i64::try_from(at.saturating_sub(prev_at) / 1000)?;
                runner.step(&step(
                    "advance_clock",
                    &[("by_secs", toml::Value::Integer(by_secs))],
                ))?;
            }
            TraceEvent::ResolverObserve {
                at,
                source,
                observed,
                ..
            } => {
                let payload = serde_json::from_str(observed)
                    .unwrap_or_else(|_| serde_json::Value::String(observed.clone()));
                runner.workspace_mut().inject_observation(Observation {
                    source: SourceId::from_name(source),
                    observed_at: Timestamp(i64::try_from(*at)?),
                    payload,
                })?;
                // Parity with the runner's post-step sweep (Algorithm B runs
                // after every scenario step).
                sweep_overlays(runner.workspace().store(), &root)?;
            }
        }

        // ── Step 2: collect observables and diff. ──
        let cur = snapshot(runner.workspace())?;

        let new_items: Vec<(&String, &String)> = cur
            .items
            .iter()
            .filter(|(id, _)| !prev.items.contains_key(*id))
            .collect();
        let prev_findings: BTreeSet<&String> = prev.findings.iter().collect();
        let new_findings: Vec<&String> = cur
            .findings
            .iter()
            .filter(|l| !prev_findings.contains(l))
            .collect();
        let new_overlays: Vec<(&String, &(OverlayState, bool))> = cur
            .overlays
            .iter()
            .filter(|(key, _)| !prev.overlays.contains_key(*key))
            .collect();

        // Expected items: the declared-transient aging item ONLY.
        let is_expected = |cause: &str| {
            declared_paths
                .iter()
                .any(|p| cause == format!("offline:path:{p}"))
        };
        let unexpected_items: Vec<&String> = new_items
            .iter()
            .filter(|(_, cause)| !is_expected(cause))
            .map(|(id, _)| *id)
            .collect();

        // Declared overlays: created by THIS save on an unavailable path.
        let is_offline_save = excluded_here > 0;
        let mut undeclared_overlay = false;
        for (key, (_state, declared_transient)) in &new_overlays {
            if *declared_transient && is_offline_save {
                out.drafts.declared += 1;
                draft_state.insert((*key).clone(), false);
            } else {
                undeclared_overlay = true;
            }
        }

        // Transient-draft aging + reconciliation accounting.
        for (id, cause) in &new_items {
            let _ = id;
            if is_expected(cause) {
                // The aging item marks every still-live declared draft on
                // that path as aged.
                for (key, aged) in &mut draft_state {
                    if cur.overlays.contains_key(key) && !*aged {
                        *aged = true;
                        out.drafts.aged_into_debt += 1;
                    }
                }
            }
        }
        let mut attempts_here = 0u64;
        for (key, (state, _)) in &prev.overlays {
            let was_active = *state == OverlayState::Active;
            let now_gone = !cur.overlays.contains_key(key);
            let now_changed = cur
                .overlays
                .get(key)
                .is_some_and(|(now_state, _)| now_state != state);
            if was_active && (now_gone || now_changed) {
                attempts_here += 1;
            }
            if now_gone
                && let Some(aged) = draft_state.get(key)
                && !aged
            {
                out.drafts.reconciled_within_window += 1;
            }
        }
        if matches!(event, TraceEvent::HolderAvailable { .. }) {
            out.reconciliation_attempts_by_event
                .insert(idx, attempts_here);
        }

        // ── Soundness bookkeeping (input-based; see module docs). ──
        let dispute = foreign_paths
            .iter()
            .any(|p| dirty_paths.get(p).copied().unwrap_or(0) > 0);
        let corruption = !foreign_paths.is_empty() && !new_findings.is_empty();
        if dispute || corruption {
            for (i, span) in spans.iter().enumerate() {
                if span.contains(at) {
                    span_unsound[i] = true;
                }
            }
            prefix_sound = false;
        }

        // Checker bytes on the entirely-sound workspace prefix (Law 3E at
        // every observation point).
        if prefix_sound && unavailable.is_empty() {
            out.sound_checker_output_bytes +=
                cur.findings.iter().map(|l| l.len() as u64).sum::<u64>();
        }

        // ── D11.4 failure attribution. ──
        let failed = !unexpected_items.is_empty() || !new_findings.is_empty() || undeclared_overlay;
        if failed {
            out.failed_ops += per_event_ops[idx].saturating_sub(excluded_here);
        }
        out.excluded_ops += excluded_here;

        // ── Session attribution. ──
        for (i, span) in spans.iter().enumerate() {
            if !span.contains(at) {
                continue;
            }
            if failed {
                span_intervention[i] = true;
            }
            span_diags[i] += new_findings.len() as u64;
            span_unexpected_items[i] += unexpected_items.len() as u64;
            for (id, cause) in &new_items {
                incidents
                    .entry((i, (*cause).clone()))
                    .or_default()
                    .insert((*id).clone());
            }
            for line in &new_findings {
                incidents
                    .entry((i, format!("diag:{line}")))
                    .or_default()
                    .insert((*line).clone());
            }
        }

        prev = cur;
        prev_at = at;
    }

    // ── Fold session-level outcomes. ──
    for i in 0..spans.len() {
        if !span_intervention[i] {
            out.intervention_free_sessions += 1;
        }
        if !span_unsound[i] {
            out.sound_session_diagnostics += span_diags[i];
            out.unexpected_items_in_sound_sessions += span_unexpected_items[i];
        }
    }
    out.max_visible_incidents_per_root_cause = incidents
        .values()
        .map(|ids| ids.len() as u64)
        .max()
        .unwrap_or(0);

    Ok(out)
}

/// The corpus label a scorecard records (`Scorecard.corpus`): the corpus
/// dir's last component, prefixed with `heldout/` for locked versions.
fn corpus_label(corpus_dir: &Utf8Path) -> String {
    let name = corpus_dir.file_name().unwrap_or("corpus").to_owned();
    match corpus_dir.parent().and_then(Utf8Path::file_name) {
        Some("heldout") => format!("heldout/{name}"),
        _ => name,
    }
}

/// Recursively collect `*.trace.ndjson` under a corpus dir, sorted.
fn corpus_traces(corpus_dir: &Utf8Path) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let mut found = Vec::new();
    let mut stack = vec![corpus_dir.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                stack.push(path);
            } else if path.as_str().ends_with(".trace.ndjson")
                || path.as_str().ends_with(".trace.ndjson.zst")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Read one corpus trace file, transparently decompressing `.zst` (AM-11.6:
/// large imported histories are stored zstd-compressed in-tree — the
/// MANIFEST freezes the COMPRESSED bytes; this is the only decode site).
///
/// # Errors
/// IO/decompression failures, or non-UTF-8 decompressed content.
pub fn read_trace_file(path: &Utf8Path) -> anyhow::Result<String> {
    if path.extension() == Some("zst") {
        let file = std::fs::File::open(path)?;
        let bytes = zstd::decode_all(std::io::BufReader::new(file))?;
        Ok(String::from_utf8(bytes)?)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}

/// Algorithm C entry: replay every trace in `corpus_dir` whose header
/// declares `profile`, aggregate per D11.4, and emit the nine frozen
/// scorecard fields plus the side transient-draft report.
///
/// # Errors
/// IO/parse/replay failures, or a corpus containing NO trace for the
/// requested profile (an empty measurement is meaningless and fails loudly).
pub fn run(
    corpus_dir: &Utf8Path,
    profile: &str,
) -> anyhow::Result<(Scorecard, TransientDraftReport)> {
    let mut totals = TraceReplay::default();
    let mut drafts = TransientDraftReport::default();
    let mut max_incidents = 0u64;
    let mut matched = 0u64;

    for path in corpus_traces(corpus_dir)? {
        let ndjson = read_trace_file(&path)?;
        let trace =
            Trace::parse(&ndjson).map_err(|e| anyhow::anyhow!("{path}: corpus defect: {e}"))?;
        let TraceEvent::TraceHeader {
            profile: trace_profile,
            ..
        } = &trace.header
        else {
            anyhow::bail!("{path}: no header");
        };
        if trace_profile != profile {
            continue;
        }
        matched += 1;
        let replay =
            replay_trace(&trace).map_err(|e| anyhow::anyhow!("{path}: replay failed: {e}"))?;
        totals.ops += replay.ops;
        totals.excluded_ops += replay.excluded_ops;
        totals.failed_ops += replay.failed_ops;
        totals.sessions += replay.sessions;
        totals.intervention_free_sessions += replay.intervention_free_sessions;
        totals.sound_session_diagnostics += replay.sound_session_diagnostics;
        totals.unexpected_items_in_sound_sessions += replay.unexpected_items_in_sound_sessions;
        totals.sound_checker_output_bytes += replay.sound_checker_output_bytes;
        max_incidents = max_incidents.max(replay.max_visible_incidents_per_root_cause);
        drafts.declared += replay.drafts.declared;
        drafts.reconciled_within_window += replay.drafts.reconciled_within_window;
        drafts.aged_into_debt += replay.drafts.aged_into_debt;
    }
    anyhow::ensure!(
        matched > 0,
        "{corpus_dir}: no {profile} traces — an empty measurement is meaningless"
    );

    let denom = totals.ops - totals.excluded_ops;
    let auto = denom - totals.failed_ops.min(denom);
    #[allow(clippy::cast_precision_loss)] // toy-scale counts
    let auto_resolution_rate = if denom == 0 {
        1.0
    } else {
        auto as f64 / denom as f64
    };
    #[allow(clippy::cast_precision_loss)]
    let intervention_free_session_rate = if totals.sessions == 0 {
        1.0
    } else {
        totals.intervention_free_sessions as f64 / totals.sessions as f64
    };

    Ok((
        Scorecard {
            profile: profile.to_owned(),
            corpus: corpus_label(corpus_dir),
            auto_resolution_rate,
            intervention_free_session_rate,
            sound_session_diagnostics: totals.sound_session_diagnostics,
            sound_checker_output_bytes: totals.sound_checker_output_bytes,
            unexpected_reconciliation_items: totals.unexpected_items_in_sound_sessions,
            max_visible_incidents_per_root_cause: max_incidents,
            manual_contract_authoring: 0, // Phase -1 surfaces no Contract authoring; collected as a constant
        },
        drafts,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labeled(name: &str) -> Trace {
        let path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("fixtures/traces/labeled/{name}.trace.ndjson"));
        let ndjson = std::fs::read_to_string(&path).expect("labeled fixture readable");
        Trace::parse(&ndjson).expect("labeled fixture parses")
    }

    /// The DG-11.1 cross-check the frozen golden's T1 ruling mandated: the
    /// denominator's trace-level `holder_available` reading (one op per
    /// distinct pending-draft path, per D05.1/D05.3) must equal the number
    /// of Overlay reconciliations the REAL replay attempts on Holder
    /// return. A mismatch here is a T1 escalation, never a silent fix.
    #[test]
    fn dg11_1_replay_reconciliation_attempts_match_frozen_denominator() {
        let trace = labeled("offline-transient");
        let per_event = denominator::per_event_ops(&trace.events);
        let replay = replay_trace(&trace).expect("offline-transient replays");

        let holder_available_indices: Vec<usize> = trace
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, TraceEvent::HolderAvailable { .. }))
            .map(|(i, _)| i)
            .collect();
        assert!(!holder_available_indices.is_empty());
        for idx in holder_available_indices {
            let counted = per_event[idx];
            let attempted = replay
                .reconciliation_attempts_by_event
                .get(&idx)
                .copied()
                .unwrap_or(0);
            assert_eq!(
                counted, attempted,
                "DG-11.1: denominator counted {counted} reconciliation op(s) at event \
                 {idx} but replay attempted {attempted} — the frozen reading and the \
                 real runner disagree (T1 escalation)"
            );
        }
    }

    /// The offline labeled fixture drives the declared-transient path: one
    /// declared draft, reconciled WITHIN its window (the fixture's
    /// clock_advance is ~16.6 min — inside the 24 h escalate window, so no
    /// aging); the save op is excluded from the D11.4 numerator/denominator.
    /// The aged-into-debt branch is exercised by `tracegen offline`'s 25 h
    /// corpus traces (third slo test, M11.8).
    #[test]
    fn offline_transient_draft_accounting() {
        let replay = replay_trace(&labeled("offline-transient")).expect("replays");
        assert_eq!(replay.drafts.declared, 1);
        assert_eq!(replay.drafts.aged_into_debt, 0);
        assert_eq!(replay.drafts.reconciled_within_window, 1);
        assert_eq!(replay.excluded_ops, 1, "the offline save is excluded");
    }
}
