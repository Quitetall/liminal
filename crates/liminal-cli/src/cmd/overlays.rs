//! `lim overlays` (v4 §7.10.8; R4 §9): reconciliation debt — count, age,
//! affected domains, repair blockers. Uses everyday vocabulary ("draft",
//! "pending sync", "review needed"), never internal terms (R4 §3).
//!
//! Algorithm C (M05.md): zero active overlays → zero bytes, exit 0. One line
//! per overlay, sorted by `(created_at, id)`, two-space columns:
//! `<status>  <path>  <age>`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

use camino::Utf8Path;
use liminal_daemon::ToyWorkspace;
use liminal_daemon::runner::{now_with_offset, overlay_operation_path};
use liminal_id::Timestamp;
use liminal_jurisdiction::{Overlay, OverlayState};

/// List reconciliation debt. Zero debt → zero output (Law 3E). Exit 0
/// normally, 2 on error.
pub(crate) fn run(workspace: &Utf8Path, all: bool) -> ExitCode {
    match run_inner(workspace, all) {
        Ok(code) => code,
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{e}");
            ExitCode::from(2)
        }
    }
}

fn run_inner(workspace: &Utf8Path, all: bool) -> anyhow::Result<ExitCode> {
    // `ToyWorkspace::open` runs Algorithm B's sweep before returning, so this
    // renders post-aging/holder-return state (M05.md: "before `lim overlays`
    // / `lim check` output").
    let ws = ToyWorkspace::open(workspace)?;
    let store = ws.store();
    let now = now_with_offset(store)?;

    let mut overlays: Vec<Overlay> = store
        .scan_aux(liminal_graph::ns::JUR_OVERLAY)?
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_value::<Overlay>(v).ok())
        .filter(|o| {
            matches!(
                o.state,
                OverlayState::Active | OverlayState::RepairProposed(_) | OverlayState::Contested
            ) || (all && matches!(o.state, OverlayState::Archived | OverlayState::Discarded))
        })
        .collect();

    if overlays.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    overlays.sort_by_key(|o| (o.created_at, o.id));

    let mut out = String::new();
    for overlay in &overlays {
        let status = status_label(overlay, now);
        let path = overlay_operation_path(&overlay.operation)
            .map(|p| p.0.to_string())
            .or_else(|| ws.alias_for(overlay.subject))
            .unwrap_or_else(|| overlay.subject.to_string());
        let age = format_age(now.0 - overlay.created_at.0);
        let _ = writeln!(out, "{status}  {path}  {age}");
    }
    print!("{out}");
    Ok(ExitCode::SUCCESS)
}

/// Everyday status label (Algorithm C; R4 §3 — never "Overlay"/"RepairProposed").
fn status_label(overlay: &Overlay, now: Timestamp) -> &'static str {
    match overlay.state {
        OverlayState::Active => {
            let age_ms = now.0 - overlay.created_at.0;
            let escalate_ms =
                i64::try_from(overlay.lifecycle.escalate_after.as_millis()).unwrap_or(i64::MAX);
            if overlay.lifecycle.declared_transient {
                if age_ms > escalate_ms {
                    "pending sync (overdue)"
                } else {
                    "pending sync"
                }
            } else {
                "draft"
            }
        }
        OverlayState::RepairProposed(_) => "review needed",
        OverlayState::Contested => "review needed (conflict)",
        OverlayState::Archived => "archived",
        OverlayState::Discarded => "discarded",
    }
}

/// Age rendering (Algorithm C): `<n>s` <60s, `<n>m` <60m, `<n>h` <24h, else `<n>d`.
fn format_age(age_ms: i64) -> String {
    let secs = (age_ms / 1000).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}
