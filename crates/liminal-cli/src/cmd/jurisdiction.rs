//! `lim jurisdiction explain <subject>` (v4 §7.9; R4 §3): the ONLY surface
//! where the internal vocabulary (Jurisdiction, Holder, Overlay, Promotion)
//! appears.

use std::io::Write as _;
use std::process::ExitCode;

use camino::Utf8Path;
use liminal_daemon::ToyWorkspace;
use liminal_jurisdiction::{ProfileSet, render_holder};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

/// Explain one subject's governance in full. Exit 0 on success, 2 on error.
pub(crate) fn explain(workspace: &Utf8Path, subject: &str) -> ExitCode {
    match explain_inner(workspace, subject) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{e}");
            ExitCode::from(2)
        }
    }
}

fn explain_inner(workspace: &Utf8Path, subject_str: &str) -> anyhow::Result<()> {
    let subject: liminal_id::JurisdictionSubject =
        subject_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?;

    let ws = ToyWorkspace::open(workspace)?;
    let checker = ws.checker();

    let basis = WorkspaceBasis {
        transaction: liminal_id::TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: std::collections::BTreeMap::new(),
    };

    let explanation = liminal_jurisdiction::explain(&checker, subject, &basis)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Profile id for the subject.
    let profiles = ProfileSet::phase_minus_1();
    let profile_id = profiles
        .for_subject(subject, ws.store())
        .map_or_else(|| "unknown".to_owned(), |p| p.id().0.into_owned());

    let alias = ws.alias_for(subject);
    let alias_suffix = alias
        .as_deref()
        .map_or_else(String::new, |a| format!(" (alias: {a})"));

    let grade = liminal_jurisdiction::grade_of(subject, ws.store());
    let required = explanation.contract.continuity.required_identity;
    let merge = explanation
        .contract
        .resolution
        .merge_runtime
        .as_ref()
        .map_or_else(|| "none".to_owned(), |m| m.0.clone());
    let overlays = if explanation.overlays.is_empty() {
        "none".to_owned()
    } else {
        explanation
            .overlays
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };

    let contract_toml =
        toml::to_string_pretty(&explanation.contract).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut contract_indented = String::new();
    for line in contract_toml.lines() {
        contract_indented.push_str("  ");
        contract_indented.push_str(line);
        contract_indented.push('\n');
    }

    let mut stdout = std::io::stdout().lock();
    write!(
        stdout,
        "subject: {subject}{alias_suffix}\n\
         profile: {profile_id}\n\
         holder: {}\n\
         write route: {}\n\
         perspective: durable-only\n\
         identity: {} (required floor: {})\n\
         merge runtime: {merge}\n\
         overlays: {overlays}\n\
         contract:\n{contract_indented}\
         narrative:\n  {}\n",
        render_holder(&explanation.holder),
        render_holder(&explanation.write_route),
        format!("{grade:?}").to_lowercase(),
        format!("{required:?}").to_lowercase(),
        explanation.narrative,
    )?;
    Ok(())
}
