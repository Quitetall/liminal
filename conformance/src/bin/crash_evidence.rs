//! `crash-evidence` — the HAQP-1 §5 fault-lane evidence recorder.
//!
//! ADR-0020 §5 requires that "every registered durable-transition crash
//! boundary is injected immediately before and after the transition. Recovery
//! runs twice and must reach the same terminal state without hidden staged
//! data, duplicate effects, or mixed Basis," and that the fault matrix be
//! "exhaustive over registered boundaries. Runtime discovery and declared
//! inventory must match exactly; either-side drift fails."
//!
//! The checks themselves already live in `ToyRun::crash_matrix` (double
//! recovery by world digest and terminal set, staged-file emptiness,
//! file-world convergence against baseline). This binary RUNS them across
//! every runnable scenario, reconciles the boundaries actually exercised
//! against the runtime registry, and records the result as evidence.
//!
//! Any boundary that is registered but never exercised is a FAILURE, not a
//! gap to note: an unexercised boundary is indistinguishable from a dead one.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use liminal_conformance::harness::{ToyRun, runnable_crash_scenarios};

fn main() -> anyhow::Result<()> {
    let source_commit = git_rev_parse("HEAD")?;
    let source_tree = git_rev_parse("HEAD^{tree}")?;
    let lockfile_blake3 = blake3::hash(&std::fs::read("Cargo.lock")?)
        .to_hex()
        .to_string();
    let registered: BTreeSet<String> = liminal_jurisdiction::CrashPoint::all()
        .iter()
        .map(|point| point.name().to_owned())
        .collect();

    let scenarios = runnable_crash_scenarios()?;
    anyhow::ensure!(
        !scenarios.is_empty(),
        "no runnable crash scenarios — an empty fault matrix proves nothing"
    );

    // Which boundaries does each scenario's baseline actually reach, and at
    // how many occurrences? This is the RUNTIME discovery side of §5.
    let mut exercised: BTreeMap<String, u64> = BTreeMap::new();
    let mut per_scenario = Vec::new();
    let mut recovery_by_boundary: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for scenario in &scenarios {
        let id = scenario.scenario.id.clone();
        let run = ToyRun::new(&format!("evidence-{id}"))?;
        let trace = run.baseline(scenario)?;
        let faults = trace.enumerate_faults();
        anyhow::ensure!(
            !faults.is_empty(),
            "{id}: baseline reached no durable boundary"
        );
        for (point, _occurrence) in &faults {
            *exercised.entry(point.clone()).or_default() += 1;
        }

        // Run the full matrix: crash at every (point, occurrence), recover
        // TWICE, compare terminal state and world digest, require no staged
        // residue, and converge the file world against baseline.
        let matrix = ToyRun::crash_matrix(scenario)?;
        for case in &matrix.cases {
            recovery_by_boundary
                .entry(case.boundary.clone())
                .or_default()
                .push(serde_json::json!({
                    "scenario": id.clone(),
                    "occurrence": case.occurrence,
                    "first_recovery_digest": case.first_recovery_digest.clone(),
                    "second_recovery_digest": case.second_recovery_digest.clone(),
                    "first_terminal_digest": case.first_terminal_digest.clone(),
                    "second_terminal_digest": case.second_terminal_digest.clone(),
                    "first_basis_digest": case.first_basis_digest.clone(),
                    "second_basis_digest": case.second_basis_digest.clone(),
                    "first_effect_digest": case.first_effect_digest.clone(),
                    "second_effect_digest": case.second_effect_digest.clone(),
                }));
        }
        per_scenario.push(serde_json::json!({
            "scenario": id,
            "faults_injected": faults.len(),
            "boundaries": faults.iter().map(|(p, _)| p.clone()).collect::<BTreeSet<_>>(),
            "result": "pass",
        }));
        println!("{id}: {} faults injected, matrix pass", faults.len());
    }

    // §5 exhaustiveness: declared registry vs what actually fired. Drift on
    // EITHER side fails.
    let exercised_names: BTreeSet<String> = exercised.keys().cloned().collect();
    let never_fired: Vec<&String> = registered.difference(&exercised_names).collect();
    let unregistered: Vec<&String> = exercised_names.difference(&registered).collect();
    anyhow::ensure!(
        never_fired.is_empty(),
        "registered boundaries never exercised (dead spec): {never_fired:?}"
    );
    anyhow::ensure!(
        unregistered.is_empty(),
        "boundaries fired that are not registered: {unregistered:?}"
    );

    let boundaries: Vec<serde_json::Value> = registered
        .iter()
        .map(|name| {
            // `before_*` / `after_*` pairs are how the registry expresses
            // "injected immediately before and after the transition"; both
            // sides of every pair are exercised or the check above failed.
            serde_json::json!({
                "boundary": name,
                "injected": exercised.contains_key(name),
                "occurrences_exercised": exercised.get(name).copied().unwrap_or(0),
                "recovery_pairs": recovery_by_boundary.get(name).cloned().unwrap_or_default(),
                "staged_residue": "none",
                "result": "pass",
            })
        })
        .collect();

    let evidence = serde_json::json!({
        "source_commit": source_commit,
        "source_tree": source_tree,
        "lockfile_blake3": lockfile_blake3,
        "registered": registered.len(),
        "exercised": exercised_names.len(),
        "scenarios": per_scenario,
        "boundaries": boundaries,
    });
    // COMMITTED evidence, not `target/` (M17.5 pass-2 #19). Written under
    // `target/` this artifact was gitignored, so `haq verify` could never read
    // it: the packet asserted a crash result that nothing on disk corroborated,
    // and the inventory exited 0 with the artifact saying `pass` and the packet
    // saying `registered`. `fuzz.json` already lives here for the same reason.
    let path = std::env::var("HAQP_CRASH_EVIDENCE_OUT").map_or_else(
        |_| camino::Utf8PathBuf::from("conformance/haqp/evidence/crash.json"),
        camino::Utf8PathBuf::from,
    );
    std::fs::create_dir_all(path.parent().expect("evidence parent"))?;
    std::fs::write(&path, serde_json::to_vec_pretty(&evidence)?)?;
    println!(
        "crash evidence complete: {}/{} registered boundaries exercised; {path}",
        exercised_names.len(),
        registered.len()
    );
    Ok(())
}

fn git_rev_parse(spec: &str) -> anyhow::Result<String> {
    let output = Command::new("git").args(["rev-parse", spec]).output()?;
    anyhow::ensure!(output.status.success(), "git rev-parse {spec:?} failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
