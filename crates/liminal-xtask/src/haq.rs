//! HAQP-1 packet verification.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use liminal_format::Formatter;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Verify committed HAQP inventories, packet status, and crash-boundary registry.
pub fn verify_inventory_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_requirement_sources(root, &packet)?;
    verify_packet_statuses(&packet, inventory_statuses())?;
    verify_markdown_surface(root, &packet)?;
    Ok(())
}

/// Verify completed HAQP qualification evidence. Planned or NOT_RUN packet rows
/// fail here; this is the command used by the M17 packet gate.
pub fn verify_qualified_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_requirement_sources(root, &packet)?;
    verify_markdown_surface(root, &packet)?;
    require_eq(
        "qualification_state",
        &packet.qualification_state,
        "complete",
    )?;
    verify_packet_statuses(
        &packet,
        PacketStatusExpectations {
            // ADR-0021 stages the mutation requirement, so the expected mutant
            // disposition is stage-dependent. Hardcoding `killed` here made
            // HAQP-1a UNREACHABLE: leave mutants `predeclared` and this gate
            // failed; mark one `killed` and verify_qualification_stage failed.
            // The first version of ADR-0021 shipped with exactly that
            // contradiction — the deadlock it existed to break, reintroduced
            // one layer down.
            mutant: if packet.qualification_stage == "1b" {
                "resolved"
            } else {
                "predeclared"
            },
            canary: "caught",
            generated: "pass",
            crash: "pass",
            review: "pass",
        },
    )?;
    // M17.5 A6 / pass-2 #14,#15,#17,#19,#21: statuses are CLAIMS. Bind them to
    // artifacts a run actually produced, or the whole gate is satisfiable by
    // editing two files.
    verify_provenance(root, &packet)?;
    verify_fuzz_evidence(root, &packet)?;
    verify_corpus_access_audit(root, &packet)?;
    verify_crash_evidence(root, &packet)?;
    verify_campaign_clock(root)?;
    verify_residual_risks(&packet)?;
    verify_qualification_stage(&packet)?;
    if packet.qualification_stage == "1b" {
        verify_mutant_killing_tests(root, &packet)?;
        verify_mutant_evidence(root, &packet)?;
    }
    verify_canary_evidence(root, &packet)?;
    verify_generated_evidence(root, &packet)?;
    verify_review_evidence(root, &packet)?;
    // HAQP-1a qualifies packet machinery before Phase 1 authorization; its
    // inventory may still name future M18-M24 tests. HAQP-1b runs at M24 and
    // must bind every declared test to a real, runnable function.
    if packet.qualification_stage == "1b" {
        verify_test_names_exist(root, &packet)?;
    }
    Ok(())
}

/// Bind the packet's crash-boundary claims to the fault lane's committed
/// artifact (M17.5 pass-2 #19).
///
/// The lane wrote `target/haqp/crash.json`, which is gitignored, so nothing
/// could ever read it back: the artifact said `pass`, the packet said
/// `registered`, and `haq verify-inventory` exited 0 with no one comparing the
/// two. The packet's crash rows were pure assertion.
fn verify_crash_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/crash.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed crash evidence is required; run `just haq-crash`")
    })?;
    let recorded: CrashEvidence =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;

    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    verify_crash_rows(&recorded, &declared)
}

/// Bind the packet's canary claims to the run that produced them
/// (M17.5 F-27 / P1-A03).
///
/// The qualified path accepted `result: caught` on every canary row while never
/// invoking the runner and never reading its output. `run_canaries_repo` wrote
/// `target/haqp/canaries.json`, which was gitignored — the same shape as the
/// crash-evidence defect fixed at pass-2 #19, left in place for canaries.
///
/// Canaries are the mechanism the whole gate rests on: each one proves a
/// deliberate violation makes the expected gate fail. A canary table nobody
/// executed proves nothing at all.
fn verify_canary_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/canaries.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed canary evidence is required; run `just haq-canaries`")
    })?;
    let recorded: Vec<CanaryEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    verify_canary_rows(&packet.canaries, &recorded)
}

fn verify_canary_rows(declared: &[Canary], recorded: &[CanaryEvidence]) -> Result<()> {
    let by_id: BTreeMap<&str, &CanaryEvidence> =
        recorded.iter().map(|row| (row.id.as_str(), row)).collect();
    for canary in declared {
        if canary.result != "caught" {
            continue;
        }
        let evidence = by_id.get(canary.id.as_str()).with_context(|| {
            format!(
                "{} claims caught but the committed canary run has no such canary",
                canary.id
            )
        })?;
        if !evidence.caught {
            anyhow::bail!(
                "{} claims caught but the committed run recorded it as NOT caught",
                canary.id
            );
        }
        // The prose must describe the mutation that actually ran, or the table
        // a reader trusts describes a different experiment from the one
        // performed.
        if evidence.violation != canary.violation {
            anyhow::bail!(
                "{}: packet says the violation is {:?} but the run performed {:?}",
                canary.id,
                canary.violation,
                evidence.violation
            );
        }
        if evidence.expected_failure != canary.expected_failure {
            anyhow::bail!(
                "{}: packet expects failure {:?} but the run expected {:?}",
                canary.id,
                canary.expected_failure,
                evidence.expected_failure
            );
        }
        anyhow::ensure!(
            canary_failure_matches(&evidence.expected_failure, &evidence.observed_failure),
            "{}: observed failure {:?} does not match expected failure {:?}",
            canary.id,
            evidence.observed_failure,
            evidence.expected_failure
        );
        require_eq("canary evidence gate", &evidence.gate, &canary.gate)?;
        require_eq(
            "canary evidence mutation semantics",
            &evidence.mutation_semantics,
            canary_mutation_semantics(&canary.id)?,
        )?;
    }
    // Every canary the run exercised must be declared, or the packet is a
    // subset of the experiment and a reader cannot tell which rows are missing.
    let declared_ids: BTreeSet<&str> = declared.iter().map(|row| row.id.as_str()).collect();
    for row in recorded {
        if !declared_ids.contains(row.id.as_str()) {
            anyhow::bail!(
                "{} was exercised by the canary run but the packet does not declare it",
                row.id
            );
        }
    }
    Ok(())
}

/// Bind each generated family's claimed hash to the committed artifact
/// (M17.5 F-27 / P1-A04, and P2-F03 — both blind passes found it).
///
/// `evidence_hash` was checked for length and nothing else: never recomputed,
/// never compared against a run, and `target/haqp/generated.json` — the file the
/// generator actually writes — was gitignored and therefore unreadable.
fn verify_generated_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/generated.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed generated evidence is required; run `just haq-generated`")
    })?;
    let recorded: GeneratedEvidenceArtifact =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "generated evidence schema_version",
        &recorded.schema_version,
        "haqp-generated-v1",
    )?;
    require_eq(
        "generated evidence artifact_blake3",
        &recorded.artifact_blake3,
        &generated_artifact_digest(&recorded.rows)?,
    )?;
    require_exact_ids(
        recorded.rows.iter().map(|row| row.family.as_str()),
        GENERATED_FAMILIES.iter().map(|family| (*family).to_owned()),
        "generated evidence family",
    )?;
    verify_generated_rows(&packet.generated, &recorded.rows)
}

fn verify_generated_rows(declared: &[Generated], recorded: &[GeneratedEvidence]) -> Result<()> {
    let mut by_family = BTreeMap::new();
    for row in recorded {
        if by_family.insert(row.family.as_str(), row).is_some() {
            anyhow::bail!("generated evidence repeats family {:?}", row.family);
        }
    }
    for family in declared {
        if family.result != "pass" {
            continue;
        }
        let evidence = by_family.get(family.family.as_str()).with_context(|| {
            format!(
                "{} claims pass but the committed generated evidence has no such family",
                family.family
            )
        })?;
        // The hash covers the family's whole accept/discard stream and cannot
        // be recomputed here without rerunning 100,000 cases. It is bound
        // instead to the digest the RUN recorded, which is the strongest check
        // available at verification time: the packet may no longer cite a
        // number no run produced. Determinism of that digest is pinned
        // separately by `generated_evidence_is_byte_identical_across_runs`, so
        // editing the committed artifact to match a doctored packet is
        // detectable by rerunning the generator.
        let claimed = family
            .evidence_hash
            .as_deref()
            .context("inventory layer requires a hash before `pass`")?;
        if !claimed.eq_ignore_ascii_case(&evidence.evidence_hash) {
            anyhow::bail!(
                "{}: packet claims evidence hash {claimed} but the committed run recorded {}",
                family.family,
                evidence.evidence_hash
            );
        }
        for (label, packet_value, evidence_value) in [
            ("accepted", family.accepted, evidence.accepted),
            ("attempts", family.attempts, evidence.attempts),
            ("discards", family.discards, evidence.discards),
        ] {
            if packet_value != evidence_value {
                anyhow::bail!(
                    "{}: packet declares {packet_value} {label} but the committed run recorded {evidence_value}",
                    family.family
                );
            }
        }
        if family.seed != Some(evidence.seed) {
            anyhow::bail!(
                "{}: packet declares seed {:?} but the committed run used {}",
                family.family,
                family.seed,
                evidence.seed
            );
        }
    }
    Ok(())
}

fn verify_crash_rows(recorded: &CrashEvidence, declared: &BTreeSet<String>) -> Result<()> {
    let evidenced = recorded
        .boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    if declared != &evidenced {
        anyhow::bail!(
            "crash evidence covers different boundaries than the packet declares: \
             packet={declared:?}, evidence={evidenced:?}"
        );
    }
    let before = evidenced
        .iter()
        .filter(|boundary| boundary.contains("/before_"))
        .count();
    let after = evidenced
        .iter()
        .filter(|boundary| boundary.contains("/after_"))
        .count();
    anyhow::ensure!(
        before > 0 && after > 0,
        "crash evidence must identify both before-transition and after-transition boundaries"
    );
    for boundary in &evidenced {
        anyhow::ensure!(
            boundary.contains("/before_") || boundary.contains("/after_"),
            "crash boundary {boundary:?} lacks an injection-side coordinate"
        );
    }
    anyhow::ensure!(
        recorded.registered == evidenced.len() && recorded.exercised == evidenced.len(),
        "crash evidence registered/exercised counts do not match evidenced boundary count"
    );
    // ADR-0020 §5: runtime discovery and declared inventory must match exactly,
    // and drift on EITHER side fails.
    if recorded.registered != recorded.exercised {
        anyhow::bail!(
            "crash evidence records {} registered boundaries but {} exercised — an \
             unexercised boundary is indistinguishable from a dead one",
            recorded.registered,
            recorded.exercised
        );
    }
    verify_crash_scenario_coverage(recorded, declared)?;
    for row in &recorded.boundaries {
        if row.occurrences_exercised == 0 {
            anyhow::bail!("crash boundary {} was never exercised", row.boundary);
        }
        if row.result != "pass" {
            anyhow::bail!(
                "crash boundary {} reports result {:?}",
                row.boundary,
                row.result
            );
        }
        verify_recovery_pairs(row)?;
        if row.staged_residue != "none" {
            anyhow::bail!(
                "crash boundary {} left staged residue {:?}",
                row.boundary,
                row.staged_residue
            );
        }
    }
    Ok(())
}

fn verify_crash_scenario_coverage(
    recorded: &CrashEvidence,
    declared: &BTreeSet<String>,
) -> Result<()> {
    anyhow::ensure!(
        !recorded.scenarios.is_empty(),
        "crash evidence records no scenarios; an empty fault matrix proves nothing"
    );
    for scenario in &recorded.scenarios {
        require_eq("crash scenario result", &scenario.result, "pass")?;
        anyhow::ensure!(
            scenario.faults_injected > 0,
            "crash scenario {} injected no faults, so it exercised nothing",
            scenario.scenario
        );
        anyhow::ensure!(
            !scenario.boundaries.is_empty(),
            "crash scenario {} injected faults but names no boundaries",
            scenario.scenario
        );
        for boundary in &scenario.boundaries {
            anyhow::ensure!(
                declared.contains(boundary),
                "crash scenario {} exercised unregistered boundary {boundary}",
                scenario.scenario
            );
        }
        anyhow::ensure!(
            scenario.faults_injected >= scenario.boundaries.len() as u64,
            "crash scenario {} reports {} faults across {} distinct boundaries",
            scenario.scenario,
            scenario.faults_injected,
            scenario.boundaries.len()
        );
    }
    let scenario_union = recorded
        .scenarios
        .iter()
        .flat_map(|scenario| scenario.boundaries.iter().cloned())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        scenario_union == *declared,
        "crash scenario matrix does not cover every declared boundary"
    );
    for row in &recorded.boundaries {
        for pair in &row.recovery_pairs {
            anyhow::ensure!(
                recorded.scenarios.iter().any(|scenario| {
                    scenario.scenario == pair.scenario
                        && scenario.boundaries.contains(&row.boundary)
                }),
                "crash recovery pair {}:{} is not covered by its scenario boundary matrix",
                pair.scenario,
                pair.occurrence
            );
        }
    }
    Ok(())
}

fn verify_recovery_pairs(row: &CrashEvidenceBoundary) -> Result<()> {
    anyhow::ensure!(
        row.recovery_pairs.len() as u64 == row.occurrences_exercised,
        "crash boundary {} records {} recovery pairs for {} exercised occurrences",
        row.boundary,
        row.recovery_pairs.len(),
        row.occurrences_exercised
    );
    let mut pair_ids = BTreeSet::new();
    for pair in &row.recovery_pairs {
        anyhow::ensure!(
            !pair.scenario.trim().is_empty(),
            "crash boundary {} has empty recovery scenario",
            row.boundary
        );
        anyhow::ensure!(
            pair_ids.insert((pair.scenario.as_str(), pair.occurrence)),
            "crash boundary {} repeats recovery case {}:{}",
            row.boundary,
            pair.scenario,
            pair.occurrence
        );
        for (label, digest) in [
            ("first_recovery_digest", &pair.first_recovery_digest),
            ("second_recovery_digest", &pair.second_recovery_digest),
        ] {
            anyhow::ensure!(
                digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
                "crash boundary {} has invalid {label}",
                row.boundary
            );
        }
        anyhow::ensure!(
            pair.first_recovery_digest == pair.second_recovery_digest,
            "crash boundary {} recovery case {}:{} changed world digest on second recovery",
            row.boundary,
            pair.scenario,
            pair.occurrence
        );
    }
    Ok(())
}

/// Bind each review row to a committed record (M17.5 pass-2 #20).
///
/// A review row was four self-asserted numbers with nothing behind them. The
/// internal-consistency rules hold at every layer; the committed record is
/// demanded only once a review claims `pass`, matching how
/// `verify_generated_inventory` treats seeds and evidence hashes.
fn verify_review_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut records: Vec<(String, ReviewRecord)> = Vec::new();
    for review in &packet.reviews {
        let raised = review.findings.iter().collect::<BTreeSet<_>>();
        let unverified = review
            .independently_reproduced
            .iter()
            .filter(|id| !raised.contains(id))
            .collect::<Vec<_>>();
        if !unverified.is_empty() {
            anyhow::bail!(
                "{} claims to have independently reproduced {unverified:?}, which it \
                 never raised as findings",
                review.reviewer
            );
        }
        // A finding counted as VERIFIED and unresolved must be one that survived
        // independent reproduction; otherwise "verified" means one model said so.
        if review.unresolved_verified_findings > review.independently_reproduced.len() as u64 {
            anyhow::bail!(
                "{} counts {} unresolved VERIFIED findings but independently reproduced \
                 only {} — a finding one reviewer asserted is not a verified finding \
                 (ADR-0020 §6)",
                review.reviewer,
                review.unresolved_verified_findings,
                review.independently_reproduced.len()
            );
        }
        if review.result != "pass" {
            continue;
        }
        let Some(evidence) = &review.evidence else {
            anyhow::bail!(
                "{} claims pass with no committed record; the lane's own output lives \
                 under target/ and cannot be read back",
                review.reviewer
            );
        };
        // The packet is the very thing this verifier exists to distrust, so its
        // paths are not taken on faith. `join` DISCARDS the base when handed an
        // absolute path, so `evidence: "/etc/passwd"` would read straight out of
        // the repository; `..` walks out just as effectively.
        let candidate = Utf8Path::new(evidence);
        if candidate.is_absolute()
            || candidate
                .components()
                .any(|part| part == camino::Utf8Component::ParentDir)
        {
            anyhow::bail!(
                "{} points its record at {evidence:?}, which escapes the repository; \
                 evidence paths must be relative and contain no `..`",
                review.reviewer
            );
        }
        let path = root.join(candidate);
        let canonical_root = fs::canonicalize(root.as_std_path())
            .context("canonicalize repository root for review evidence")?;
        let canonical_path = fs::canonicalize(&path).with_context(|| {
            format!(
                "{path}: {} claims pass but its record is missing",
                review.reviewer
            )
        })?;
        anyhow::ensure!(
            canonical_path.starts_with(&canonical_root),
            "{} review record resolves outside repository via symlink",
            review.reviewer
        );
        let bytes = fs::read(&path).with_context(|| {
            format!(
                "{path}: {} claims pass but its record is missing",
                review.reviewer
            )
        })?;
        let record: ReviewRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
        verify_review_record(root, review, &record)?;
        let provenance = packet
            .provenance
            .as_ref()
            .context("qualified review evidence requires packet provenance")?;
        require_eq(
            "review record fixed_base.commit",
            &record.fixed_base.commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "review record fixed_base.tree",
            &record.fixed_base.tree,
            &provenance.fixed_tree,
        )?;
        anyhow::ensure!(
            record.fixed_base.clean,
            "{} review record was produced from a dirty fixed base",
            review.reviewer
        );
        records.push((review.reviewer.clone(), record));
    }
    verify_reviewer_independence(&records)
}

/// One review row against its own committed record (M17.5 F-27 / P1-A05, A06).
///
/// The previous version counted the record's `attempts` array and stopped, so a
/// record of twelve EMPTY objects satisfied it, and a record whose own `result`
/// was `fail` satisfied a packet row claiming `pass`. Everything a reviewer
/// actually concluded was unread.
fn verify_review_record(root: &Utf8Path, review: &Review, record: &ReviewRecord) -> Result<()> {
    let who = &review.reviewer;
    require_eq(
        "review record model_family",
        &record.reviewer.model_family,
        who,
    )?;
    require_eq(
        "review record schema_version",
        &record.schema_version,
        "haqp-blind-review-v1",
    )?;
    require_hex_digest(
        "review sanitized_prompt_hash",
        &record.sanitized_prompt_hash,
    )?;
    require_git_object_id("review fixed_base.commit", &record.fixed_base.commit)?;
    require_git_object_id("review fixed_base.tree", &record.fixed_base.tree)?;
    if record.attempts.len() as u64 != review.attempts {
        anyhow::bail!(
            "{who} declares {} attempts but its record contains {}",
            review.attempts,
            record.attempts.len()
        );
    }
    verify_review_attempts(root, &record.fixed_base.commit, who, &record.attempts)?;
    // P1-A06: the packet must not summarize the record more kindly than the
    // record summarizes itself. A row claiming `pass` over a record that says
    // `fail` is the whole failure mode.
    if record.result != review.result {
        anyhow::bail!(
            "{who} declares result {:?} but its record says {:?}",
            review.result,
            record.result
        );
    }
    if record.unresolved_verified_findings != review.unresolved_verified_findings {
        anyhow::bail!(
            "{who} declares {} unresolved verified findings but its record counts {}",
            review.unresolved_verified_findings,
            record.unresolved_verified_findings
        );
    }
    verify_review_findings(review, record, who, root)?;
    // P1-A07: blindness is a property of how the pass was RUN. The runner emits
    // this proof; before F-27 nothing read it back.
    if record.blindness_proof.prior_pass_artifact_supplied {
        anyhow::bail!("{who} was shown a prior pass's artifacts; ADR-0020 §6 requires blindness");
    }
    if record.pass == 2 && !record.blindness_proof.pass_two_original_spec_only {
        anyhow::bail!("{who} is pass 2 but did not start from the original spec alone");
    }
    anyhow::ensure!(
        matches!(
            record.blindness_proof.session_state.as_str(),
            "fresh-codex-session" | "ephemeral-cloud-session"
        ),
        "{who} has unknown blindness session state {:?}",
        record.blindness_proof.session_state
    );
    let expected_ephemeral = record.blindness_proof.session_state == "ephemeral-cloud-session";
    anyhow::ensure!(
        record.blindness_proof.ephemeral_session_requested == expected_ephemeral,
        "{who} blindness session state disagrees with ephemeral_session_requested"
    );
    Ok(())
}

fn verify_review_findings(
    review: &Review,
    record: &ReviewRecord,
    who: &str,
    root: &Utf8Path,
) -> Result<()> {
    require_hex_digest(
        "review isolated_session_hash",
        &record.isolated_session_hash,
    )?;
    let record_findings = review_finding_ids(&record.findings, &record.attempts, who)?;
    let packet_findings = review
        .findings
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if record_findings.len() != review.findings.len() {
        anyhow::bail!(
            "{who} lists {} findings but its record contains {}",
            review.findings.len(),
            record.findings.len()
        );
    }
    if record_findings.keys().copied().collect::<BTreeSet<_>>() != packet_findings {
        anyhow::bail!("{who} packet findings do not match its committed record");
    }
    let mut reproduced = BTreeSet::new();
    for finding_id in &record.independently_reproduced {
        anyhow::ensure!(
            reproduced.insert(finding_id.as_str()),
            "{who}: independently reproduced finding id {finding_id:?} appears twice"
        );
        anyhow::ensure!(
            record_findings.contains_key(finding_id.as_str()),
            "{who}: independently reproduced finding {finding_id:?} is absent from record"
        );
        let attempt_id = record_findings[finding_id.as_str()];
        let attempt = record
            .attempts
            .iter()
            .find(|attempt| attempt.id == attempt_id)
            .expect("finding linkage validated above");
        anyhow::ensure!(
            attempt.independently_reproduced,
            "{who}: finding {finding_id:?} is independently reproduced but its attempt says false"
        );
    }
    let packet_reproduced = review
        .independently_reproduced
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        reproduced == packet_reproduced,
        "{who} packet independent reproductions do not match its committed record"
    );
    for attempt in &record.attempts {
        if attempt.classification == "verified_defect" {
            let linked = record_findings
                .iter()
                .filter(|(_, attempt_id)| **attempt_id == attempt.id)
                .map(|(finding_id, _)| *finding_id)
                .collect::<BTreeSet<_>>();
            anyhow::ensure!(
                linked
                    .iter()
                    .any(|finding_id| reproduced.contains(finding_id)),
                "{who}: verified defect attempt {:?} lacks independent reproduction",
                attempt.id
            );
            verify_review_resolution(root, &record.fixed_base, who, attempt)?;
        }
    }
    let verified_unresolved = record
        .independently_reproduced
        .iter()
        .filter(|finding_id| {
            let Some(attempt_id) = record_findings.get(finding_id.as_str()) else {
                return false;
            };
            record.attempts.iter().any(|attempt| {
                attempt.id == *attempt_id
                    && attempt.classification == "verified_defect"
                    && !attempt.resolved
            })
        })
        .count() as u64;
    if record.unresolved_verified_findings != verified_unresolved {
        anyhow::bail!(
            "{who} record counts {} unresolved verified findings but its attempts prove {verified_unresolved}",
            record.unresolved_verified_findings
        );
    }
    Ok(())
}

fn verify_review_attempts(
    root: &Utf8Path,
    fixed_commit: &str,
    who: &str,
    attempts: &[ReviewRecordAttempt],
) -> Result<()> {
    // P1-A05: the attempts must be attempt RECORDS, not array padding.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut seen_attempts = BTreeSet::new();
    for attempt in attempts {
        for (field, value) in [
            ("id", &attempt.id),
            ("attack_class", &attempt.attack_class),
            ("target", &attempt.target),
            ("attempt", &attempt.attempt),
            ("observed_result", &attempt.observed_result),
            ("classification", &attempt.classification),
        ] {
            if value.trim().is_empty() {
                anyhow::bail!("{who}: attempt {:?} has an empty {field}", attempt.id);
            }
        }
        if !matches!(
            attempt.classification.as_str(),
            "verified_defect" | "false_positive" | "caught_violation"
        ) {
            anyhow::bail!(
                "{who}: attempt {:?} has unknown classification {:?}",
                attempt.id,
                attempt.classification
            );
        }
        if !seen.insert(attempt.id.as_str()) {
            anyhow::bail!("{who}: attempt id {:?} appears twice", attempt.id);
        }
        let (target_file, target_coordinate) =
            attempt.target.split_once(':').with_context(|| {
                format!(
                    "{who}: attempt {:?} target lacks file:coordinate",
                    attempt.id
                )
            })?;
        anyhow::ensure!(
            !target_file.trim().is_empty()
                && !target_coordinate.trim().is_empty()
                && !target_file.starts_with('/')
                && !target_file.split('/').any(|part| part == ".."),
            "{who}: attempt {:?} target must be a safe file:coordinate",
            attempt.id
        );
        let target_lower = target_file.to_ascii_lowercase();
        anyhow::ensure!(
            !target_lower.contains("heldout") && !target_lower.contains("conformance/corpora"),
            "{who}: attempt {:?} target may not name locked acceptance corpus paths",
            attempt.id
        );
        let signature = format!(
            "{}\0{}\0{}\0{}",
            attempt.attack_class, attempt.target, attempt.attempt, attempt.observed_result
        );
        anyhow::ensure!(
            seen_attempts.insert(signature),
            "{who}: attempt {:?} duplicates another substantive falsification attempt",
            attempt.id
        );
        let target_source = git_text(
            root,
            &["show", &format!("{fixed_commit}:{target_file}")],
        )
        .with_context(|| {
            format!(
                "{who}: attempt {:?} target file {target_file:?} is absent at fixed review base",
                attempt.id
            )
        })?;
        let line: usize = target_coordinate.parse().with_context(|| {
            format!(
                "{who}: attempt {:?} target coordinate must be a positive line number",
                attempt.id
            )
        })?;
        anyhow::ensure!(
            line > 0 && line <= target_source.lines().count().max(1),
            "{who}: attempt {:?} target line {line} is outside {target_file}",
            attempt.id
        );
    }
    Ok(())
}

fn verify_review_resolution(
    root: &Utf8Path,
    fixed_base: &ReviewFixedBase,
    who: &str,
    attempt: &ReviewRecordAttempt,
) -> Result<()> {
    if !attempt.resolved {
        return Ok(());
    }
    let resolution = attempt.resolution.as_ref().with_context(|| {
        format!(
            "{who}: resolved finding attempt {:?} has no resolution proof",
            attempt.id
        )
    })?;
    require_git_object_id(&format!("{who} resolution commit"), &resolution.commit)?;
    let fixed_oid = git_text(root, &["rev-parse", &fixed_base.commit])?;
    let resolution_oid = git_text(root, &["rev-parse", &resolution.commit])?;
    anyhow::ensure!(
        fixed_oid != resolution_oid,
        "{who}: resolution commit must be a strict descendant of fixed base"
    );
    git_is_ancestor(root, &fixed_base.commit, &resolution.commit).with_context(|| {
        format!(
            "{who}: resolution commit {} is not a descendant of fixed base {}",
            resolution.commit, fixed_base.commit
        )
    })?;
    git_is_ancestor(root, &resolution.commit, "HEAD").with_context(|| {
        format!(
            "{who}: resolution commit {} is not reachable from current qualification HEAD",
            resolution.commit
        )
    })?;
    let evidence_path = safe_repo_path(root, &resolution.evidence_path, who)?;
    let evidence_bytes = fs::read(&evidence_path)
        .with_context(|| format!("{evidence_path}: resolution evidence is missing"))?;
    let mut hasher = Sha256::new();
    hasher.update(&evidence_bytes);
    let digest = format!("{:x}", hasher.finalize());
    require_eq(
        "resolution evidence_sha256",
        &resolution.evidence_sha256,
        &digest,
    )?;
    anyhow::ensure!(
        !resolution.coordinate.trim().is_empty(),
        "{who}: resolved finding attempt {:?} has empty fix coordinate",
        attempt.id
    );
    require_hex_digest(
        &format!("{who} resolution evidence_sha256"),
        &resolution.evidence_sha256,
    )?;
    let (coordinate_file, coordinate_line) = resolution
        .coordinate
        .rsplit_once(':')
        .with_context(|| format!("{who}: resolution coordinate must be file:line"))?;
    let line: usize = coordinate_line
        .parse()
        .with_context(|| format!("{who}: resolution coordinate line is not numeric"))?;
    anyhow::ensure!(
        line > 0,
        "{who}: resolution coordinate line must be positive"
    );
    anyhow::ensure!(
        !coordinate_file.trim().is_empty()
            && !coordinate_file.starts_with('/')
            && !coordinate_file.split('/').any(|part| part == ".."),
        "{who}: resolution coordinate path must stay inside repository"
    );
    let source = git_text(
        root,
        &["show", &format!("{}:{coordinate_file}", resolution.commit)],
    )?;
    anyhow::ensure!(
        line <= source.lines().count().max(1),
        "{who}: resolution coordinate line {line} is outside {coordinate_file}"
    );
    let changed = git_text(
        root,
        &[
            "diff",
            "--name-only",
            &format!("{}..{}", fixed_base.commit, resolution.commit),
            "--",
            coordinate_file,
        ],
    )?;
    anyhow::ensure!(
        changed.lines().any(|path| path == coordinate_file),
        "{who}: resolution commit did not change coordinate file {coordinate_file}"
    );
    Ok(())
}

fn review_finding_ids<'a>(
    findings: &'a [serde_json::Value],
    attempts: &'a [ReviewRecordAttempt],
    who: &str,
) -> Result<BTreeMap<&'a str, &'a str>> {
    let attempts_by_id = attempts
        .iter()
        .map(|attempt| (attempt.id.as_str(), attempt))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeMap::new();
    for finding in findings {
        let id = finding
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .with_context(|| format!("{who}: record finding has no non-empty id"))?;
        let attempt_id = finding
            .get("attempt_id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .with_context(|| format!("{who}: finding {id:?} has no non-empty attempt_id"))?;
        let attempt = attempts_by_id.get(attempt_id).with_context(|| {
            format!("{who}: finding {id:?} names unknown attempt {attempt_id:?}")
        })?;
        anyhow::ensure!(
            attempt.classification == "verified_defect",
            "{who}: finding {id:?} must link to a verified_defect attempt"
        );
        anyhow::ensure!(
            ids.insert(id, attempt_id).is_none(),
            "{who}: record finding id {id:?} appears twice"
        );
    }
    Ok(ids)
}

fn require_hex_digest(label: &str, digest: &str) -> Result<()> {
    anyhow::ensure!(
        digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
        "{label} must be a 64-hex digest"
    );
    Ok(())
}

fn require_git_object_id(label: &str, digest: &str) -> Result<()> {
    anyhow::ensure!(
        matches!(digest.len(), 40 | 64) && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
        "{label} must be a 40- or 64-hex Git object id"
    );
    Ok(())
}

fn git_is_ancestor(root: &Utf8Path, ancestor: &str, descendant: &str) -> Result<()> {
    let status = Command::new("git")
        .current_dir(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()
        .with_context(|| format!("check Git ancestry {ancestor} -> {descendant}"))?;
    anyhow::ensure!(
        status.success(),
        "Git ancestry check failed for {ancestor} -> {descendant}"
    );
    Ok(())
}

fn safe_repo_path(root: &Utf8Path, value: &str, who: &str) -> Result<Utf8PathBuf> {
    let candidate = Utf8Path::new(value);
    anyhow::ensure!(
        !candidate.is_absolute()
            && !candidate
                .components()
                .any(|part| part == camino::Utf8Component::ParentDir),
        "{who} resolution evidence path {value:?} escapes repository"
    );
    let path = root.join(candidate);
    let canonical_root =
        fs::canonicalize(root.as_std_path()).context("canonicalize repository root")?;
    let canonical_path = fs::canonicalize(&path)
        .with_context(|| format!("{path}: resolution evidence is missing"))?;
    anyhow::ensure!(
        canonical_path.starts_with(&canonical_root),
        "{who} resolution evidence path resolves outside repository"
    );
    Utf8PathBuf::from_path_buf(canonical_path)
        .map_err(|_| anyhow::anyhow!("{path}: resolution evidence path is not UTF-8"))
}

/// ADR-0020 §6's independence, checked rather than assumed (M17.5 F-27 / P1-A07).
///
/// The runner records `reviewer.model_family`, `reviewer.backend` and an
/// identity hash precisely so independence is auditable. The qualification
/// verifier never looked at any of them — this session BUILT that evidence and
/// then failed to check it, which is the same mistake as trusting a status
/// string. Two reviews from one family satisfy every other check in this file.
fn verify_reviewer_independence(records: &[(String, ReviewRecord)]) -> Result<()> {
    // M17.5 Stage 3: `< 2` -> `> 2` survives here and is EQUIVALENT, not
    // untested. With fewer than two records the loop below builds sets of size
    // `records.len()`, and `distinct != records.len()` is then false, so the
    // function returns Ok either way. The early return states the intent —
    // independence is a claim about a PAIR — and costs nothing.
    if records.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        records.len() == 2,
        "blind review evidence must contain exactly two records"
    );
    let passes = records
        .iter()
        .map(|(_, record)| record.pass)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        passes == BTreeSet::from([1, 2]),
        "blind review records must have exactly pass ordinals 1 and 2"
    );
    let mut families: BTreeSet<&str> = BTreeSet::new();
    let mut backends: BTreeSet<&str> = BTreeSet::new();
    let mut identities: BTreeSet<&str> = BTreeSet::new();
    let mut prompts: BTreeSet<&str> = BTreeSet::new();
    for (who, record) in records {
        if record.reviewer.model_family.trim().is_empty() {
            anyhow::bail!("{who}: record names no model family");
        }
        if record.reviewer.identity_hash.trim().is_empty() {
            anyhow::bail!("{who}: record carries no reviewer identity hash");
        }
        require_hex_digest(
            &format!("{who} reviewer identity_hash"),
            &record.reviewer.identity_hash,
        )?;
        require_hex_digest(
            &format!("{who} sanitized_prompt_hash"),
            &record.sanitized_prompt_hash,
        )?;
        families.insert(record.reviewer.model_family.as_str());
        backends.insert(record.reviewer.backend.as_str());
        identities.insert(record.reviewer.identity_hash.as_str());
        prompts.insert(record.sanitized_prompt_hash.as_str());
    }
    for (label, distinct) in [
        ("model families", families.len()),
        ("backends", backends.len()),
        ("reviewer identities", identities.len()),
        ("sanitized prompts", prompts.len()),
    ] {
        if distinct != records.len() {
            anyhow::bail!(
                "{} reviews share {label} ({distinct} distinct across {} reviews); \
                 ADR-0020 §6 requires distinct model families with isolated state",
                records.len(),
                records.len()
            );
        }
    }
    Ok(())
}

/// Every declared test name must name a test that EXISTS (M17.5 pass-2 #11).
///
/// `Test.name` was carried as an opaque string and never checked, so renaming a
/// row to `does_not_exist` changed nothing — the traceability matrix could name
/// 38 tests none of which were real.
///
/// This runs only at the QUALIFIED layer, not the inventory layer, and that
/// asymmetry is deliberate: the proposed packet legitimately predeclares tests
/// that milestones M19–M24 have not yet written, which is the whole point of an
/// inventory. But a packet claiming `qualification_state: complete` while
/// naming a test nobody wrote is exactly the false green this gate exists to
/// stop.
/// The declared stage must match what the packet actually claims (ADR-0021).
///
/// HAQP-1 is evaluated in two stages because ADR-0020 §3 assumed the suite it
/// mutates already exists, and for Phase 1 it does not (F-28). Stage `1a` is
/// everything provable before Phase 1 authorization; `1b` adds the mutation
/// requirement at M24.
///
/// Both directions are errors. A `1a` packet claiming kills would smuggle the
/// deferred requirement back in as an unwitnessed assertion — the exact thing
/// the deferral exists to prevent. A `1b` packet without kills would claim the
/// stronger stage while supplying the weaker evidence.
fn verify_qualification_stage(packet: &Packet) -> Result<()> {
    let claims_resolved = packet
        .mutants
        .iter()
        .any(|mutant| mutant.disposition != "predeclared");
    match packet.qualification_stage.as_str() {
        "1a" if claims_resolved => anyhow::bail!(
            "packet declares stage 1a but claims evaluated mutants; ADR-0021 defers the \
             mutation requirement to 1b at M24, when the runner can produce evidence"
        ),
        "1b" if !claims_resolved => anyhow::bail!(
            "packet declares stage 1b, which carries ADR-0020 §3's mutation \
             requirement, but no mutant is killed or otherwise evaluated"
        ),
        "1a" | "1b" => Ok(()),
        other => anyhow::bail!("unknown qualification stage {other:?}; expected 1a or 1b"),
    }
}

/// A mutant claiming `killed` must be killed by a test that CAN RUN
/// (M17.5 F-27 / P1-A02, F-28).
///
/// ADR-0020 §4 wants at least 64 semantic mutants at a 100% kill rate. The
/// packet declares 65, each naming the tests that would catch it — and as of
/// 2026-08-03 **not one of those 35 distinct tests is runnable**: 27 exist only
/// as strings in the packet (M18–M24 have not been authorized, so
/// `conformance/tests/milestones/` has no m18..m24 at all), and the remaining 8
/// exist but are `#[ignore]`d under the AM-17.2 Phase 1 quarantine.
///
/// A kill demonstrated by a test that does not exist is not demonstrated. A
/// kill demonstrated by a test nobody runs is not demonstrated either — an
/// `#[ignore]`d test cannot fail, so it cannot witness anything.
///
/// This check exists so that gap cannot be closed by editing dispositions to
/// `killed`. It is deliberately unsatisfiable today, and that is the correct
/// state: the mutation requirement is GATED on the Phase 1 milestones writing
/// their tests, not on anything M17.5 can produce.
fn verify_mutant_killing_tests(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let killed: Vec<&Mutant> = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition == "killed")
        .collect();
    if killed.is_empty() {
        return Ok(());
    }
    let by_id: BTreeMap<&str, &Test> = packet
        .tests
        .iter()
        .map(|test| (test.id.as_str(), test))
        .collect();
    let ignored = ignored_test_names(root);
    for mutant in killed {
        if mutant.killing_tests.is_empty() {
            anyhow::bail!("{} claims killed but names no killing test", mutant.id);
        }
        for test_id in &mutant.killing_tests {
            let test = by_id.get(test_id.as_str()).with_context(|| {
                format!(
                    "{} names killing test {test_id}, which the packet does not declare",
                    mutant.id
                )
            })?;
            let leaf = test.name.rsplit("::").next().unwrap_or(test.name.as_str());
            if ignored.contains(leaf) {
                anyhow::bail!(
                    "{} claims killed by {} ({}), which is #[ignore]d — an ignored test \
                     never runs, so it cannot witness a kill",
                    mutant.id,
                    test_id,
                    test.name
                );
            }
        }
    }
    Ok(())
}

fn verify_mutant_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/mutants.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed mutant evidence is required; run `just haq-mutants`")
    })?;
    let evidence: MutantEvidence =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "mutant evidence schema_version",
        &evidence.schema_version,
        "haqp-mutants-v1",
    )?;
    require_eq(
        "mutant evidence source_commit",
        &evidence.source_commit,
        &packet
            .provenance
            .as_ref()
            .context("qualified mutant evidence requires packet provenance")?
            .fixed_commit,
    )?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    require_eq(
        "mutant evidence lockfile_blake3",
        &evidence.lockfile_blake3,
        &lockfile_blake3,
    )?;
    require_eq(
        "mutant evidence runner",
        &evidence.runner,
        "liminal-xtask haq mutants",
    )?;

    let declared = packet
        .mutants
        .iter()
        .map(|mutant| (mutant.id.as_str(), mutant))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::<String>::new();
    for row in &evidence.rows {
        verify_mutant_evidence_row(row, &declared, &mut seen)?;
    }
    let expected = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition != "predeclared")
        .map(|mutant| mutant.id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        seen == expected,
        "mutant evidence rows differ: recorded={seen:?}, expected={expected:?}"
    );
    Ok(())
}

fn verify_mutant_evidence_row(
    row: &MutantEvidenceRow,
    declared: &BTreeMap<&str, &Mutant>,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    anyhow::ensure!(
        seen.insert(row.id.clone()),
        "duplicate mutant evidence row {}",
        row.id
    );
    let mutant = declared
        .get(row.id.as_str())
        .with_context(|| format!("mutant evidence names undeclared {}", row.id))?;
    anyhow::ensure!(
        mutant.disposition != "predeclared",
        "predeclared mutant {} has evaluated evidence",
        row.id
    );
    require_eq(
        "mutant evidence disposition",
        &row.disposition,
        &mutant.disposition,
    )?;
    let mutant_patch = mutant
        .patch
        .as_ref()
        .context("evaluated mutant lacks patch")?;
    anyhow::ensure!(
        row.patch.as_ref() == Some(mutant_patch),
        "{} evidence patch differs from packet",
        row.id
    );
    anyhow::ensure!(
        row.runnable_tests.len() == row.exit_codes.len()
            && row.runnable_tests.len() == row.baseline_exit_codes.len()
            && row.runnable_tests.len() == row.stdout_blake3.len()
            && row.runnable_tests.len() == row.stderr_blake3.len(),
        "{} evidence vectors have different lengths",
        row.id
    );
    anyhow::ensure!(
        row.baseline_exit_codes.iter().all(|code| *code == 0),
        "{} baseline witness suite failed before mutation",
        row.id
    );
    for witness in &mutant.killing_tests {
        anyhow::ensure!(
            row.runnable_tests.iter().any(|test| test == witness),
            "{} evidence omits declared killing test {}",
            row.id,
            witness
        );
    }
    for digest in row.stdout_blake3.iter().chain(row.stderr_blake3.iter()) {
        anyhow::ensure!(
            digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
            "{} has invalid output digest",
            row.id
        );
    }
    match row.disposition.as_str() {
        "killed" => {
            require_eq("mutant evidence status", &row.status, "killed")?;
            anyhow::ensure!(
                !row.failed_tests.is_empty(),
                "{} claims killed with no failed test",
                row.id
            );
            let mut seen_failed = BTreeSet::new();
            for failed in &row.failed_tests {
                anyhow::ensure!(
                    seen_failed.insert(failed),
                    "{} lists failed test {:?} more than once",
                    row.id,
                    failed
                );
                let index = row
                    .runnable_tests
                    .iter()
                    .position(|test| test == failed)
                    .with_context(|| {
                        format!(
                            "{} lists failed test {:?} that was not runnable",
                            row.id, failed
                        )
                    })?;
                anyhow::ensure!(
                    row.exit_codes[index] != 0,
                    "{} lists failed test {:?} with zero exit code",
                    row.id,
                    failed
                );
            }
        }
        "equivalent" | "duplicate" => {
            require_eq(
                "mutant evidence status",
                &row.status,
                row.disposition.as_str(),
            )?;
        }
        other => anyhow::bail!("{} has unsupported evaluated disposition {other:?}", row.id),
    }
    Ok(())
}

fn validate_mutant_patch(patch: &MutantPatch) -> Result<()> {
    anyhow::ensure!(!patch.file.trim().is_empty(), "mutant patch names no file");
    let source_path = Path::new(&patch.file);
    anyhow::ensure!(
        !source_path.is_absolute(),
        "mutant patch path {:?} is absolute",
        patch.file
    );
    anyhow::ensure!(
        !source_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }),
        "mutant patch path {:?} escapes worktree",
        patch.file
    );
    anyhow::ensure!(
        !patch.before.is_empty(),
        "mutant patch {:?} has empty before text",
        patch.file
    );
    Ok(())
}

fn apply_mutant_patch(root: &Utf8Path, patch: &MutantPatch) -> Result<()> {
    validate_mutant_patch(patch)?;
    let target_path = root.join(&patch.file);
    let text = fs::read_to_string(&target_path)
        .with_context(|| format!("read mutant target {target_path}"))?;
    let matches = text.match_indices(&patch.before).count();
    anyhow::ensure!(
        matches == 1,
        "mutant patch {:?} expected one before occurrence, found {matches}",
        patch.file
    );
    let replaced = text.replacen(&patch.before, &patch.after, 1);
    fs::write(&target_path, replaced)
        .with_context(|| format!("write mutant target {target_path}"))?;
    Ok(())
}

fn validate_disposition(mutant: &Mutant) -> Result<()> {
    if mutant.disposition == "predeclared" {
        return Ok(());
    }
    let patch = mutant
        .patch
        .as_ref()
        .with_context(|| format!("{} has evaluated disposition but no patch", mutant.id))?;
    validate_mutant_patch(patch)?;
    if matches!(mutant.disposition.as_str(), "equivalent" | "duplicate") {
        anyhow::ensure!(
            mutant
                .disposition_proof
                .as_deref()
                .is_some_and(|proof| !proof.trim().is_empty()),
            "{} requires written proof for {} disposition",
            mutant.id,
            mutant.disposition
        );
        let mut concurrence = BTreeSet::new();
        for reviewer in &mutant.disposition_concurrence {
            anyhow::ensure!(
                !reviewer.trim().is_empty(),
                "{} has empty concurrence",
                mutant.id
            );
            concurrence.insert(reviewer);
        }
        anyhow::ensure!(
            concurrence.len() >= 2,
            "{} requires concurrence from both adversarial passes",
            mutant.id
        );
    }
    if mutant.disposition == "killed" {
        anyhow::ensure!(
            !mutant.killing_tests.is_empty(),
            "{} claims killed but names no killing test",
            mutant.id
        );
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn git_text(root: &Utf8Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("run git {args:?}"))?;
    anyhow::ensure!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn regex_literal(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                vec!['\\', ch]
            }
            other => vec![other],
        })
        .collect()
}

fn run_one_mutant_test(
    worktree: &Utf8Path,
    test_name: &str,
    run_ignored: bool,
) -> Result<(i32, Vec<u8>, Vec<u8>)> {
    let leaf = test_name.rsplit("::").next().unwrap_or(test_name);
    let expression = format!("test(/^{}$/)", regex_literal(leaf));
    let mut command = Command::new("cargo");
    command
        .current_dir(worktree)
        .args([
            "nextest",
            "run",
            "--workspace",
            "--all-features",
            "--profile",
            "ci",
        ])
        .arg("-E")
        .arg(expression);
    if run_ignored {
        command.args(["--run-ignored", "all"]);
    }
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run mutation witness {test_name}"))?;
    let code = output.status.code().unwrap_or(-1);
    Ok((code, output.stdout, output.stderr))
}

fn run_mutant_row(
    worktree: &Utf8Path,
    packet: &Packet,
    mutant: &Mutant,
    ignored: &BTreeSet<String>,
    run_ignored: bool,
) -> Result<MutantEvidenceRow> {
    let Some(patch) = mutant.patch.clone() else {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "not-ready".to_owned(),
            patch: None,
            runnable_tests: Vec::new(),
            baseline_exit_codes: Vec::new(),
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some("no patch declared".to_owned()),
        });
    };
    let runnable = mutant
        .killing_tests
        .iter()
        .filter_map(|test_id| packet.tests.iter().find(|test| &test.id == test_id))
        .map(|test| test.name.clone())
        .filter(|name| run_ignored || !ignored.contains(name.rsplit("::").next().unwrap_or(name)))
        .collect::<Vec<_>>();
    if runnable.is_empty() {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "not-runnable".to_owned(),
            patch: Some(patch),
            runnable_tests: Vec::new(),
            baseline_exit_codes: Vec::new(),
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some("no runnable killing tests".to_owned()),
        });
    }
    let baseline_exit_codes = runnable
        .iter()
        .map(|test_name| {
            run_one_mutant_test(worktree, test_name, run_ignored).map(|(code, _, _)| code)
        })
        .collect::<Result<Vec<_>>>()?;
    if let Err(error) = apply_mutant_patch(worktree, &patch) {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "error".to_owned(),
            patch: Some(patch),
            runnable_tests: runnable,
            baseline_exit_codes,
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some(error.to_string()),
        });
    }
    let mut failed_tests = Vec::new();
    let mut exit_codes = Vec::new();
    let mut stdout_blake3 = Vec::new();
    let mut stderr_blake3 = Vec::new();
    for test_name in &runnable {
        let (code, stdout, stderr) = run_one_mutant_test(worktree, test_name, run_ignored)?;
        if code != 0 {
            failed_tests.push(test_name.clone());
        }
        exit_codes.push(code);
        stdout_blake3.push(hex_digest(&stdout));
        stderr_blake3.push(hex_digest(&stderr));
    }
    let status = if failed_tests.is_empty() {
        "survived"
    } else {
        "killed"
    };
    Ok(MutantEvidenceRow {
        id: mutant.id.clone(),
        disposition: mutant.disposition.clone(),
        status: status.to_owned(),
        patch: Some(patch),
        runnable_tests: runnable,
        baseline_exit_codes,
        failed_tests,
        exit_codes,
        stdout_blake3,
        stderr_blake3,
        reason: None,
    })
}

fn reset_mutant_worktree(worktree: &Utf8Path, source_commit: &str) -> Result<()> {
    let reset = Command::new("git")
        .current_dir(worktree)
        .args(["reset", "--hard", "--quiet", source_commit])
        .output()?;
    anyhow::ensure!(reset.status.success(), "reset mutant worktree failed");
    let clean = Command::new("git")
        .current_dir(worktree)
        .args(["clean", "-fdx", "-q"])
        .output()?;
    anyhow::ensure!(clean.status.success(), "clean mutant worktree failed");
    Ok(())
}

struct WorktreeGuard {
    repo: Utf8PathBuf,
    path: Utf8PathBuf,
    active: bool,
}

impl WorktreeGuard {
    fn new(repo: &Utf8Path, path: &Utf8Path) -> Self {
        Self {
            repo: repo.to_owned(),
            path: path.to_owned(),
            active: true,
        }
    }

    fn remove(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let output = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "remove", "-f"])
            .arg(&self.path)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        self.active = false;
        Ok(())
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = Command::new("git")
                .current_dir(&self.repo)
                .args(["worktree", "remove", "-f"])
                .arg(&self.path)
                .output();
        }
    }
}

/// Execute source patches in disposable git worktrees. This command writes only
/// digests and statuses; raw test/model output never enters the repository.
pub fn run_mutants_repo(root: &Utf8Path, ids: &[String], run_ignored: bool) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    anyhow::ensure!(
        git_text(root, &["status", "--porcelain"])?.is_empty(),
        "mutant runner requires a clean source tree"
    );
    let source_commit = git_text(root, &["rev-parse", "HEAD"])?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    let selected = if ids.is_empty() {
        packet.mutants.iter().collect::<Vec<_>>()
    } else {
        let wanted = ids.iter().cloned().collect::<BTreeSet<_>>();
        let selected = packet
            .mutants
            .iter()
            .filter(|mutant| wanted.contains(&mutant.id))
            .collect::<Vec<_>>();
        let present = selected
            .iter()
            .map(|mutant| mutant.id.clone())
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            present.len() == wanted.len(),
            "unknown mutant id(s): {:?}",
            wanted.difference(&present).collect::<Vec<_>>()
        );
        selected
    };

    let ignored = ignored_test_names(root);
    let scratch = liminal_scratch::ScratchDir::new("haq-mutants")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(&source_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut worktree_guard = WorktreeGuard::new(root, &worktree);

    let mut rows = Vec::with_capacity(selected.len());
    for mutant in selected {
        rows.push(run_mutant_row(
            &worktree,
            &packet,
            mutant,
            &ignored,
            run_ignored,
        )?);
        reset_mutant_worktree(&worktree, &source_commit)?;
    }
    worktree_guard.remove()?;
    drop(scratch);

    let evidence = MutantEvidence {
        schema_version: "haqp-mutants-v1".to_owned(),
        source_commit,
        lockfile_blake3,
        runner: "liminal-xtask haq mutants".to_owned(),
        rows,
    };
    let path = root.join("conformance/haqp/evidence/mutants.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, serde_json::to_vec_pretty(&evidence)?)?;
    println!(
        "mutant evidence written to {path}; rows={}",
        evidence.rows.len()
    );
    Ok(())
}

/// Leaf names of every `#[ignore]`d test in the tree.
fn ignored_test_names(root: &Utf8Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                collect_ignored(&text, &mut names);
            }
        }
    }
    names
}

/// An `#[ignore]` attribute applies to the next `fn` declaration.
fn collect_ignored(text: &str, names: &mut BTreeSet<String>) {
    let mut pending = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        // A test is non-runnable if it is ignored OUTRIGHT, ignored
        // CONDITIONALLY, or compiled out. All three cannot witness a kill, and
        // only the first was recognised: `#[cfg_attr(unix, ignore)]` and
        // `#[cfg(any())]` both vanish at run time while reading as live source.
        if trimmed.starts_with("#[ignore")
            || (trimmed.starts_with("#[cfg_attr(") && trimmed.contains("ignore"))
            || trimmed.starts_with("#[cfg(any())]")
        {
            pending = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("fn ") {
            if pending && let Some(name) = rest.split('(').next() {
                names.insert(name.trim().to_owned());
            }
            pending = false;
        }
    }
}

fn verify_test_names_exist(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut runnable = BTreeMap::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                collect_runnable_test_functions(&text, &mut runnable);
            }
        }
    }

    let mut missing = Vec::new();
    for test in &packet.tests {
        // Names are module-qualified (`milestones::m19::foo`); the test itself
        // is the final segment.
        let leaf = test.name.rsplit("::").next().unwrap_or(test.name.as_str());
        if leaf.trim().is_empty() {
            anyhow::bail!("{} declares an empty test name", test.id);
        }
        if !runnable.contains_key(leaf) {
            missing.push(format!("{} -> {}", test.id, test.name));
        } else if runnable.get(leaf) != Some(&1) {
            missing.push(format!(
                "{} -> {} (leaf name is ambiguous; use a unique module-qualified test)",
                test.id, test.name
            ));
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "qualification claims {} test(s) that do not exist in the tree: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    Ok(())
}

fn collect_runnable_test_functions(text: &str, names: &mut BTreeMap<String, usize>) {
    let mut test_pending = false;
    let mut excluded = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[test]") {
            test_pending = true;
            excluded = false;
        } else if test_pending && (trimmed.starts_with("#[ignore") || trimmed.starts_with("#[cfg("))
        {
            excluded = true;
        } else if test_pending {
            let declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            let declaration = declaration.strip_prefix("async ").unwrap_or(declaration);
            if let Some(rest) = declaration.strip_prefix("fn ") {
                if let Some(name) = rest
                    .split('(')
                    .next()
                    .filter(|name| !name.trim().is_empty())
                    && !excluded
                {
                    *names.entry(name.trim().to_owned()).or_default() += 1;
                }
                test_pending = false;
            }
        }
    }
}

/// The recorded fuzz campaign is too long to re-run per verification (150
/// target-minutes), so its artifact is COMMITTED and the packet must agree
/// with it exactly. Cheap lanes are re-run instead — see `haq verify-full`.
/// Target identity is checked here; family budget reconciliation happens in
/// `verify_fuzz_budget`, which derives each family's minutes from its declared
/// target mapping. There is one source of campaign duration truth: this
/// artifact's per-target seconds.
fn verify_fuzz_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let bytes =
        fs::read(&path).with_context(|| format!("{path}: committed fuzz evidence is required"))?;
    let recorded: Vec<FuzzEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;

    // M17.5 pass-2 #17: the count was compared against `packet.generated.len()`,
    // which is a category error — fuzz TARGETS are not generated FAMILIES. The
    // the counts can differ, so comparing them would be a category error and
    // could accept a row naming a target that does not exist.
    // Bind to the targets that actually exist in the tree instead.
    let mut present = BTreeSet::new();
    let targets_dir = root.join("fuzz/fuzz_targets");
    let entries = fs::read_dir(&targets_dir)
        .with_context(|| format!("{targets_dir}: fuzz targets are required to verify the lane"))?;
    for entry in entries.flatten() {
        let Ok(file) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if file.extension() == Some("rs")
            && let Some(stem) = file.file_stem()
        {
            present.insert(stem.to_owned());
        }
    }
    verify_fuzz_rows(&recorded, &present)?;
    verify_fuzz_logs(root, &recorded)?;
    verify_fuzz_budget(&packet.generated, &recorded)
}

fn verify_fuzz_rows(recorded: &[FuzzEvidence], present: &BTreeSet<String>) -> Result<()> {
    let declared = recorded
        .iter()
        .map(|row| row.target.clone())
        .collect::<BTreeSet<_>>();
    if &declared != present {
        anyhow::bail!(
            "fuzz evidence does not cover the targets in the tree: recorded={declared:?}, \
             present={present:?}"
        );
    }
    require_unique(
        recorded.iter().map(|row| row.target.as_str()),
        "fuzz target",
    )?;

    // M17.5 F-17: `log` names a gitignored path, so the counts it would
    // substantiate were unfalsifiable once the run ended. The digest makes a
    // later-produced log checkable.
    for row in recorded {
        if row.log_blake3.len() != 64 || !row.log_blake3.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!(
                "{}: log_blake3 {:?} is not a 64-hex digest, so its log can never be checked",
                row.target,
                row.log_blake3
            );
        }
    }

    // M17.5 F-27 / P1-A09: ADR-0020 §4 requires a SANITIZER-ENABLED campaign
    // and the evidence had no way to say whether one ran. An unsanitized
    // campaign finds crashes it can see and silently misses every
    // memory-safety defect it cannot.
    for row in recorded {
        if !matches!(
            row.sanitizer.as_str(),
            "address" | "memory" | "thread" | "leak"
        ) {
            anyhow::bail!(
                "{}: sanitizer {:?} is not a sanitizer ADR-0020 §4 accepts",
                row.target,
                row.sanitizer
            );
        }
    }

    let total_minutes: u64 = recorded.iter().map(|row| row.seconds / 60).sum();
    if total_minutes < 150 {
        anyhow::bail!("recorded fuzz budget {total_minutes} target-minutes is below 150");
    }
    for row in recorded {
        if row.exit_code != 0 {
            anyhow::bail!(
                "fuzz target {} exited {} with {} artifact(s) — every crash is a failing \
                 result (ADR-0020 §4)",
                row.target,
                row.exit_code,
                row.artifacts
            );
        }
        if row.artifacts != 0 {
            anyhow::bail!(
                "fuzz target {} left {} artifacts",
                row.target,
                row.artifacts
            );
        }
        if row.execs == 0 {
            anyhow::bail!("fuzz target {} recorded zero executions", row.target);
        }
        if row.log.trim().is_empty() {
            anyhow::bail!("fuzz target {} records no log path", row.target);
        }
        // ADR-0020 line 90: "each family also receives one 30-minute
        // sanitizer-enabled fuzz campaign". That is a per-target floor, not just
        // a contribution to the 150 target-minute total — without it one long
        // target could carry the sum while another ran for seconds.
        //
        // The same 30-minute floor is used by family budget reconciliation.
        if row.seconds < 30 * 60 {
            anyhow::bail!(
                "fuzz target {} was budgeted {}s, below the 30-minute per-target floor \
                 (ADR-0020)",
                row.target,
                row.seconds
            );
        }
        // A clean campaign runs to its budget. Finishing far short of it means
        // the target stopped early, and a clean early exit is a contradiction
        // the evidence should not be able to state.
        if row.elapsed_s + 60 < row.seconds {
            anyhow::bail!(
                "fuzz target {} claims a clean {}s campaign but only ran {}s — an early \
                 exit with exit_code 0 and no artifacts is not a coherent result",
                row.target,
                row.seconds,
                row.elapsed_s
            );
        }
    }
    // `haqp_fuzz_campaign.sh` drives every target from ONE `SEED`, so the rows
    // must agree. Checking only for a nonzero seed would let the comment claim
    // more than the code enforced — rows carrying three different seeds describe
    // three campaigns, and none of them is the one the packet points at.
    let seeds = recorded.iter().map(|row| row.seed).collect::<BTreeSet<_>>();
    match seeds.iter().copied().next() {
        None => anyhow::bail!("fuzz evidence records no targets at all"),
        Some(0) => {
            anyhow::bail!(
                "fuzz evidence records no campaign seed, so the lane cannot be reproduced"
            )
        }
        Some(_) if seeds.len() > 1 => anyhow::bail!(
            "fuzz evidence carries {} distinct seeds {seeds:?}; one campaign has one seed, so \
             these rows describe runs the packet cannot point at",
            seeds.len()
        ),
        Some(_) => {}
    }
    Ok(())
}

/// Counts and exit states are only useful if their retained logs are the bytes
/// the campaign actually produced. The release artifact keeps one log per
/// target under the tracked evidence directory and binds its digest here.
fn verify_fuzz_logs(root: &Utf8Path, recorded: &[FuzzEvidence]) -> Result<()> {
    for row in recorded {
        let expected = format!("conformance/haqp/evidence/logs/{}.log", row.target);
        require_eq("fuzz log path", &row.log, &expected)?;
        let path = root.join(&expected);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("{path}: retained fuzz log is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{path}: retained fuzz log must be a regular file",
        );
        let bytes = fs::read(&path).with_context(|| format!("read retained fuzz log {path}"))?;
        let digest = blake3::hash(&bytes).to_hex().to_string();
        require_eq("fuzz log_blake3", &row.log_blake3, &digest)?;
    }
    Ok(())
}

/// Bind the locked-corpus prohibition to an OS-level path audit rather than
/// trusting `locked_acceptance_corpora_touched: false`. The audit records only
/// normalized paths, never corpus bytes; forbidden held-out paths fail closed.
fn verify_corpus_access_audit(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/corpus-access.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed corpus-access audit is required; run the traced fuzz lane")
    })?;
    let audit: CorpusAccessAudit =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "corpus access audit schema_version",
        &audit.schema_version,
        "haqp-corpus-access-v1",
    )?;
    require_eq(
        "corpus access audit tracer",
        &audit.tracer,
        "strace-open-paths",
    )?;

    let expected = packet
        .generated
        .iter()
        .flat_map(|family| family.fuzz_targets.iter().cloned())
        .collect::<BTreeSet<_>>();
    let actual = audit
        .targets
        .iter()
        .map(|row| row.target.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        actual == expected,
        "corpus access audit target set differs: recorded={actual:?}, expected={expected:?}"
    );
    require_unique(
        audit.targets.iter().map(|row| row.target.as_str()),
        "corpus access audit target",
    )?;
    for row in &audit.targets {
        let expected_manifest = format!("conformance/haqp/evidence/access/{}.paths", row.target);
        require_eq(
            "corpus access manifest path",
            &row.manifest,
            &expected_manifest,
        )?;
        let manifest = root.join(&expected_manifest);
        let metadata = fs::symlink_metadata(&manifest)
            .with_context(|| format!("{manifest}: corpus access manifest is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{manifest}: corpus access manifest must be a regular file"
        );
        let manifest_bytes = fs::read(&manifest).with_context(|| format!("read {manifest}"))?;
        let manifest_digest = blake3::hash(&manifest_bytes).to_hex();
        require_eq(
            "corpus access manifest_blake3",
            &row.manifest_blake3,
            manifest_digest.as_ref(),
        )?;
        let manifest_text = String::from_utf8(manifest_bytes)
            .with_context(|| format!("{manifest}: corpus access manifest is not UTF-8"))?;
        anyhow::ensure!(
            !manifest_text.trim().is_empty(),
            "{manifest}: corpus access manifest is empty"
        );
        let manifest_lower = manifest_text.to_ascii_lowercase();
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !manifest_lower.contains(forbidden),
                "{manifest}: corpus access audit observed forbidden path fragment {forbidden:?}"
            );
        }
    }
    Ok(())
}

/// Check the bounded-campaign clock artifact. One clean breach is retained as
/// residual risk; two clean breaches block ratification per ADR-0020 §7.
fn verify_campaign_clock(root: &Utf8Path) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/campaign.json");
    let bytes = fs::read(&path)
        .with_context(|| format!("{path}: committed campaign clock evidence is required"))?;
    let clock: CampaignClock =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "campaign clock schema_version",
        &clock.schema_version,
        "haqp-campaign-clock-v1",
    )?;
    anyhow::ensure!(
        !clock.reference_machine.trim().is_empty(),
        "campaign clock has no reference machine"
    );
    require_unique(clock.runs.iter().map(|row| row.id.as_str()), "campaign run")?;
    anyhow::ensure!(!clock.runs.is_empty(), "campaign clock records no runs");
    for run in &clock.runs {
        anyhow::ensure!(!run.id.trim().is_empty(), "campaign run has empty id");
        anyhow::ensure!(
            run.elapsed_s > 0,
            "campaign run {} has zero elapsed seconds",
            run.id
        );
        require_eq("campaign run result", &run.result, "pass")?;
        anyhow::ensure!(run.clean, "campaign run {} was not clean", run.id);
    }
    let breaches = clock
        .runs
        .iter()
        .filter(|run| run.elapsed_s > 8 * 60 * 60)
        .count();
    anyhow::ensure!(
        breaches < 2,
        "campaign exceeded the eight-hour ceiling on {breaches} clean runs; ratification is blocked"
    );
    Ok(())
}

/// Every limitation carried into qualification needs an owner, trigger,
/// evidence coordinate, mitigation, and acceptance authority. Bare prose in a
/// residual-risk table is not auditable (ADR-0020 §7).
fn verify_residual_risks(packet: &Packet) -> Result<()> {
    anyhow::ensure!(
        !packet.residual_risks.is_empty(),
        "qualified packet must record residual-risk coordinates"
    );
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        packet.residual_risks.iter().map(|row| row.id.as_str()),
        "residual risk",
    )?;
    for risk in &packet.residual_risks {
        for (label, value) in [
            ("owner", &risk.owner),
            ("severity", &risk.severity),
            ("trigger", &risk.trigger),
            ("requirement", &risk.requirement),
            ("evidence", &risk.evidence),
            ("planned_phase", &risk.planned_phase),
            ("mitigation", &risk.mitigation),
            ("authority", &risk.authority),
            ("state", &risk.state),
        ] {
            anyhow::ensure!(
                !value.trim().is_empty(),
                "risk {} has empty {label}",
                risk.id
            );
        }
        anyhow::ensure!(
            requirement_ids.contains(risk.requirement.as_str()),
            "risk {} names unknown requirement {}",
            risk.id,
            risk.requirement
        );
        anyhow::ensure!(
            risk.evidence.contains(':')
                || (risk.evidence.len() == 64
                    && risk.evidence.chars().all(|ch| ch.is_ascii_hexdigit())),
            "risk {} evidence must be a file:coordinate or 64-hex digest",
            risk.id
        );
        anyhow::ensure!(
            matches!(risk.state.as_str(), "open" | "mitigated" | "accepted"),
            "risk {} has unknown state {:?}",
            risk.id,
            risk.state
        );
    }
    Ok(())
}

/// ADR-0020 §1: the qualification lane runs from ONE fixed clean commit and
/// tree. The packet is committed one commit after that fixed evidence commit;
/// requiring the packet's metadata commit to equal HEAD would be a
/// self-referential hash claim and cannot be satisfied by Git.
fn verify_provenance(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let Some(provenance) = &packet.provenance else {
        anyhow::bail!("packet has no provenance block; qualification is unbound to any tree");
    };
    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").current_dir(root).args(args).output()?;
        anyhow::ensure!(out.status.success(), "git {args:?} failed");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    require_eq(
        "provenance.commit/fixed_commit",
        &provenance.commit,
        &provenance.fixed_commit,
    )?;
    let parent = git(&["rev-parse", "HEAD^"])?;
    let parents = git(&["rev-list", "--parents", "-n1", "HEAD"])?
        .split_whitespace()
        .count()
        .saturating_sub(1);
    anyhow::ensure!(
        parents == 1,
        "qualification metadata commit must have exactly one parent, found {parents}"
    );
    require_eq(
        "provenance.evidence_parent",
        &provenance.evidence_parent,
        &parent,
    )?;
    require_eq(
        "provenance.fixed_commit/evidence_parent",
        &provenance.fixed_commit,
        &provenance.evidence_parent,
    )?;
    let fixed_tree = git(&[
        "rev-parse",
        &format!("{}^{{tree}}", provenance.fixed_commit),
    ])?;
    require_eq("provenance.fixed_tree", &provenance.fixed_tree, &fixed_tree)?;
    let dirty = git(&["status", "--porcelain"])?;
    anyhow::ensure!(
        dirty.is_empty(),
        "qualification requires a clean tree; {} path(s) are dirty",
        dirty.lines().count()
    );
    let lockfile = fs::read(root.join("Cargo.lock"))?;
    let digest = blake3::hash(&lockfile).to_hex().to_string();
    require_eq(
        "provenance.lockfile_blake3",
        &provenance.lockfile_blake3,
        &digest,
    )?;
    Ok(())
}

/// Requirement coordinates must resolve to committed source files. The
/// coordinate suffix remains domain-specific (for example `D18.1`), but a
/// nonexistent file cannot be an auditable normative source.
fn verify_requirement_sources(root: &Utf8Path, packet: &Packet) -> Result<()> {
    for requirement in &packet.requirements {
        let (source_file, coordinate) = requirement
            .source
            .split_once(':')
            .with_context(|| format!("requirement {} source lacks coordinate", requirement.id))?;
        let lower = source_file.to_ascii_lowercase();
        anyhow::ensure!(
            !source_file.starts_with('/')
                && !source_file.split('/').any(|part| part == "..")
                && !lower.contains("heldout")
                && !lower.contains("conformance/corpora"),
            "requirement {} source path is unsafe or locked",
            requirement.id
        );
        let path = root.join(source_file);
        let metadata = fs::symlink_metadata(&path).with_context(|| {
            format!(
                "requirement {} source file is missing: {path}",
                requirement.id
            )
        })?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "requirement {} source is not a regular file: {path}",
            requirement.id
        );
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read requirement {} source: {path}", requirement.id))?;
        anyhow::ensure!(
            source.contains(coordinate),
            "requirement {} coordinate {:?} is absent from {path}",
            requirement.id,
            coordinate
        );
    }
    Ok(())
}

fn read_packet(root: &Utf8Path) -> Result<Packet> {
    let path = root.join("conformance/haqp/packet.json");
    let bytes = fs::read(&path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

fn verify_packet_shape(packet: &Packet) -> Result<()> {
    require_eq(
        "suite_version",
        &packet.suite_version,
        "phase1-haqp1-proposed-v2-seven-fuzz",
    )?;
    require_eq("status", &packet.status, "proposed")?;
    require_eq("ratification", &packet.ratification, "unratified")?;
    anyhow::ensure!(
        matches!(
            packet.concurrent_code.as_str(),
            "not_applicable" | "deterministic_schedule_exploration"
        ),
        "concurrent_code must be not_applicable or deterministic_schedule_exploration, got {:?}",
        packet.concurrent_code
    );
    if packet.locked_acceptance_corpora_touched {
        anyhow::bail!("locked_acceptance_corpora_touched must be false");
    }
    require_exact_ids(
        packet.requirements.iter().map(|row| row.id.as_str()),
        (1..=REQUIREMENT_COUNT).map(|idx| format!("P1-R{idx:03}")),
        "requirement",
    )?;
    require_unique(
        packet.requirements.iter().map(|row| row.id.as_str()),
        "requirement",
    )?;
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(packet.tests.iter().map(|row| row.id.as_str()), "test")?;
    // The inventory grows as evidence kinds are declared (M17.5 F-05): the IDs
    // must stay dense and sequential, but the COUNT is not frozen at 8 — that
    // ceiling was the mechanism that kept 36 requirements on 8 coarse tests.
    require_exact_ids(
        packet.tests.iter().map(|row| row.id.as_str()),
        (1..=packet.tests.len()).map(|idx| format!("P1-T{idx:02}")),
        "test",
    )?;
    let mut covered_requirements = BTreeSet::new();
    for test in &packet.tests {
        if test.requirements.is_empty() {
            anyhow::bail!("{} has no requirement mapping", test.id);
        }
        for requirement in &test.requirements {
            if !requirement_ids.contains(requirement.as_str()) {
                anyhow::bail!("{} references unknown requirement {}", test.id, requirement);
            }
            covered_requirements.insert(requirement.as_str());
        }
    }
    for requirement in &packet.requirements {
        // M17.5 pass-2 #12: the `critical` guard let a non-critical requirement
        // sit in the inventory forever with no test behind it, inflating the
        // requirement count without adding any evidence. Every declared
        // requirement must be mapped; criticality decides how MUCH evidence is
        // needed (see `verify_evidence_coverage`), not whether any is.
        if !covered_requirements.contains(requirement.id.as_str()) {
            anyhow::bail!(
                "requirement {} ({}) is not mapped to any test — an unmapped \
                 requirement counts toward the inventory while proving nothing",
                requirement.id,
                if requirement.critical {
                    "critical"
                } else {
                    "non-critical"
                }
            );
        }
        if requirement.kind.trim().is_empty() || requirement.source.trim().is_empty() {
            anyhow::bail!("requirement {} has empty kind/source", requirement.id);
        }
        if !REQUIREMENT_KINDS.contains(&requirement.kind.as_str()) {
            anyhow::bail!(
                "requirement {} has unknown kind {:?}; expected one of {:?}",
                requirement.id,
                requirement.kind,
                REQUIREMENT_KINDS
            );
        }
        if STATEFUL_REQUIREMENT_IDS.contains(&requirement.id.as_str()) && !requirement.stateful {
            anyhow::bail!(
                "requirement {} is stateful and must set stateful=true",
                requirement.id
            );
        }
        let (source_file, coordinate) = requirement
            .source
            .split_once(':')
            .with_context(|| format!("requirement {} source lacks coordinate", requirement.id))?;
        anyhow::ensure!(
            !source_file.trim().is_empty() && !coordinate.trim().is_empty(),
            "requirement {} source must contain file and coordinate",
            requirement.id
        );
    }
    verify_evidence_coverage(packet)?;
    verify_mutant_inventory(packet)?;
    verify_kill_concentration(packet)?;
    verify_canary_inventory(packet)?;
    verify_generated_inventory(packet)?;
    verify_reviews_inventory(packet)?;
    verify_crash_boundary_inventory(packet)?;
    Ok(())
}

/// Evidence kinds every applicable critical requirement must carry
/// (ADR-0020 §2: "positive, negative, malformed/adversarial, basis/provenance,
/// and deterministic-replay evidence").
const CORE_EVIDENCE: [&str; 5] = ["positive", "negative", "malformed", "basis", "replay"];
/// "Stateful laws also have injected-fault and idempotent-recovery evidence."
const STATEFUL_EVIDENCE: [&str; 2] = ["fault", "recovery"];
/// Requirement kinds are closed by the HAQP packet contract. An open kind
/// vocabulary lets a packet relabel a stateful requirement and evade its
/// fault/recovery evidence obligations (M17.5 P1-A04).
const REQUIREMENT_KINDS: [&str; 4] = ["law", "gate", "fault", "abuse"];
/// Current packet's stateful law is M20 D20.3. Keep the semantic coordinate
/// closed even if an attacker relabels its `kind` or drops its JSON marker.
const STATEFUL_REQUIREMENT_IDS: [&str; 1] = ["P1-R015"];
/// No single test may carry more than this share of the mutation denominator.
const MAX_KILL_SHARE: f64 = 0.25;

/// M17.5 F-05: a requirement mapped to one coarse end-to-end test is not
/// covered. Every critical requirement must carry all five core evidence
/// kinds, and fault-class requirements must additionally carry injected-fault
/// and idempotent-recovery evidence.
fn verify_evidence_coverage(packet: &Packet) -> Result<()> {
    let known = CORE_EVIDENCE
        .iter()
        .chain(STATEFUL_EVIDENCE.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut covered = BTreeMap::<&str, BTreeSet<&str>>::new();
    for test in &packet.tests {
        if test.evidence.is_empty() {
            anyhow::bail!("{} declares no evidence kind", test.id);
        }
        for kind in &test.evidence {
            if !known.contains(kind.as_str()) {
                anyhow::bail!("{} declares unknown evidence kind {kind}", test.id);
            }
        }
        for requirement in &test.requirements {
            covered
                .entry(requirement.as_str())
                .or_default()
                .extend(test.evidence.iter().map(String::as_str));
        }
    }
    for requirement in &packet.requirements {
        if !requirement.critical {
            continue;
        }
        let have = covered
            .get(requirement.id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut needed = CORE_EVIDENCE.to_vec();
        if requirement.stateful || requirement.kind == "fault" {
            needed.extend_from_slice(&STATEFUL_EVIDENCE);
        }
        let missing = needed
            .into_iter()
            .filter(|kind| !have.contains(kind))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            anyhow::bail!(
                "critical requirement {} ({}) is missing {} evidence",
                requirement.id,
                requirement.kind,
                missing.join(", ")
            );
        }
    }
    Ok(())
}

/// M17.5 F-05: ADR-0020 §3 forbids mutation coverage clustering on
/// easy-to-kill paths. A 100% kill rate carried by a handful of coarse
/// end-to-end tests measures almost nothing, so cap any one test's share of
/// the denominator and require every named killer to exist.
fn verify_kill_concentration(packet: &Packet) -> Result<()> {
    let total = packet.mutants.len();
    if total == 0 {
        return Ok(());
    }
    let test_ids = packet
        .tests
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut kills = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
        for test in &mutant.killing_tests {
            if !test_ids.contains(test.as_str()) {
                anyhow::bail!("{} names unknown killing test {test}", mutant.id);
            }
            *kills.entry(test.as_str()).or_default() += 1;
        }
    }
    for (test, count) in &kills {
        #[allow(clippy::cast_precision_loss, reason = "inventory-scale counts")]
        let share = *count as f64 / total as f64;
        // M17.5 F-30: `>` -> `>=` survives here and is EQUIVALENT within the
        // reachable domain. `share` is `count / total` over 65 declared
        // mutants, and 65/4 is not an integer, so `share == 0.25` exactly
        // cannot occur.
        if share > MAX_KILL_SHARE {
            anyhow::bail!(
                "{test} is the named killer for {count}/{total} mutants ({:.0}%), \
                 above the {:.0}% ceiling — mutation coverage is clustering on one \
                 coarse test",
                share * 100.0,
                MAX_KILL_SHARE * 100.0
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PacketStatusExpectations {
    mutant: &'static str,
    canary: &'static str,
    generated: &'static str,
    crash: &'static str,
    review: &'static str,
}

fn verify_packet_statuses(packet: &Packet, expected: PacketStatusExpectations) -> Result<()> {
    for mutant in &packet.mutants {
        if expected.mutant == "resolved" {
            anyhow::ensure!(
                mutant.disposition != "predeclared",
                "mutant {} remains predeclared in a resolved packet",
                mutant.id
            );
        } else {
            require_eq("mutant.disposition", &mutant.disposition, expected.mutant)?;
        }
    }
    for canary in &packet.canaries {
        require_eq("canary.result", &canary.result, expected.canary)?;
    }
    for family in &packet.generated {
        require_eq("generated.result", &family.result, expected.generated)?;
    }
    for row in &packet.crash_boundaries {
        require_eq("crash.result", &row.result, expected.crash)?;
    }
    for review in &packet.reviews {
        require_eq("review.result", &review.result, expected.review)?;
    }
    Ok(())
}

/// The five ADR-0020 §4 evidence families. A mutant naming anything else is not
/// describing this suite.
const MUTANT_FAMILIES: [&str; 5] = [
    "source/CST/formatting",
    "graph/interchange codecs",
    "transforms/projections",
    "repair/ILRP/recovery",
    "Basis/revision/query invalidation",
];

const GENERATED_FAMILIES: [&str; 5] = MUTANT_FAMILIES;

/// Phase 1's M18-M24 inventory is closed. A count-only check lets a doctored
/// packet remove a law and replace it with an invented row while preserving
/// the same cardinality.
const REQUIREMENT_COUNT: usize = 36;

/// The declared mutation operators. Closed by design (M17.5 pass-2 #13): an
/// open vocabulary lets a fabricated inventory invent an operator per mutant
/// and sail through the per-operator concentration ceiling, since a label used
/// once can never exceed it.
const MUTANT_OPERATORS: [&str; 13] = [
    "predicate-deletion",
    "predicate-inversion",
    "threshold-plus-one",
    "threshold-minus-one",
    "missing-enum-dispatch",
    "success-error-substitution",
    "oracle-short-circuit",
    "ordering-nondeterminism",
    "stale-basis-acceptance",
    "skipped-durable-transition",
    "disabled-crash-point",
    "broadened-allow-list",
    "wrong-holder-selection",
];

/// Dispositions a mutant row may carry. `equivalent` and `duplicate` are the
/// only ADR-0020 §3 escapes from "one survivor blocks eligibility", and both
/// demand a written proof, so neither may be spelled freehand.
const MUTANT_DISPOSITIONS: [&str; 4] = ["predeclared", "killed", "equivalent", "duplicate"];

fn verify_mutant_inventory(packet: &Packet) -> Result<()> {
    if packet.mutants.len() != 65 {
        anyhow::bail!("mutant count must be 65, got {}", packet.mutants.len());
    }
    require_unique(packet.mutants.iter().map(|row| row.id.as_str()), "mutant")?;
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut by_family = BTreeMap::<&str, usize>::new();
    let mut by_operator = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
        // M17.5 pass-2 #13: family/operator/source/disposition were free text,
        // so a fabricated inventory with invented labels was accepted whole.
        // Each must now name something this suite actually declares.
        if !MUTANT_FAMILIES.contains(&mutant.family.as_str()) {
            anyhow::bail!(
                "{} declares unknown mutation family {:?}; expected one of {:?}",
                mutant.id,
                mutant.family,
                MUTANT_FAMILIES
            );
        }
        if !MUTANT_OPERATORS.contains(&mutant.operator.as_str()) {
            anyhow::bail!(
                "{} declares unknown mutation operator {:?}; expected one of {:?}",
                mutant.id,
                mutant.operator,
                MUTANT_OPERATORS
            );
        }
        if !MUTANT_DISPOSITIONS.contains(&mutant.disposition.as_str()) {
            anyhow::bail!(
                "{} declares unknown disposition {:?}; expected one of {:?}",
                mutant.id,
                mutant.disposition,
                MUTANT_DISPOSITIONS
            );
        }
        if !requirement_ids.contains(mutant.source.as_str()) {
            anyhow::bail!(
                "{} names source {:?}, which is not a declared requirement — a \
                 mutant must attack something the inventory claims to require",
                mutant.id,
                mutant.source
            );
        }
        if mutant.defect.trim().is_empty() {
            anyhow::bail!("{} describes no defect", mutant.id);
        }
        validate_disposition(mutant)?;
        *by_family.entry(mutant.family.as_str()).or_default() += 1;
        *by_operator.entry(mutant.operator.as_str()).or_default() += 1;
        if mutant.disposition == "predeclared" && mutant.killing_tests.is_empty() {
            anyhow::bail!("{} has no killing test", mutant.id);
        }
    }
    for (family, count) in by_family {
        if count != 13 {
            anyhow::bail!("family {family} mutant count must be 13, got {count}");
        }
    }
    for (operator, count) in by_operator {
        if count > 16 {
            anyhow::bail!("operator {operator} supplies {count} mutants; max 16");
        }
    }
    Ok(())
}

fn verify_canary_inventory(packet: &Packet) -> Result<()> {
    require_exact_ids(
        packet.canaries.iter().map(|row| row.id.as_str()),
        (1..=16).map(|idx| format!("C{idx:02}")),
        "canary",
    )?;
    for canary in &packet.canaries {
        if canary.expected_failure.trim().is_empty() {
            anyhow::bail!("{} has empty expected failure", canary.id);
        }
        let expected_gate = CANARY_GATES
            .iter()
            .find_map(|(id, gate)| (*id == canary.id.as_str()).then_some(*gate))
            .with_context(|| format!("{} has no closed gate mapping", canary.id))?;
        require_eq("canary gate", &canary.gate, expected_gate)?;
    }
    Ok(())
}

/// Closed canary-to-gate map. The sixteen rows are not merely a count: each
/// ratification gate has one named, executable violation (M17.5 P1-A04).
const CANARY_GATES: [(&str, &str); 16] = [
    ("C01", "status"),
    ("C02", "ratification"),
    ("C03", "locked corpus"),
    ("C04", "requirements"),
    ("C05", "tests"),
    ("C06", "tests"),
    ("C07", "mutants"),
    ("C08", "mutants"),
    ("C09", "mutants"),
    ("C10", "canaries"),
    ("C11", "generated"),
    ("C12", "generated"),
    ("C13", "fuzz"),
    ("C14", "crash"),
    ("C15", "reviews"),
    ("C16", "markdown"),
];

fn verify_generated_inventory(packet: &Packet) -> Result<()> {
    if packet.generated.len() != 5 {
        anyhow::bail!(
            "generated family count must be 5, got {}",
            packet.generated.len()
        );
    }
    require_exact_ids(
        packet.generated.iter().map(|row| row.family.as_str()),
        GENERATED_FAMILIES.iter().map(|family| (*family).to_owned()),
        "generated family",
    )?;
    for family in &packet.generated {
        if family.accepted < 100_000 {
            anyhow::bail!("{} accepted cases below 100000", family.family);
        }
        // M17.5 F-30: `>` -> `>=` survives and is EQUIVALENT within the
        // reachable domain. Exactly 1% needs `accepted == 99 * discards`, and
        // with the >=100,000 accepted floor the nearest such point is not
        // expressible alongside the packet's own declared counts. The ceiling
        // is checked from both sides at the nearest reachable pair.
        if family.discards * 100 > family.attempts {
            anyhow::bail!("{} discard rate exceeds 1%", family.family);
        }
        if family.seed_categories.len() < 16 {
            anyhow::bail!("{} has fewer than 16 seed categories", family.family);
        }
        let mut categories = BTreeSet::new();
        for category in &family.seed_categories {
            anyhow::ensure!(
                !category.trim().is_empty(),
                "{} has an empty seed category",
                family.family
            );
            anyhow::ensure!(
                categories.insert(category),
                "{} repeats seed category {:?}",
                family.family,
                category
            );
        }
        // Internal consistency: the three counts must describe one run.
        if family.accepted + family.discards != family.attempts {
            anyhow::bail!(
                "{}: accepted {} + discards {} != attempts {} — the counts do not \
                 describe a single run",
                family.family,
                family.accepted,
                family.discards,
                family.attempts
            );
        }
        // M17.5 F-04/F-07: a `pass` must be backed by a reproducible run, not
        // by numbers typed into the packet.
        if family.result == "pass" {
            if family.seed.is_none() {
                anyhow::bail!("{} claims pass without a recorded seed", family.family);
            }
            // M17.5 F-27 / P1-A04: this checked LENGTH only, so any 64
            // characters passed — including 64 spaces. A hash that is not
            // hexadecimal cannot be the digest of anything.
            if family
                .evidence_hash
                .as_ref()
                .is_none_or(|hash| hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()))
            {
                anyhow::bail!(
                    "{} claims pass without a 64-hex evidence hash",
                    family.family
                );
            }
        }
    }
    // The fuzz budget is no longer checkable here: it is DERIVED from the
    // committed campaign by `verify_fuzz_budget` at the qualified layer. What
    // the inventory layer can still check is the mapping's shape.
    verify_fuzz_target_mapping(&packet.generated)
}

/// The family→target mapping must be a partition, not an overlap
/// (M17.5 F-17, F-29).
///
/// ADR-0020 §4 gives each family its own 30-minute campaign. If one target
/// evidenced two families, a single campaign would satisfy two budgets and both
/// would pass on half the work — so a target may be claimed at most once.
///
/// Empty is allowed HERE and refused at the qualified layer. The inventory
/// layer describes a plan, and a family whose fuzz target has not been written
/// yet is a legitimate plan; claiming `pass` for it is not.
fn verify_fuzz_target_mapping(declared: &[Generated]) -> Result<()> {
    let mut claimed: BTreeMap<&str, &str> = BTreeMap::new();
    for family in declared {
        for target in &family.fuzz_targets {
            if let Some(other) = claimed.insert(target.as_str(), family.family.as_str()) {
                anyhow::bail!(
                    "fuzz target {target:?} is claimed by both {other:?} and {:?}; \
                     one campaign cannot satisfy two families' budgets",
                    family.family
                );
            }
        }
    }
    Ok(())
}

/// Derive each family's fuzz minutes from the committed campaign and enforce
/// ADR-0020 §4's floors against the DERIVED value (M17.5 F-17).
///
/// The packet says which targets belong to each family; the artifact supplies
/// their seconds. Summing those rows is the sole family-budget calculation.
fn verify_fuzz_budget(declared: &[Generated], recorded: &[FuzzEvidence]) -> Result<()> {
    let elapsed_seconds: BTreeMap<&str, u64> = recorded
        .iter()
        .map(|row| (row.target.as_str(), row.elapsed_s))
        .collect();
    let mut total_minutes = 0_u64;
    for family in declared {
        if family.result != "pass" {
            continue;
        }
        if family.fuzz_targets.is_empty() {
            anyhow::bail!(
                "{} claims pass but names no fuzz target; ADR-0020 §4 gives every \
                 family its own sanitizer campaign, and a family with no target \
                 has had none",
                family.family
            );
        }
        let mut family_minutes = 0_u64;
        for target in &family.fuzz_targets {
            let recorded_seconds = elapsed_seconds.get(target.as_str()).with_context(|| {
                format!(
                    "{} names fuzz target {target:?}, which the committed campaign did not run",
                    family.family
                )
            })?;
            family_minutes += recorded_seconds / 60;
        }
        // ADR-0020 line 90: one 30-minute sanitizer campaign per family.
        if family_minutes < 30 {
            anyhow::bail!(
                "{}: its targets {:?} recorded {family_minutes} minutes, below the 30 \
                 ADR-0020 §4 requires per family",
                family.family,
                family.fuzz_targets
            );
        }
        total_minutes += family_minutes;
    }
    // ADR-0020 line 92: "Total fuzz budget is at least 150 target-minutes."
    if total_minutes < 150 {
        anyhow::bail!("derived fuzz budget {total_minutes} target-minutes is below 150");
    }
    Ok(())
}

fn verify_reviews_inventory(packet: &Packet) -> Result<()> {
    if packet.reviews.len() != 2 {
        anyhow::bail!(
            "review record count must be 2, got {}",
            packet.reviews.len()
        );
    }
    require_unique(
        packet.reviews.iter().map(|row| row.reviewer.as_str()),
        "reviewer",
    )?;
    for review in &packet.reviews {
        if review.attempts < 12 {
            anyhow::bail!("{} has fewer than 12 attempts", review.reviewer);
        }
        if review.unresolved_verified_findings != 0 {
            anyhow::bail!("{} has unresolved verified findings", review.reviewer);
        }
    }
    Ok(())
}

fn verify_markdown_surface(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("docs/execution/phase1-suite-review.md");
    let text = fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    verify_markdown_surface_text(&text, packet)
}

fn verify_markdown_surface_text(text: &str, packet: &Packet) -> Result<()> {
    if !text.contains("status: proposed") {
        anyhow::bail!("review packet markdown must stay proposed");
    }
    if !text.contains("ratification decision | unratified") {
        anyhow::bail!("review packet markdown must record unratified status");
    }
    let digest = blake3::hash(
        serde_json::to_vec(packet)
            .context("serialize packet for markdown digest")?
            .as_slice(),
    )
    .to_hex()
    .to_string();
    if !text.contains(&digest) {
        anyhow::bail!("review packet markdown missing packet digest {digest}");
    }
    Ok(())
}

fn verify_crash_boundary_inventory(packet: &Packet) -> Result<()> {
    let registered = liminal_jurisdiction::CrashPoint::all()
        .iter()
        .map(|point| point.name().to_owned())
        .collect::<BTreeSet<_>>();
    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    if declared != registered {
        anyhow::bail!(
            "crash-boundary inventory mismatch: declared={declared:?}, registered={registered:?}"
        );
    }
    for row in &packet.crash_boundaries {
        if !row.before || !row.after {
            anyhow::bail!(
                "{} missing before/after injection declaration",
                row.boundary
            );
        }
    }
    Ok(())
}

fn require_eq(field: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        anyhow::bail!("{field}: expected {expected:?}, got {actual:?}")
    }
}

fn require_unique<'a>(ids: impl Iterator<Item = &'a str>, label: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.to_owned()) {
            anyhow::bail!("duplicate {label} id {id}");
        }
    }
    Ok(())
}

fn require_exact_ids<'a>(
    actual: impl Iterator<Item = &'a str>,
    expected: impl Iterator<Item = String>,
    label: &str,
) -> Result<()> {
    let actual = actual.map(ToOwned::to_owned).collect::<BTreeSet<_>>();
    let expected = expected.collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        anyhow::bail!("{label} ids differ: actual={actual:?}, expected={expected:?}")
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Packet {
    suite_version: String,
    status: String,
    ratification: String,
    qualification_state: String,
    /// Which HAQP stage this packet claims (ADR-0021).
    ///
    /// `1a` is everything Phase 0 can prove; `1b` adds ADR-0020 §3's mutation
    /// requirement and runs at M24, when M18–M23 have written the tests that
    /// witness a kill. Required, not defaulted: a packet that did not say which
    /// stage it completed would let a reader assume the stronger one.
    qualification_stage: String,
    /// ADR-0020 §4 requires deterministic schedule evidence when concurrent
    /// code exists, and an explicit N/A record otherwise. This is required
    /// rather than defaulted so omission cannot masquerade as N/A.
    concurrent_code: String,
    locked_acceptance_corpora_touched: bool,
    requirements: Vec<Requirement>,
    tests: Vec<Test>,
    mutants: Vec<Mutant>,
    canaries: Vec<Canary>,
    generated: Vec<Generated>,
    crash_boundaries: Vec<CrashBoundary>,
    reviews: Vec<Review>,
    /// Residual-risk coordinates are populated at qualification time; the
    /// proposed inventory may leave this empty while no campaign has run.
    #[serde(default)]
    residual_risks: Vec<ResidualRisk>,
    /// ADR-0020 §1 fixed-base binding. Absent until the qualification lane
    /// runs from one clean tree.
    #[serde(default)]
    provenance: Option<Provenance>,
}

/// The fixed base a qualification run was taken from.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Provenance {
    /// Fixed clean source/evidence commit. The metadata packet is committed as
    /// its child, avoiding impossible self-referential commit hashing.
    commit: String,
    lockfile_blake3: String,
    /// Clean source commit the evidence ran against; must be HEAD's parent.
    fixed_commit: String,
    fixed_tree: String,
    evidence_parent: String,
}

/// One target's recorded fuzz campaign (committed artifact).
///
/// `deny_unknown_fields` is load-bearing (M17.5 pass-2 #17). The committed
/// artifact already carried `elapsed_s`, `seed` and `log`, written by
/// `scripts/haqp_fuzz_campaign.sh`, and this struct silently dropped all three:
/// the campaign recorded its own reproduction seed and the verifier threw it
/// away. Anything the campaign records must now either be checked here or fail
/// to parse.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FuzzEvidence {
    target: String,
    /// Budget the campaign was asked for (`-max_total_time`).
    seconds: u64,
    /// Wall-clock the campaign actually took. A target that exits early spent
    /// less than its budget, which is how a crash shows up in the timings.
    elapsed_s: u64,
    exit_code: i32,
    execs: u64,
    artifacts: u64,
    /// Fixed so the campaign is reproducible (ADR-0020 §1).
    seed: u64,
    /// Sanitizer the campaign was built with (ADR-0020 §4 requires a
    /// sanitizer-enabled campaign; M17.5 F-27 / P1-A09).
    ///
    /// This binds the packet to the campaign's CONFIGURATION, not to proof the
    /// binary was instrumented — a clean libFuzzer run emits no sanitizer
    /// marker to grep for, so there is nothing stronger available from the
    /// artifact. Recorded as a known limit rather than dressed up.
    sanitizer: String,
    /// Path the campaign wrote its libFuzzer log to.
    log: String,
    /// BLAKE3 of that log (M17.5 F-17).
    ///
    /// Defaulted so a pre-F-17 artifact still parses at the inventory layer;
    /// the qualified layer requires 64 hex, so an empty digest cannot reach a
    /// `complete` packet. Populating it needs a fresh campaign — the digest of
    /// a log nobody kept cannot be reconstructed.
    ///
    /// `log` names a file under `target/`, which is gitignored and therefore
    /// guaranteed absent by the time anyone verifies — so execs, timings and
    /// exit codes were unfalsifiable in the committed state. The digest makes a
    /// later-produced log checkable without committing megabytes of libFuzzer
    /// output; `--keep-logs` commits them when someone actually wants them.
    #[serde(default)]
    log_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAudit {
    schema_version: String,
    tracer: String,
    targets: Vec<CorpusAccessAuditTarget>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAuditTarget {
    target: String,
    manifest: String,
    manifest_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignClock {
    schema_version: String,
    reference_machine: String,
    runs: Vec<CampaignRun>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignRun {
    id: String,
    elapsed_s: u64,
    clean: bool,
    result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ResidualRisk {
    id: String,
    owner: String,
    severity: String,
    trigger: String,
    requirement: String,
    evidence: String,
    planned_phase: String,
    mitigation: String,
    authority: String,
    state: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Requirement {
    id: String,
    kind: String,
    source: String,
    critical: bool,
    /// Explicit stateful-law marker. The qualified layer requires fault and
    /// recovery evidence for this marker, independently of `kind` spelling.
    #[serde(default)]
    stateful: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Test {
    id: String,
    name: String,
    requirements: Vec<String>,
    /// Evidence kinds this test supplies for the requirements it maps to
    /// (ADR-0020 §2). One of `positive`, `negative`, `malformed`, `basis`,
    /// `replay`, `fault`, `recovery`.
    evidence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Mutant {
    id: String,
    family: String,
    operator: String,
    source: String,
    defect: String,
    killing_tests: Vec<String>,
    disposition: String,
    /// Exact source replacement used by the mutation runner. Predeclared
    /// inventory rows may omit this until their owning milestone supplies the
    /// implementation coordinate; every evaluated disposition must carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    patch: Option<MutantPatch>,
    /// Required proof for an equivalent or duplicate disposition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    disposition_proof: Option<String>,
    /// Reviewer names that concurred with an equivalent/duplicate proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    disposition_concurrence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
struct MutantPatch {
    file: String,
    before: String,
    after: String,
}

/// Durable result of one mutation-run invocation. Raw test output is never
/// persisted; digests and exit states are enough to audit what ran without
/// turning model/test output into a secret-bearing artifact.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct MutantEvidence {
    schema_version: String,
    source_commit: String,
    lockfile_blake3: String,
    runner: String,
    rows: Vec<MutantEvidenceRow>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct MutantEvidenceRow {
    id: String,
    disposition: String,
    status: String,
    patch: Option<MutantPatch>,
    runnable_tests: Vec<String>,
    baseline_exit_codes: Vec<i32>,
    failed_tests: Vec<String>,
    exit_codes: Vec<i32>,
    stdout_blake3: Vec<String>,
    stderr_blake3: Vec<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Canary {
    id: String,
    gate: String,
    violation: String,
    expected_failure: String,
    result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Generated {
    family: String,
    accepted: u64,
    attempts: u64,
    discards: u64,
    /// Fuzz targets that evidence THIS family (ADR-0020 §4; M17.5 F-29).
    ///
    /// Replaces a per-family `fuzz_minutes` number. Minutes are derived by
    /// summing these targets' recorded seconds, so family budget is reconciled
    /// to one committed campaign artifact rather than asserted independently.
    ///
    /// May be empty at the inventory layer, which describes a plan; the
    /// qualified layer refuses a family claiming `pass` with no target, the
    /// same way it refuses a test name that does not exist.
    #[serde(default)]
    fuzz_targets: Vec<String>,
    seed_categories: Vec<String>,
    result: String,
    /// Seed the recorded run used; required once `result` is `pass` so the
    /// campaign is reproducible (ADR-0020 §1) and its numbers cannot be
    /// asserted without a run behind them (M17.5 F-04/F-07).
    #[serde(default)]
    seed: Option<u64>,
    /// BLAKE3 of the recorded run's evidence stream; required once `pass`.
    #[serde(default)]
    evidence_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct CrashBoundary {
    boundary: String,
    before: bool,
    after: bool,
    result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Review {
    reviewer: String,
    attempts: u64,
    /// Finding ids this reviewer raised.
    #[serde(default)]
    findings: Vec<String>,
    /// The subset of `findings` a SECOND party reproduced independently.
    ///
    /// ADR-0020 §6 exists because one model asserting a defect is not evidence
    /// of a defect. Without this field a review row was four self-asserted
    /// numbers (M17.5 pass-2 #20): a reviewer could claim twelve attempts and
    /// zero unresolved findings having reproduced nothing at all.
    #[serde(default)]
    independently_reproduced: Vec<String>,
    unresolved_verified_findings: u64,
    /// Committed record of the run; required once `result` is `pass`.
    #[serde(default)]
    evidence: Option<String>,
    result: String,
}

/// A blind-review run's committed record, as `scripts/haqp_blind_review.py`
/// writes it. Typed rather than probed as a `serde_json::Value` so a field the
/// verifier depends on cannot quietly go missing.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewRecord {
    schema_version: String,
    pass: u8,
    reviewer: ReviewRecordReviewer,
    attempts: Vec<ReviewRecordAttempt>,
    #[serde(default)]
    findings: Vec<serde_json::Value>,
    independently_reproduced: Vec<String>,
    unresolved_verified_findings: u64,
    result: String,
    isolated_session_hash: String,
    sanitized_prompt_hash: String,
    fixed_base: ReviewFixedBase,
    blindness_proof: ReviewRecordBlindness,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ReviewRecordReviewer {
    model_family: String,
    identity_hash: String,
    backend: String,
}

/// Findings carry model-chosen key names, so they stay untyped; ATTEMPTS are
/// the runner's own contract and are pinned.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ReviewRecordAttempt {
    id: String,
    attack_class: String,
    target: String,
    attempt: String,
    observed_result: String,
    independently_reproduced: bool,
    classification: String,
    resolved: bool,
    #[serde(default)]
    resolution: Option<ReviewResolution>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewResolution {
    commit: String,
    coordinate: String,
    evidence_path: String,
    evidence_sha256: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ReviewFixedBase {
    commit: String,
    tree: String,
    clean: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ReviewRecordBlindness {
    ephemeral_session_requested: bool,
    session_state: String,
    prior_pass_artifact_supplied: bool,
    #[serde(default)]
    pass_two_original_spec_only: bool,
}

/// One scenario's fault-matrix outcome (M17.5 F-31 / P1-A08).
///
/// The artifact has always carried this block and the verifier has always
/// ignored it — the field was typed `Vec<serde_json::Value>` and read by
/// nothing, so a scenario that injected no faults, or reported a non-pass
/// result, satisfied every check.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashScenario {
    scenario: String,
    faults_injected: u64,
    boundaries: BTreeSet<String>,
    result: String,
}

/// One boundary's recorded fault-matrix outcome (committed artifact).
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidence {
    registered: usize,
    exercised: usize,
    /// Per-scenario rows; carried so `deny_unknown_fields` cannot be defeated
    /// by the lane emitting a field the verifier silently drops (the F-17
    /// lesson applied before it bites).
    scenarios: Vec<CrashScenario>,
    boundaries: Vec<CrashEvidenceBoundary>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidenceBoundary {
    boundary: String,
    occurrences_exercised: u64,
    recovery_pairs: Vec<RecoveryPair>,
    staged_residue: String,
    result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryPair {
    scenario: String,
    occurrence: u64,
    first_recovery_digest: String,
    second_recovery_digest: String,
}

/// Execute every declared disposable gate canary without touching the working
/// tree.
pub fn run_canaries_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_packet(root)?;
    verify_inventory_repo(root)?;
    let markdown_path = root.join("docs/execution/phase1-suite-review.md");
    let markdown = fs::read_to_string(&markdown_path)?;
    let records = run_canary_suite(&baseline, &markdown)?;
    let bytes = serde_json::to_vec_pretty(&records)?;
    let path = root.join("conformance/haqp/evidence/canaries.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, &bytes)?;
    println!(
        "all {} canaries caught; evidence {path}; blake3={}",
        records.len(),
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
}

/// Run one canary per DECLARED row, and require each row's prose to describe
/// the mutation that was actually performed (M17.5 pass-2 #22).
///
/// The runner used to iterate a hardcoded `C01..=C16` and read only
/// `expected_failure`. `Canary.gate` and `Canary.violation` were therefore
/// decorative: the packet could say C09 relabels a mutation family while the
/// executed arm did something entirely different, and nothing would notice.
/// That matters because the canary table is the artifact a reader trusts to
/// learn WHAT the sixteen canaries prove — unverified prose in that position is
/// worse than no prose, because it reads as evidence.
///
/// Driving the loop from `packet.canaries` rather than a hardcoded `1..=16`
/// closes a drift hazard rather than a live hole: `verify_canary_inventory`
/// currently pins the id set to exactly C01–C16, so an extra row is rejected
/// before it reaches this function. But that rule is the only thing keeping the
/// hardcoded range honest, and the natural future edit — relaxing it to a
/// minimum when canary 17 is written — would have left the new canaries
/// declared, counted, and never executed. The loop now cannot disagree with the
/// table it reports on.
fn run_canary_suite(baseline: &Packet, markdown: &str) -> Result<Vec<CanaryEvidence>> {
    let baseline_digest = packet_digest(baseline)?;
    let mut records = Vec::new();
    for row in &baseline.canaries {
        let mut packet = baseline.clone();
        let mut altered_markdown = markdown.to_owned();
        let performed = mutate_canary(
            &row.id,
            &mut packet,
            &mut altered_markdown,
            &baseline_digest,
        )?;
        require_eq(
            &format!("{}.violation (packet prose vs executed mutation)", row.id),
            &row.violation,
            performed,
        )?;
        let observed = verify_packet_shape(&packet)
            .and_then(|()| verify_packet_statuses(&packet, inventory_statuses()))
            .and_then(|()| verify_markdown_surface_text(&altered_markdown, &packet))
            .expect_err("mutated canary must fail closed")
            .to_string();
        if !canary_failure_matches(&row.expected_failure, &observed) {
            anyhow::bail!(
                "{}: expected failure {:?}, observed {observed:?}",
                row.id,
                row.expected_failure
            );
        }
        records.push(CanaryEvidence {
            id: row.id.clone(),
            gate: row.gate.clone(),
            violation: row.violation.clone(),
            expected_failure: row.expected_failure.clone(),
            observed_failure: observed,
            caught: true,
            mutation_semantics: canary_mutation_semantics(&row.id)?.to_owned(),
        });
    }
    Ok(records)
}

/// Match expected canary coordinates at the beginning of the verifier error.
/// A substring search lets unrelated detail later in an error satisfy a row;
/// exact equality or a delimited suffix preserves diagnostic detail without
/// allowing that false positive.
fn canary_failure_matches(expected: &str, observed: &str) -> bool {
    let expected = expected.trim().replace('"', "");
    let observed = observed.replace('"', "");
    observed == expected
        || observed.starts_with(&format!("{expected}:"))
        || observed.starts_with(&format!("{expected},"))
        || observed.starts_with(&format!("{expected} "))
}

fn inventory_statuses() -> PacketStatusExpectations {
    PacketStatusExpectations {
        mutant: "predeclared",
        canary: "predeclared",
        generated: "planned",
        crash: "registered",
        review: "planned",
    }
}

/// Apply one canary's mutation and RETURN the violation it performed, so the
/// caller can hold the packet's prose to it (M17.5 pass-2 #22). The returned
/// string is the authority: if the packet disagrees, the packet is wrong.
fn mutate_canary(
    id: &str,
    packet: &mut Packet,
    markdown: &mut String,
    baseline_digest: &str,
) -> Result<&'static str> {
    let performed = match id {
        "C01" => {
            packet.status.clear();
            packet.status.push_str("ratified");
            "set status to ratified"
        }
        "C02" => {
            packet.ratification.clear();
            packet.ratification.push_str("approved");
            "set ratification to approved"
        }
        "C03" => {
            packet.locked_acceptance_corpora_touched = true;
            "set locked_acceptance_corpora_touched true"
        }
        "C04" => {
            packet.requirements.push(packet.requirements[0].clone());
            "duplicate requirement id"
        }
        "C05" => {
            packet.tests.retain(|row| row.id != "P1-T08");
            "remove P1-T08"
        }
        "C06" => {
            packet.tests[0].requirements.clear();
            "empty requirement mapping"
        }
        "C07" => {
            packet.mutants.pop();
            "remove one mutant"
        }
        "C08" => {
            for mutant in packet.mutants.iter_mut().take(17) {
                mutant.operator.clear();
                mutant.operator.push_str("predicate-deletion");
            }
            "operator supplies 17 mutants"
        }
        "C09" => {
            // Moving one mutant out of its family leaves that family at 12,
            // which is what `verify_mutant_inventory`'s exactly-13 rule catches.
            let family = packet.mutants[0].family.clone();
            packet.mutants[0].family = if family == "source/CST/formatting" {
                "graph/interchange codecs".to_owned()
            } else {
                "source/CST/formatting".to_owned()
            };
            "family supplies 12 mutants"
        }
        "C10" => {
            packet.canaries.retain(|row| row.id != "C16");
            "remove C16"
        }
        "C11" => {
            packet.generated[0].accepted = 0;
            "accepted below 100000"
        }
        "C12" => {
            packet.generated[0].discards = packet.generated[0].attempts;
            "discard rate above 1 percent"
        }
        "C13" => {
            // Was "fuzz minutes below 30". F-17 removed the per-family minutes
            // (they were a second, independently-assertable claim about one
            // campaign); the budget is now derived from the artifact, so the
            // packet-level violation this canary can perform is an overlapping
            // mapping — one campaign satisfying two families' budgets.
            let stolen = packet.generated[1]
                .fuzz_targets
                .first()
                .cloned()
                .unwrap_or_else(|| "cst_parse".to_owned());
            packet.generated[0].fuzz_targets.push(stolen);
            "claim another family's fuzz target"
        }
        "C14" => {
            packet
                .crash_boundaries
                .retain(|row| row.boundary != "ilrp/before_ack");
            "drop before_ack boundary"
        }
        "C15" => {
            packet.reviews[0].attempts = 0;
            "review attempts below 12"
        }
        "C16" => {
            // The only arm that attacks the markdown surface rather than the
            // packet, so it must NOT re-stamp the digest it just removed.
            *markdown = markdown.replace(baseline_digest, "");
            return Ok("remove packet digest");
        }
        _ => anyhow::bail!(
            "unknown canary {id}: the packet declares a canary with no implemented \
             mutation, so it would otherwise be counted as caught without running"
        ),
    };
    let digest = packet_digest(packet)?;
    *markdown = markdown.replace(baseline_digest, &digest);
    Ok(performed)
}

fn packet_digest(packet: &Packet) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(packet)?)
        .to_hex()
        .to_string())
}

fn generated_artifact_digest(rows: &[GeneratedEvidence]) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(rows)?)
        .to_hex()
        .to_string())
}

fn canary_mutation_semantics(id: &str) -> Result<&'static str> {
    match id {
        "C01" => Ok("packet.status := ratified"),
        "C02" => Ok("packet.ratification := approved"),
        "C03" => Ok("packet.locked_acceptance_corpora_touched := true"),
        "C04" => Ok("duplicate requirements[0]"),
        "C05" => Ok("remove test P1-T08"),
        "C06" => Ok("clear tests[0].requirements"),
        "C07" => Ok("remove final mutant"),
        "C08" => Ok("set first 17 mutant operators to predicate-deletion"),
        "C09" => Ok("move first mutant to another family"),
        "C10" => Ok("remove canary C16"),
        "C11" => Ok("set generated[0].accepted := 0"),
        "C12" => Ok("set generated[0].discards := attempts"),
        "C13" => Ok("duplicate one fuzz target across families"),
        "C14" => Ok("remove crash boundary ilrp/before_ack"),
        "C15" => Ok("set reviews[0].attempts := 0"),
        "C16" => Ok("remove baseline packet digest from markdown"),
        _ => anyhow::bail!("unknown canary {id}"),
    }
}

/// Run deterministic generated checks for all five HAQP families. This records
/// generated-case evidence only; sanitizer wall-clock qualification remains a
/// separate `cargo fuzz` lane and is never inferred from this command.
/// Deterministic 64-bit stream; one seed per case, recorded per family.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 ^ (self.0 >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    /// A deterministic identifier drawn from the seeded stream.
    ///
    /// M17.5 F-16: the generators minted ids with `NodeId::new()`,
    /// `RepairStepId::new()` and friends, all of which are `Uuid::now_v7()` —
    /// wall-clock milliseconds plus OS randomness. So the `seed` recorded in the
    /// packet, whose stated purpose is to make the campaign reproducible under
    /// ADR-0020 §1, did not reproduce a run: a crash at case 45,231 could not be
    /// replayed from it. The defect stayed invisible because the digest absorbed
    /// only RNG state, so nothing the ids touched ever reached the artifact.
    ///
    /// v5 (namespaced SHA-1 of the seeded bytes) rather than v7, because a
    /// deterministic v7 would have to invent a timestamp. The ids are opaque to
    /// every assertion here; nothing in these families depends on UUID version
    /// or on time ordering. That is a mild strengthening: v7 ids arrive in
    /// ascending creation order, so `topo_order` was only ever exercised on
    /// plans whose id order agreed with insertion order.
    fn uuid(&mut self) -> uuid::Uuid {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&self.next().to_le_bytes());
        bytes[8..].copy_from_slice(&self.next().to_le_bytes());
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &bytes)
    }
    fn word(&mut self) -> String {
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789-_ ";
        let len = 1 + self.below(12);
        (0..len)
            .map(|_| {
                let idx = usize::try_from(self.below(ALPHABET.len() as u64)).expect("index fits");
                char::from(ALPHABET[idx])
            })
            .collect()
    }
}

/// Outcome of one generated case: the generator decides acceptance from the
/// candidate itself, so `accepted`/`discards` are MEASURED, never assumed.
enum Case {
    /// In-domain and every metamorphic relation held. Carries a WITNESS of what
    /// the code under test actually produced for this input.
    ///
    /// The witness exists because of M17.5 pass-2 #16. The evidence digest used
    /// to absorb only the generator's RNG state, which made it a fingerprint of
    /// the GENERATOR and provably independent of the code being tested: delete
    /// every `ensure!` in every case function below and, so long as the number
    /// of RNG draws is unchanged, the recorded `evidence_hash` is bit-identical.
    /// Two different formatters would hash the same. A hash that cannot
    /// distinguish the implementation from a no-op is not evidence about the
    /// implementation.
    ///
    /// So each family now returns bytes derived from the output it observed, and
    /// the runner folds those into the digest. An empty witness is refused.
    Accepted(Vec<u8>),
    /// Out of the declared domain; not evidence either way.
    /// Out of domain, or a relation that legitimately does not apply. Carries
    /// a WITNESS of WHY, for the same reason `Accepted` does (M17.5 F-30).
    ///
    /// The runner used to record a discard as the single byte `b"D"`, so the
    /// digest absorbed the COUNT of discards and nothing about them. Six
    /// mutants inside `case_repair`'s cycle construction survived on exactly
    /// that: each kept the plan cyclic, so each still discarded, and the
    /// evidence could not tell the shapes apart. ADR-0020 §4 says attempts and
    /// discards are RECORDED; counting is not recording.
    Discarded(Vec<u8>),
}

/// Family 0 — source/CST/formatting. Metamorphic relations: **lossless
/// emit** (the CST must reproduce its input byte-for-byte) and **format
/// idempotence**, both compared over bytes rather than parsed values.
fn case_source_cst(rng: &mut Rng) -> Result<Case> {
    let source = rng.word();
    // Out of domain: the compact surface has no all-whitespace document.
    if source.trim().is_empty() {
        return Ok(Case::Discarded(
            format!("whitespace-source:{source:?}").into_bytes(),
        ));
    }
    let basis = liminal_source::SourceBasis {
        source: liminal_id::SourceId::from_name("haqp-generated"),
        content_hash: liminal_id::ContentHash::of(source.as_bytes()),
    };
    let view = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes())
        .context("generated source must load")?;
    let cst = liminal_cst::parse(&view);
    anyhow::ensure!(
        cst.emit_lossless() == source,
        "CST emit is not lossless for {source:?}"
    );
    let fmt = liminal_format::MarkdownFormatter::default();
    let once = fmt
        .format(&source)
        .map_err(|e| anyhow::anyhow!("formatter rejected {source:?}: {e}"))?;
    let twice = fmt
        .format(&once)
        .map_err(|e| anyhow::anyhow!("reformat failed: {e}"))?;
    anyhow::ensure!(once == twice, "formatting is not idempotent");
    // Witness: what the CST emitted and what the formatter produced. A changed
    // formatter changes the digest.
    let mut witness = cst.emit_lossless().into_bytes();
    witness.extend_from_slice(once.as_bytes());
    Ok(Case::Accepted(witness))
}

/// Family 1 — graph/interchange codecs.
///
/// Metamorphic relation: **byte-canonical stability**. Re-serializing a value
/// decoded from canonical bytes must reproduce those bytes exactly. The
/// comparison is over BYTES, never the type's own `PartialEq`, so a broken
/// `Eq` cannot make this pass (ADR-0020 §5).
fn case_interchange(rng: &mut Rng) -> Result<Case> {
    use liminal_graph::{Node, NodeFlags, PayloadRef};

    let text = rng.word();
    let durable = rng.below(2) == 1;
    // Domain rule: a node claiming a durable id must carry payload text.
    // Candidates violating it are out of domain and discarded, not "fixed".
    if durable && text.trim().is_empty() {
        return Ok(Case::Discarded(
            format!("durable-without-payload:{text:?}").into_bytes(),
        ));
    }
    let node = Node {
        id: liminal_id::NodeId::from_uuid(rng.uuid()),
        kind: liminal_graph::KindId(u32::try_from(rng.below(8)).expect("kind fits")),
        payload: if rng.below(4) == 0 {
            PayloadRef::None
        } else {
            PayloadRef::Text(text)
        },
        revision: liminal_id::RevisionId(rng.below(1_000)),
        flags: if durable {
            NodeFlags::HAS_DURABLE_ID
        } else {
            NodeFlags::default()
        },
    };

    let once = serde_json::to_vec(&node)?;
    let decoded: Node = serde_json::from_slice(&once)?;
    let twice = serde_json::to_vec(&decoded)?;
    anyhow::ensure!(
        once == twice,
        "interchange codec is not byte-canonical for {node:?}"
    );

    // M17.5 pass-2 #10: `once == twice` alone is satisfied by a codec that
    // ignores its input entirely — encode anything to one fixed valid document
    // and the round trip is stable forever. Stability is not fidelity. So check
    // that the DECODED value carries the fields we actually put in, field by
    // field rather than through `Node`'s own `PartialEq` (ADR-0020 §5 forbids
    // proving a relation with the implementation's equality on both sides).
    anyhow::ensure!(decoded.id == node.id, "codec lost the node id");
    anyhow::ensure!(decoded.kind == node.kind, "codec lost the node kind");
    anyhow::ensure!(decoded.revision == node.revision, "codec lost the revision");
    anyhow::ensure!(decoded.flags == node.flags, "codec lost the node flags");
    anyhow::ensure!(
        decoded.payload == node.payload,
        "codec lost the payload: {:?} -> {:?}",
        node.payload,
        decoded.payload
    );

    // Witness: the canonical encoding itself.
    Ok(Case::Accepted(once))
}

/// Family 2 — transforms/projections.
///
/// Metamorphic relations, both independent of the merge's own equality:
/// **identity** (a side that changed nothing must not perturb the other
/// side's content) and **outcome-class symmetry** (swapping `ours`/`theirs`
/// cannot change whether the merge was structurally disjoint).
fn case_transform(rng: &mut Rng) -> Result<Case> {
    use liminal_source::merge::{MergeOutcome, three_way};

    let blocks = 1 + rng.below(4);
    let base = (0..blocks)
        .map(|i| format!("{} {{#b{i}}}", rng.word()))
        .collect::<Vec<_>>()
        .join("\n\n");
    // Out of domain: a degenerate base has no slots to align.
    if base.trim().is_empty() {
        return Ok(Case::Discarded(
            format!("degenerate-base:{base:?}").into_bytes(),
        ));
    }
    let mutate = |rng: &mut Rng, text: &str| -> String {
        text.lines()
            .map(|line| {
                if rng.below(3) == 0 {
                    format!("{} {line}", rng.word())
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let ours = mutate(rng, &base);
    let theirs = mutate(rng, &base);

    // Identity: theirs unchanged => the merge must carry ours' content.
    //
    // M17.5 pass-2 #9: this used to be `if let Disjoint { merged } = ...`, so an
    // implementation that returned `Conflict` for EVERY input never entered the
    // branch and the relation held vacuously — the metamorphic check tested
    // nothing at all. A side that changed nothing cannot conflict with anything,
    // so `Disjoint` is a REQUIREMENT here, not a case to handle.
    let identity = three_way(&base, &ours, &base);
    let MergeOutcome::Disjoint { merged } = &identity else {
        anyhow::bail!(
            "merging ours against an UNCHANGED theirs reported {identity:?}; a side that \
             changed nothing cannot conflict"
        );
    };
    anyhow::ensure!(
        merged.split_whitespace().eq(ours.split_whitespace()),
        "identity merge dropped content: {ours:?} -> {merged:?}"
    );

    let forward = three_way(&base, &ours, &theirs);
    let swapped = three_way(&base, &theirs, &ours);
    let disjoint = |o: &MergeOutcome| matches!(o, MergeOutcome::Disjoint { .. });
    anyhow::ensure!(
        disjoint(&forward) == disjoint(&swapped),
        "merge disjointness is not symmetric under swapping sides"
    );
    // Witness: all three merge results, so a changed merge changes the digest.
    // `Debug` is used deliberately — it distinguishes the outcome VARIANT as
    // well as the merged text, and the identity of the variant is exactly what
    // the symmetry relation above is about.
    let witness = format!("{identity:?}|{forward:?}|{swapped:?}").into_bytes();
    Ok(Case::Accepted(witness))
}

/// Family 3 — repair/ILRP/recovery.
///
/// Builds a real `RepairPlan` DAG and orders it. Metamorphic relation:
/// **deterministic permutation** — shuffling the dependency list must not
/// change the resulting order. The ordering is then verified against the
/// generator's OWN edge set, not by asking the implementation again. Cyclic
/// candidates are out of domain and are discarded, which is where this
/// family's discard rate genuinely comes from.
fn case_repair(rng: &mut Rng) -> Result<Case> {
    use liminal_jurisdiction::repair::{
        ProposedMutation, RepairDependency, RepairOperation, RepairPlan, StatePredicate, topo_order,
    };

    let count = usize::try_from(2 + rng.below(5)).expect("step count fits");
    let ids: Vec<liminal_id::RepairStepId> = (0..count)
        .map(|_| liminal_id::RepairStepId::from_uuid(rng.uuid()))
        .collect();
    let steps = ids
        .iter()
        .map(|id| {
            (
                *id,
                ProposedMutation {
                    id: *id,
                    subject: liminal_id::JurisdictionSubject::Node(liminal_id::NodeId::from_uuid(
                        rng.uuid(),
                    )),
                    operation: RepairOperation::WriteFile {
                        path: liminal_id::PathId("generated.md".into()),
                        contents: rng.word().into_bytes(),
                    },
                    expected_prestate: StatePredicate::Any,
                    expected_poststate: StatePredicate::Any,
                    idempotency_key: liminal_id::IdempotencyKey::from_uuid(rng.uuid()),
                },
            )
        })
        .collect();

    let mut edges = Vec::new();
    for i in 0..count {
        for j in (i + 1)..count {
            if rng.below(3) == 0 {
                edges.push((i, j));
            }
        }
    }
    // Rarely close a genuine cycle. A lone back edge is NOT a cycle unless a
    // forward path exists, so add both directions to guarantee one.
    let cyclic = count >= 2 && rng.below(384) == 0;
    if cyclic {
        edges.retain(|(before, after)| !(*before == 0 && *after == count - 1));
        edges.push((0, count - 1));
        edges.push((count - 1, 0));
    }
    let dependencies: Vec<RepairDependency> = edges
        .iter()
        .map(|(before, after)| RepairDependency {
            before: ids[*before],
            after: ids[*after],
        })
        .collect();

    let plan = RepairPlan {
        id: liminal_id::RepairId::from_uuid(rng.uuid()),
        basis: liminal_revision::WorkspaceBasis {
            transaction: liminal_id::TransactionId::from_uuid(rng.uuid()),
            perspective: liminal_revision::BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps,
        dependencies: dependencies.clone(),
        inverse: None,
    };
    let Ok(order) = topo_order(&plan) else {
        // Cyclic plans are correctly refused; not acceptance evidence. The
        // witness carries the EDGES, so the shape of the refused cycle reaches
        // the digest — without it, every cycle looks alike and the
        // construction that built it is unobservable (M17.5 F-30).
        return Ok(Case::Discarded(format!("cyclic:{edges:?}").into_bytes()));
    };
    anyhow::ensure!(
        !cyclic,
        "a cyclic plan was ordered instead of refused: {dependencies:?}"
    );

    // Independent verification against the generator's own edges.
    let position: BTreeMap<liminal_id::RepairStepId, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    anyhow::ensure!(order.len() == count, "ordering dropped or duplicated steps");
    for (before, after) in &edges {
        anyhow::ensure!(
            position[&ids[*before]] < position[&ids[*after]],
            "ordering violated a declared dependency"
        );
    }

    // Deterministic permutation: reversing the edge list must not change it.
    let mut permuted = plan.clone();
    permuted.dependencies.reverse();
    anyhow::ensure!(
        topo_order(&permuted).map_err(|e| anyhow::anyhow!("{e}"))? == order,
        "topological order changed under permutation of the dependency list"
    );
    // Witness: the order the implementation chose. A different ordering
    // algorithm changes the digest even when both orders are valid.
    let witness = order
        .iter()
        .flat_map(|id| id.as_uuid().into_bytes())
        .collect::<Vec<u8>>();
    Ok(Case::Accepted(witness))
}

/// Family 4 — Basis/revision/query invalidation.
///
/// Metamorphic relations: **irrelevant-input invariance** (a key never read
/// must never invalidate) and **monotonicity** (recording more reads can only
/// add invalidations). Both are checked against a key set the generator built
/// itself, so `ComponentDeps` is never its own oracle.
fn case_invalidation(rng: &mut Rng) -> Result<Case> {
    use liminal_revision::ComponentDeps;

    // Target the declared domain (computations that read >=1 component) and
    // keep a rare out-of-domain probe so the discard path stays exercised.
    let read_count = if rng.below(384) == 0 {
        0
    } else {
        1 + rng.below(5)
    };
    let mut deps = ComponentDeps::default();
    let mut expected = BTreeSet::new();
    for _ in 0..read_count {
        let key = liminal_id::JurisdictionKey::Path(liminal_id::PathId(rng.word().into()));
        deps.record(key.clone());
        expected.insert(key);
    }
    // Out of domain: nothing was read, so invalidation is vacuous.
    if expected.is_empty() {
        return Ok(Case::Discarded(b"vacuous-invalidation".to_vec()));
    }

    for key in &expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "a recorded read did not invalidate: {key:?}"
        );
    }
    // Select a negative witness outside the generated read set. A random
    // `unread/<word>` can collide with a legitimate read key, making the only
    // negative assertion disappear exactly when the generator hits that case.
    let unrelated = (0..u32::MAX)
        .map(|index| {
            liminal_id::JurisdictionKey::Path(liminal_id::PathId(
                format!("unread/__haqp_negative_{index}").into(),
            ))
        })
        .find(|candidate| !expected.contains(candidate))
        .expect("finite generated read set leaves a negative witness");
    anyhow::ensure!(
        !deps.invalidated_by(&unrelated),
        "an unread key invalidated the computation: {unrelated:?}"
    );
    // Monotonicity: adding a read never un-invalidates an existing one.
    let extra = liminal_id::JurisdictionKey::Path(liminal_id::PathId(rng.word().into()));
    deps.record(extra.clone());
    for key in &expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "recording another read un-invalidated {key:?}"
        );
    }
    // Witness: the implementation's answer for every key probed, including the
    // two negative probes. A `invalidated_by` that always returned `true` would
    // still satisfy the assertions above for recorded keys, but it changes these
    // bytes.
    let mut witness = Vec::new();
    for key in expected.iter().chain([&unrelated, &extra]) {
        witness.push(u8::from(deps.invalidated_by(key)));
    }
    Ok(Case::Accepted(witness))
}

/// One generated-evidence family: its name and its case runner.
type Family = (&'static str, fn(&mut Rng) -> Result<Case>);

/// Run the five HAQP generated-evidence families with MEASURED acceptance.
///
/// Each family generates structured candidates, decides acceptance from the
/// candidate itself, and checks at least one metamorphic relation that does
/// not route through the implementation's own equality path. Counts are
/// tallied from outcomes — never echoed from the requested case count
/// (M17.5 finding F-07).
pub fn run_generated_repo(root: &Utf8Path, cases: u64) -> Result<()> {
    let (evidence, timings) = generate_evidence(cases)?;
    let artifact = GeneratedEvidenceArtifact {
        schema_version: "haqp-generated-v1".to_owned(),
        artifact_blake3: generated_artifact_digest(&evidence)?,
        rows: evidence.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&artifact)?;
    let path = root.join("conformance/haqp/evidence/generated.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, &bytes)?;
    for (row, elapsed_ms) in evidence.iter().zip(&timings) {
        #[allow(clippy::cast_precision_loss, reason = "reporting only")]
        let rate = row.discards as f64 * 100.0 / row.attempts.max(1) as f64;
        println!(
            "{}: {} accepted / {} attempts ({} discarded, {rate:.2}%) in {elapsed_ms}ms",
            row.family, row.accepted, row.attempts, row.discards
        );
    }
    println!(
        "generated evidence written to {path}; blake3={}",
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
}

/// Run the five families and return their evidence rows plus per-family
/// wall-clock timings.
///
/// The timings are returned SEPARATELY rather than embedded in the rows on
/// purpose: the rows are the hashable artifact and must be a pure function of
/// the seed and the code under test, while the timings are telemetry for the
/// operator (M17.5 pass-2 #18).
///
/// The case-count guard is a separate function so its BOUNDARIES are testable:
/// `>` -> `>=` at the safety limit can only be discriminated by a case that
/// runs at exactly 10,000,000, which no test can afford. Extracting the
/// predicate makes the boundary checkable in microseconds (M17.5 F-30).
/// Reject a case count that is zero or beyond the safety limit.
fn validate_case_count(cases: u64) -> Result<()> {
    if cases == 0 {
        anyhow::bail!("generated case count must be positive");
    }
    if cases > 10_000_000 {
        anyhow::bail!("generated case count exceeds safety limit: {cases} > 10000000");
    }
    Ok(())
}

fn generate_evidence(cases: u64) -> Result<(Vec<GeneratedEvidence>, Vec<u64>)> {
    validate_case_count(cases)?;
    let families: [Family; 5] = [
        ("source/CST/formatting", case_source_cst),
        ("graph/interchange codecs", case_interchange),
        ("transforms/projections", case_transform),
        ("repair/ILRP/recovery", case_repair),
        ("Basis/revision/query invalidation", case_invalidation),
    ];

    let mut evidence = Vec::new();
    let mut timings = Vec::new();
    for (index, (family, run)) in families.into_iter().enumerate() {
        let seed = 0x9E37_79B9_7F4A_7C15_u64 ^ (index as u64).wrapping_mul(0x0100_0000_01B3);
        let mut rng = Rng(seed);
        let started = Instant::now();
        let mut digest = blake3::Hasher::new();
        digest.update(&seed.to_le_bytes());
        // ADR-0020 §4 requires >=100,000 ACCEPTED cases, so run until the
        // acceptance target is met rather than stopping at N attempts. The
        // attempt cap keeps a badly-targeted generator from running forever
        // instead of silently reporting a short campaign.
        let (mut accepted, mut discards, mut attempts) = (0u64, 0u64, 0u64);
        let attempt_cap = cases.saturating_mul(2);
        while accepted < cases {
            anyhow::ensure!(
                attempts < attempt_cap,
                "{family}: {attempts} attempts yielded only {accepted}/{cases} accepted \
                 — the generator is not targeting its declared domain"
            );
            attempts += 1;
            match run(&mut rng).with_context(|| format!("{family} attempt {attempts}"))? {
                Case::Accepted(witness) => {
                    // A family that witnesses nothing would restore exactly the
                    // #16 defect: a digest independent of the code under test.
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: accepted attempt {attempts} with an empty witness, so \
                         the evidence hash would not depend on what the code produced"
                    );
                    accepted += 1;
                    digest.update(b"A");
                    digest.update(&witness);
                }
                Case::Discarded(witness) => {
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: discarded attempt {attempts} with an empty witness, so \
                         the evidence records that something was discarded and not what"
                    );
                    discards += 1;
                    digest.update(b"D");
                    digest.update(&witness);
                }
            }
            // The RNG state binds the INPUT; the witness above binds the OUTPUT.
            digest.update(&rng.0.to_le_bytes());
        }
        anyhow::ensure!(
            accepted + discards == attempts,
            "{family}: accounting lost cases"
        );
        evidence.push(GeneratedEvidence {
            family: family.to_owned(),
            accepted,
            attempts,
            discards,
            seed,
            evidence_hash: digest.finalize().to_hex().to_string(),
        });
        timings.push(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    }
    Ok((evidence, timings))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanaryEvidence {
    id: String,
    gate: String,
    /// The mutation that was actually performed, checked against the packet's
    /// own prose (M17.5 pass-2 #22).
    violation: String,
    expected_failure: String,
    observed_failure: String,
    caught: bool,
    mutation_semantics: String,
}

/// One family's generated-evidence row.
///
/// Every field here must be a function of the seed and the code under test and
/// of NOTHING ELSE. Wall-clock timing used to live here as `elapsed_ms`, which
/// made the artifact unhashable — two runs of the same seed differed (50→57,
/// 94→110, 12→15 ms), so the artifact could never be compared against a
/// recorded digest (M17.5 pass-2 #18). Timing is operational telemetry, not
/// evidence; it is printed to stdout instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedEvidence {
    family: String,
    accepted: u64,
    attempts: u64,
    discards: u64,
    /// Recorded so the run is reproducible (ADR-0020 §1).
    seed: u64,
    evidence_hash: String,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedEvidenceArtifact {
    schema_version: String,
    artifact_blake3: String,
    rows: Vec<GeneratedEvidence>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> Utf8PathBuf {
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn packet_from_repo() -> Packet {
        read_packet(&repo_root()).expect("read HAQP packet")
    }

    fn mark_complete(packet: &mut Packet) {
        packet.qualification_state = "complete".to_owned();
        for mutant in &mut packet.mutants {
            mutant.disposition = "killed".to_owned();
            mutant.patch = Some(MutantPatch {
                file: "Cargo.toml".to_owned(),
                before: "[workspace]".to_owned(),
                after: "[workspace]".to_owned(),
            });
        }
        for canary in &mut packet.canaries {
            canary.result = "caught".to_owned();
        }
        for family in &mut packet.generated {
            family.result = "pass".to_owned();
        }
        for row in &mut packet.crash_boundaries {
            row.result = "pass".to_owned();
        }
        for review in &mut packet.reviews {
            review.result = "pass".to_owned();
        }
    }

    #[test]
    fn completed_packet_statuses_can_pass_qualified_layer() {
        let mut packet = packet_from_repo();
        mark_complete(&mut packet);

        verify_packet_shape(&packet).expect("complete packet shape stays valid");
        verify_packet_statuses(
            &packet,
            PacketStatusExpectations {
                mutant: "killed",
                canary: "caught",
                generated: "pass",
                crash: "pass",
                review: "pass",
            },
        )
        .expect("complete statuses accepted by qualified verifier");
    }

    #[test]
    fn mutant_patch_requires_one_relative_source_coordinate() {
        let good = MutantPatch {
            file: "crates/example.rs".to_owned(),
            before: "old".to_owned(),
            after: "new".to_owned(),
        };
        validate_mutant_patch(&good).expect("ordinary relative patch is valid");
        for file in ["/tmp/example.rs", "../example.rs"] {
            let bad = MutantPatch {
                file: file.to_owned(),
                ..good.clone()
            };
            validate_mutant_patch(&bad).expect_err("patch must not escape worktree");
        }
    }

    #[test]
    fn evaluated_equivalent_mutant_requires_proof_and_two_concurrences() {
        let mut packet = packet_from_repo();
        packet.mutants[0].disposition = "equivalent".to_owned();
        packet.mutants[0].patch = Some(MutantPatch {
            file: "Cargo.toml".to_owned(),
            before: "[workspace]".to_owned(),
            after: "[workspace]".to_owned(),
        });
        verify_mutant_inventory(&packet).expect_err("equivalence without proof must fail");
        packet.mutants[0].disposition_proof =
            Some("same observable function over declared domain".to_owned());
        packet.mutants[0].disposition_concurrence = vec!["pass-1".to_owned()];
        verify_mutant_inventory(&packet).expect_err("equivalence needs both pass concurrences");
        packet.mutants[0]
            .disposition_concurrence
            .push("pass-2".to_owned());
        verify_mutant_inventory(&packet).expect("fully documented equivalence is admissible");
    }

    #[test]
    fn mutant_patch_replacement_rejects_zero_or_multiple_matches() {
        let scratch = liminal_scratch::ScratchDir::new("haq-mutant-patch").expect("scratch");
        let fixture_path = scratch.join("target.txt");
        fs::write(&fixture_path, "one old two old").expect("write fixture");
        let replacement = MutantPatch {
            file: "target.txt".to_owned(),
            before: "old".to_owned(),
            after: "new".to_owned(),
        };
        apply_mutant_patch(&scratch, &replacement).expect_err("ambiguous replacement must fail");
        fs::write(&fixture_path, "one old").expect("rewrite fixture");
        apply_mutant_patch(&scratch, &replacement).expect("unique replacement succeeds");
        assert_eq!(
            fs::read_to_string(fixture_path).expect("read fixture"),
            "one new"
        );
    }

    /// M17.5 F-05 canary: dropping one evidence kind must fail the shape
    /// check. Without this, "36 requirements covered" can mean 36 requirements
    /// on one coarse end-to-end test.
    #[test]
    fn shape_check_rejects_a_requirement_missing_an_evidence_kind() {
        let mut packet = packet_from_repo();
        // Strip every `negative` test; the requirements they served lose a kind.
        packet
            .tests
            .retain(|test| test.evidence != vec!["negative".to_owned()]);
        for (index, test) in packet.tests.iter_mut().enumerate() {
            test.id = format!("P1-T{:02}", index + 1);
        }
        for mutant in &mut packet.mutants {
            mutant.killing_tests = vec![packet.tests[0].id.clone()];
        }
        let err = verify_packet_shape(&packet)
            .expect_err("a requirement missing negative evidence must be rejected");
        assert!(
            err.to_string().contains("missing"),
            "unexpected error: {err}"
        );
    }

    /// M17.5 F-05 canary: a 100% kill rate carried by one coarse test is the
    /// clustering ADR-0020 §3 forbids.
    #[test]
    fn shape_check_rejects_mutation_coverage_clustered_on_one_test() {
        let mut packet = packet_from_repo();
        let sole = packet.tests[0].id.clone();
        for mutant in &mut packet.mutants {
            mutant.killing_tests = vec![sole.clone()];
        }
        let err = verify_packet_shape(&packet)
            .expect_err("kill concentration on one test must be rejected");
        assert!(
            err.to_string().contains("clustering"),
            "unexpected error: {err}"
        );
    }

    // ── M17.5 pass-2 #11/#12/#13: the fabricated-inventory cluster ──────────
    // An independent reviewer built packets whose rows named nothing real and
    // watched the verifier accept them. One canary per hole.

    /// #12: a requirement with no test behind it inflates the inventory while
    /// proving nothing. The old check exempted non-critical requirements.
    #[test]
    fn shape_check_rejects_a_requirement_mapped_to_no_test() {
        let mut packet = packet_from_repo();
        let orphan = packet.requirements.last().expect("requirements").id.clone();
        for test in &mut packet.tests {
            test.requirements.retain(|id| id != &orphan);
        }
        let err = verify_packet_shape(&packet)
            .expect_err("an unmapped requirement must be rejected even when non-critical");
        assert!(
            err.to_string().contains("not mapped to any test"),
            "unexpected error: {err}"
        );
    }

    /// #13: free-text operators defeat the concentration ceiling, because a
    /// label invented once can never exceed a share of the denominator.
    #[test]
    fn shape_check_rejects_an_invented_mutation_operator() {
        let mut packet = packet_from_repo();
        packet.mutants[0].operator = "artisanal-bespoke-operator".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("an operator outside the declared vocabulary must be rejected");
        assert!(
            err.to_string().contains("unknown mutation operator"),
            "unexpected error: {err}"
        );
    }

    /// #13: a mutant must attack something the inventory claims to require.
    #[test]
    fn shape_check_rejects_a_mutant_whose_source_is_not_a_requirement() {
        let mut packet = packet_from_repo();
        packet.mutants[0].source = "P1-R999".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("a mutant naming an unknown requirement must be rejected");
        assert!(
            err.to_string().contains("not a declared requirement"),
            "unexpected error: {err}"
        );
    }

    /// #13: an invented family would let a fabricated packet satisfy the
    /// per-family balance with labels that name no evidence family.
    #[test]
    fn shape_check_rejects_an_invented_mutation_family() {
        let mut packet = packet_from_repo();
        packet.mutants[0].family = "vibes/general-goodness".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("a family outside ADR-0020 §4's five must be rejected");
        assert!(
            err.to_string().contains("unknown mutation family"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shape_check_rejects_an_invented_generated_family() {
        let mut packet = packet_from_repo();
        packet.generated[0].family = "invented/family".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("generated evidence must name one of the five declared families");
        assert!(
            err.to_string().contains("generated family ids differ"),
            "unexpected error: {err}"
        );
    }

    /// #11: the qualified layer must reject a packet naming tests nobody wrote.
    /// The inventory layer must NOT, because predeclaring M19–M24's tests is
    /// the inventory's purpose — this pins both halves of that asymmetry.
    #[test]
    fn qualified_layer_rejects_test_names_that_do_not_exist() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.tests[0].name = "laws::this_test_was_never_written".to_owned();

        let err = verify_test_names_exist(&root, &packet)
            .expect_err("a qualification naming a nonexistent test must be rejected");
        assert!(
            err.to_string().contains("do not exist in the tree"),
            "unexpected error: {err}"
        );

        // The same packet stays valid at the inventory layer.
        verify_packet_shape(&packet).expect("predeclared names are legal in a proposed inventory");
    }

    #[test]
    fn runnable_test_scanner_rejects_comments_helpers_and_ignored_tests() {
        let mut names = BTreeMap::new();
        collect_runnable_test_functions(
            "// fn commented() { }\nfn helper() {}\n#[test]\nfn live() {}\n#[test]\n#[ignore]\nfn dormant() {}",
            &mut names,
        );
        assert_eq!(names, BTreeMap::from([("live".to_owned(), 1)]));
    }

    #[test]
    fn inventory_status_layer_still_rejects_completed_rows() {
        let mut packet = packet_from_repo();
        mark_complete(&mut packet);

        let err = verify_packet_statuses(
            &packet,
            PacketStatusExpectations {
                mutant: "predeclared",
                canary: "predeclared",
                generated: "planned",
                crash: "registered",
                review: "planned",
            },
        )
        .expect_err("inventory verifier must reject completed statuses");
        assert!(
            err.to_string().contains("mutant.disposition"),
            "unexpected error: {err}"
        );
    }

    /// #22: a canary row whose prose disagrees with the mutation the runner
    /// actually performs must be rejected. Without this, `gate` and `violation`
    /// are decorative and the canary table describes whatever it likes.
    #[test]
    fn canary_runner_rejects_prose_that_misdescribes_its_own_mutation() {
        let root = repo_root();
        let markdown = fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
            .expect("review markdown");
        let baseline = packet_from_repo();

        // The honest packet must pass, or the assertion below proves nothing.
        run_canary_suite(&baseline, &markdown).expect("the committed canary table must agree");

        let mut doctored = baseline.clone();
        doctored.canaries[8].violation = "reticulates splines".to_owned();
        let err = run_canary_suite(&doctored, &markdown)
            .expect_err("prose that misdescribes the executed mutation must fail closed");
        assert!(
            err.to_string().contains("C09.violation"),
            "unexpected error: {err}"
        );
    }

    /// #22, second half: a declared canary with no implemented mutation must
    /// never be recorded as caught.
    ///
    /// This exercises `mutate_canary` directly because the guard is
    /// defence-in-depth: `verify_canary_inventory` pins the id set to exactly
    /// C01–C16, so a C17 row is rejected before `run_canary_suite` ever sees it.
    /// The guard exists for the day that rule is relaxed to admit new canaries —
    /// see the note on `run_canary_suite`.
    #[test]
    fn canary_runner_refuses_a_declared_canary_it_cannot_execute() {
        let mut packet = packet_from_repo();
        let mut markdown = String::new();
        let err = mutate_canary("C17", &mut packet, &mut markdown, "")
            .expect_err("a canary with no arm must not be counted as caught");
        assert!(
            err.to_string().contains("unknown canary C17"),
            "unexpected error: {err}"
        );
    }

    /// The committed fuzz artifact must parse into the struct that checks it.
    /// `deny_unknown_fields` is what makes that meaningful: the artifact already
    /// carried `elapsed_s`, `seed` and `log`, and the struct silently discarded
    /// all three (#17).
    #[test]
    fn committed_fuzz_evidence_parses_with_every_field_checked() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let rows: Vec<FuzzEvidence> =
            serde_json::from_slice(&bytes).expect("every recorded field must be declared");
        assert_eq!(rows.len(), 7, "seven targets");
        assert!(
            rows.iter().all(|row| row.seed != 0),
            "the campaign seed must survive parsing"
        );

        let doctored = String::from_utf8(bytes)
            .expect("utf8")
            .replace("\"seed\":", "\"unchecked_new_field\":");
        serde_json::from_str::<Vec<FuzzEvidence>>(&doctored)
            .expect_err("a field the verifier does not check must not parse silently");
    }

    /// #17: a fuzz row naming a target that does not exist must be rejected. The
    /// old count-only check compared against `packet.generated.len()` — families,
    /// not targets — so an invented target name passed.
    #[test]
    fn fuzz_lane_rejects_a_target_that_does_not_exist() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();

        // The honest artifact must PASS, or the rejection below proves nothing.
        // This assertion used to expect failure, because the committed evidence
        // still carried F-09's crash; the campaign of 2026-07-30 is clean
        // (seven targets, 217 target-minutes, 0 crashes, 0 artifacts), so the
        // expectation flipped with the evidence.
        verify_fuzz_rows(&rows, &present).expect("the committed fuzz evidence must be clean");

        rows[0].target = "target_nobody_wrote".to_owned();
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("a target that does not exist must be rejected");
        assert!(
            err.to_string().contains("does not cover the targets"),
            "unexpected error: {err}"
        );
    }

    /// #17: a target claiming a clean full-length campaign that finished far
    /// short of its budget is stating a contradiction.
    #[test]
    fn fuzz_lane_rejects_a_clean_campaign_that_exited_early() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        // Make every row clean so the early-exit check is what fires. Note that
        // `elapsed_s` has to be normalised too: `canonical_round_trip` really did
        // stop early (1675s of an 1800s budget) because of F-09, so simply
        // clearing its exit code leaves behind exactly the contradiction this
        // check exists to catch.
        for row in &mut rows {
            row.exit_code = 0;
            row.artifacts = 0;
            row.elapsed_s = row.seconds + 1;
        }
        verify_fuzz_rows(&rows, &present).expect("an all-clean campaign is otherwise valid");

        rows[0].elapsed_s = 10;
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("a clean campaign cannot finish in a fraction of its budget");
        assert!(
            err.to_string().contains("not a coherent result"),
            "unexpected error: {err}"
        );
    }

    /// #17: rows carrying different seeds describe different campaigns, none of
    /// which is the one the packet points at. Checking only for a nonzero seed
    /// let the comment claim more than the code enforced.
    #[test]
    fn fuzz_lane_rejects_rows_from_different_campaigns() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        for row in &mut rows {
            row.exit_code = 0;
            row.artifacts = 0;
            row.elapsed_s = row.seconds + 1;
        }
        verify_fuzz_rows(&rows, &present).expect("one campaign, one seed");

        rows[2].seed += 1;
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("rows from two campaigns must be rejected");
        assert!(
            err.to_string().contains("distinct seeds"),
            "unexpected error: {err}"
        );
    }

    fn crash_evidence_from_repo() -> (CrashEvidence, BTreeSet<String>) {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/crash.json"))
            .expect("committed crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse");
        let declared = packet_from_repo()
            .crash_boundaries
            .iter()
            .map(|row| row.boundary.clone())
            .collect::<BTreeSet<_>>();
        (recorded, declared)
    }

    /// #19: the committed artifact and the packet must agree on which
    /// boundaries exist. The lane used to write into gitignored `target/`, so
    /// the artifact could say `pass` while the packet said `registered` and the
    /// inventory exited 0 without ever comparing them.
    #[test]
    fn crash_lane_rejects_evidence_that_does_not_match_the_packet() {
        let (recorded, declared) = crash_evidence_from_repo();
        verify_crash_rows(&recorded, &declared)
            .expect("the committed crash evidence must match the packet");

        let mut doctored = recorded.clone();
        doctored.boundaries.pop();
        let err = verify_crash_rows(&doctored, &declared)
            .expect_err("evidence missing a declared boundary must be rejected");
        assert!(
            err.to_string().contains("different boundaries"),
            "unexpected error: {err}"
        );
    }

    /// #19: a boundary that never fired is indistinguishable from a dead one
    /// (ADR-0020 §5), and a failing recovery must not read as evidence.
    #[test]
    fn crash_lane_rejects_an_unexercised_or_failing_boundary() {
        let (recorded, declared) = crash_evidence_from_repo();

        let mut unexercised = recorded.clone();
        unexercised.boundaries[0].occurrences_exercised = 0;
        let err = verify_crash_rows(&unexercised, &declared)
            .expect_err("an unexercised boundary must be rejected");
        assert!(
            err.to_string().contains("never exercised"),
            "unexpected error: {err}"
        );

        let mut residue = recorded;
        residue.boundaries[0].staged_residue = "one staged file".to_owned();
        let err =
            verify_crash_rows(&residue, &declared).expect_err("staged residue must be rejected");
        assert!(
            err.to_string().contains("staged residue"),
            "unexpected error: {err}"
        );
    }

    /// #20: a review may not count findings as VERIFIED that no second party
    /// reproduced. One model asserting a defect is not evidence of a defect.
    #[test]
    fn review_lane_rejects_verified_findings_nobody_reproduced() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        verify_review_evidence(&root, &packet).expect("the committed review rows are consistent");

        packet.reviews[0].unresolved_verified_findings = 2;
        packet.reviews[0].findings = vec!["P1-F01".to_owned(), "P1-F02".to_owned()];
        // ...but nothing was independently reproduced.
        let err = verify_review_evidence(&root, &packet)
            .expect_err("verified findings with no independent reproduction must be rejected");
        assert!(
            err.to_string().contains("independently reproduced only 0"),
            "unexpected error: {err}"
        );
    }

    /// #20: a review cannot claim to have reproduced a finding it never raised.
    #[test]
    fn review_lane_rejects_reproducing_a_finding_never_raised() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.reviews[1].independently_reproduced = vec!["P1-F99".to_owned()];
        let err = verify_review_evidence(&root, &packet)
            .expect_err("reproducing an unraised finding must be rejected");
        assert!(
            err.to_string().contains("never raised"),
            "unexpected error: {err}"
        );
    }

    /// #20: `pass` requires a committed record. The blind-review lane writes
    /// under `target/`, so without this a passing review is four numbers typed
    /// into the packet.
    #[test]
    fn review_lane_rejects_a_pass_with_no_committed_record() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.reviews[0].result = "pass".to_owned();
        let err = verify_review_evidence(&root, &packet)
            .expect_err("a passing review with no record must be rejected");
        assert!(
            err.to_string().contains("no committed record"),
            "unexpected error: {err}"
        );
    }

    /// #20 hardening: the packet is the artifact this verifier distrusts, so an
    /// evidence path that escapes the repository must be refused rather than
    /// read. `join` discards its base when given an absolute path.
    #[test]
    fn review_lane_refuses_an_evidence_path_that_escapes_the_repo() {
        let root = repo_root();
        for escape in ["/etc/passwd", "../../../etc/passwd"] {
            let mut packet = packet_from_repo();
            packet.reviews[0].result = "pass".to_owned();
            packet.reviews[0].evidence = Some(escape.to_owned());
            let err = verify_review_evidence(&root, &packet)
                .expect_err("an evidence path outside the repo must be refused");
            assert!(
                err.to_string().contains("escapes the repository"),
                "{escape} produced the wrong error: {err}"
            );
        }
    }

    /// #18: the generated artifact must be byte-identical across runs, or it
    /// cannot be hash-compared against a recorded digest. `elapsed_ms` used to
    /// make this impossible.
    ///
    /// This also guards the reproducibility of the generators themselves: any
    /// entropy they draw from outside the seeded `Rng` shows up here as a
    /// differing artifact.
    #[test]
    fn generated_evidence_is_byte_identical_across_runs() {
        let (first, _) = generate_evidence(64).expect("first run");
        let (second, _) = generate_evidence(64).expect("second run");
        assert_eq!(
            serde_json::to_string_pretty(&first).expect("serialize first"),
            serde_json::to_string_pretty(&second).expect("serialize second"),
            "the same seed produced two different artifacts, so the recorded \
             evidence hash certifies nothing"
        );
    }

    // ── M17.5 F-27 / P1-A05, A06, A07 ─────────────────────────────────────
    // The blind lane's own findings. `verify_review_evidence` counted the
    // record's attempts array and read nothing else, so a record of twelve
    // empty objects, a record contradicting its packet row, and two reviews
    // from one model family all passed. Each check gets the degenerate case
    // that must trip it.

    fn attempt(id: &str) -> ReviewRecordAttempt {
        ReviewRecordAttempt {
            id: id.to_owned(),
            attack_class: format!("vacuity:{id}"),
            target: "crates/liminal-xtask/src/haq.rs:1".to_owned(),
            attempt: format!("did a thing {id}"),
            observed_result: format!("saw a thing {id}"),
            independently_reproduced: false,
            classification: "caught_violation".to_owned(),
            resolved: false,
            resolution: None,
        }
    }

    fn review_record(pass: u8, family: &str, backend: &str) -> ReviewRecord {
        let root = repo_root();
        let commit = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let tree = git_text(&root, &["rev-parse", "HEAD^{tree}"]).expect("test tree");
        ReviewRecord {
            schema_version: "haqp-blind-review-v1".to_owned(),
            pass,
            reviewer: ReviewRecordReviewer {
                model_family: family.to_owned(),
                identity_hash: hex_digest(family.as_bytes()),
                backend: backend.to_owned(),
            },
            attempts: (0..12).map(|i| attempt(&format!("A{i}"))).collect(),
            findings: Vec::new(),
            independently_reproduced: Vec::new(),
            unresolved_verified_findings: 0,
            result: "pass".to_owned(),
            isolated_session_hash: "ef".repeat(32),
            sanitized_prompt_hash: hex_digest(format!("prompt:{family}").as_bytes()),
            fixed_base: ReviewFixedBase {
                commit,
                tree,
                clean: true,
            },
            blindness_proof: ReviewRecordBlindness {
                ephemeral_session_requested: backend == "lamu",
                session_state: if backend == "lamu" {
                    "ephemeral-cloud-session".to_owned()
                } else {
                    "fresh-codex-session".to_owned()
                },
                prior_pass_artifact_supplied: false,
                pass_two_original_spec_only: pass == 2,
            },
        }
    }

    fn review_row() -> Review {
        Review {
            reviewer: "openai".to_owned(),
            attempts: 12,
            findings: Vec::new(),
            independently_reproduced: Vec::new(),
            unresolved_verified_findings: 0,
            evidence: Some("conformance/haqp/evidence/review.json".to_owned()),
            result: "pass".to_owned(),
        }
    }

    #[test]
    fn a_well_formed_review_record_is_accepted() {
        verify_review_record(
            &repo_root(),
            &review_row(),
            &review_record(1, "openai", "codex"),
        )
        .expect("a complete record must pass, or every check below is vacuous");
    }

    /// P1-A05: array padding is not a set of attempts.
    #[test]
    fn review_record_rejects_empty_attempt_records() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[3].observed_result = "   ".to_owned();
        let err = verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("an attempt with no observed result is not an attempt");
        assert!(err.to_string().contains("empty observed_result"), "{err}");
    }

    #[test]
    fn review_record_rejects_unknown_classifications_and_duplicate_ids() {
        let mut unknown = review_record(1, "openai", "codex");
        unknown.attempts[0].classification = "inconclusive".to_owned();
        verify_review_record(&repo_root(), &review_row(), &unknown)
            .expect_err("a classification outside the declared set must be rejected");
        let mut duplicated = review_record(1, "openai", "codex");
        duplicated.attempts[1].id = duplicated.attempts[0].id.clone();
        verify_review_record(&repo_root(), &review_row(), &duplicated)
            .expect_err("twelve attempts must be twelve DISTINCT attempts");
    }

    #[test]
    fn review_record_rejects_an_unbound_fixed_tree() {
        let mut record = review_record(1, "openai", "codex");
        record.fixed_base.tree = "not-a-digest".to_owned();
        verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("a review record without a fixed tree binding is untrusted");
    }

    /// P1-A06: the packet may not summarize the record more kindly than the
    /// record summarizes itself.
    #[test]
    fn review_record_rejects_a_packet_row_that_contradicts_it() {
        let mut failed = review_record(1, "openai", "codex");
        failed.result = "fail".to_owned();
        let err = verify_review_record(&repo_root(), &review_row(), &failed)
            .expect_err("a pass row over a failing record must be rejected");
        assert!(err.to_string().contains("record says"), "{err}");

        let mut unresolved = review_record(1, "openai", "codex");
        unresolved.unresolved_verified_findings = 9;
        verify_review_record(&repo_root(), &review_row(), &unresolved)
            .expect_err("a row claiming zero unresolved findings over a record counting nine");
    }

    /// P1-A07: blindness is a property of the run, and it is recorded.
    #[test]
    fn review_record_rejects_a_pass_that_saw_prior_artifacts() {
        let mut leaked = review_record(1, "openai", "codex");
        leaked.blindness_proof.prior_pass_artifact_supplied = true;
        verify_review_record(&repo_root(), &review_row(), &leaked)
            .expect_err("a reviewer shown the prior pass is not blind");

        let mut informed = review_record(2, "xiaomi", "lamu");
        informed.blindness_proof.pass_two_original_spec_only = false;
        verify_review_record(&repo_root(), &review_row(), &informed)
            .expect_err("pass 2 must start from the original spec alone");
    }

    /// P1-A07: two reviews from one family satisfy every other check here.
    #[test]
    fn independence_rejects_reviews_that_share_a_family_backend_or_identity() {
        let distinct = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "xiaomi", "lamu")),
        ];
        verify_reviewer_independence(&distinct)
            .expect("two genuinely distinct reviewers must pass");

        let same_family = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "openai", "lamu")),
        ];
        let err = verify_reviewer_independence(&same_family)
            .expect_err("two reviews from one model family are not independent");
        assert!(err.to_string().contains("model families"), "{err}");

        let same_backend = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "anthropic", "codex")),
        ];
        verify_reviewer_independence(&same_backend)
            .expect_err("two reviews on one backend are not independent");
    }

    // ── M17.5 F-27 / P1-A04, P2-F03 ───────────────────────────────────────
    // Both blind passes, independently, found generated evidence unbound: the
    // hash was checked for LENGTH only and the artifact the generator wrote was
    // gitignored, so nothing could read it back.

    fn generated_row(family: &str) -> Generated {
        Generated {
            family: family.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            fuzz_targets: vec!["cst_parse".to_owned()],
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
        }
    }

    fn generated_evidence_row(family: &str) -> GeneratedEvidence {
        GeneratedEvidence {
            family: family.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            seed: 7,
            evidence_hash: "ab".repeat(32),
        }
    }

    #[test]
    fn generated_rows_accept_a_packet_that_matches_its_run() {
        verify_generated_rows(&[generated_row("f")], &[generated_evidence_row("f")])
            .expect("a matching packet must pass, or every check below is vacuous");
    }

    #[test]
    fn generated_rows_reject_a_hash_no_run_produced() {
        let mut doctored = generated_row("f");
        doctored.evidence_hash = Some("cd".repeat(32));
        let err = verify_generated_rows(&[doctored], &[generated_evidence_row("f")])
            .expect_err("a packet may not cite a digest the run never recorded");
        assert!(err.to_string().contains("evidence hash"), "{err}");
    }

    #[test]
    fn generated_rows_reject_counts_and_seeds_that_disagree_with_the_run() {
        let mut counts = generated_row("f");
        counts.accepted = 99_999;
        counts.discards = 1;
        verify_generated_rows(&[counts], &[generated_evidence_row("f")])
            .expect_err("packet counts must match the committed run");

        let mut seed = generated_row("f");
        seed.seed = Some(8);
        verify_generated_rows(&[seed], &[generated_evidence_row("f")])
            .expect_err("packet seed must match the committed run");
    }

    #[test]
    fn generated_rows_reject_a_family_with_no_committed_run() {
        verify_generated_rows(&[generated_row("ghost")], &[generated_evidence_row("f")])
            .expect_err("a family claiming pass with no run in the artifact");
    }

    #[test]
    fn generated_artifact_requires_exact_unique_family_set() {
        let scratch = liminal_scratch::ScratchDir::new("haq-generated-artifact").expect("scratch");
        let root: &Utf8Path = &scratch;
        let rows = GENERATED_FAMILIES
            .iter()
            .map(|family| generated_evidence_row(family))
            .collect::<Vec<_>>();
        let artifact = GeneratedEvidenceArtifact {
            schema_version: "haqp-generated-v1".to_owned(),
            artifact_blake3: generated_artifact_digest(&rows).expect("digest"),
            rows: rows.clone(),
        };
        let path = root.join("conformance/haqp/evidence/generated.json");
        fs::create_dir_all(path.parent().expect("evidence parent")).expect("mkdir");
        fs::write(&path, serde_json::to_vec(&artifact).expect("json")).expect("write");
        let packet_rows = GENERATED_FAMILIES
            .iter()
            .map(|family| generated_row(family))
            .collect::<Vec<_>>();
        let packet = Packet {
            generated: packet_rows,
            ..read_packet(&repo_root()).expect("packet")
        };
        verify_generated_evidence(root, &packet).expect("exact family artifact must pass");

        let mut duplicate = rows;
        duplicate[0].family = duplicate[1].family.clone();
        let tampered = GeneratedEvidenceArtifact {
            schema_version: "haqp-generated-v1".to_owned(),
            artifact_blake3: generated_artifact_digest(&duplicate).expect("digest"),
            rows: duplicate,
        };
        fs::write(&path, serde_json::to_vec(&tampered).expect("json")).expect("write");
        verify_generated_evidence(root, &packet)
            .expect_err("duplicate or missing family cannot qualify generated evidence");
    }

    /// The inventory layer accepted any 64 characters, including 64 spaces.
    #[test]
    fn inventory_rejects_an_evidence_hash_that_is_not_hexadecimal() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        for family in &mut packet.generated {
            family.result = "pass".to_owned();
            family.seed = Some(1);
            family.evidence_hash = Some(" ".repeat(64));
        }
        let err =
            verify_generated_inventory(&packet).expect_err("64 spaces is not a digest of anything");
        assert!(err.to_string().contains("64-hex"), "{err}");
    }

    // ── M17.5 F-27 / P1-A03 ───────────────────────────────────────────────
    // The qualified path accepted `caught` on every canary row while never
    // invoking the runner. Canaries are what prove the gate can fail at all, so
    // a canary table nobody executed proves nothing.

    fn canary_row(id: &str) -> Canary {
        Canary {
            id: id.to_owned(),
            gate: "status".to_owned(),
            violation: "set status to ratified".to_owned(),
            expected_failure: "status: expected proposed".to_owned(),
            result: "caught".to_owned(),
        }
    }

    fn canary_evidence_row(id: &str) -> CanaryEvidence {
        CanaryEvidence {
            id: id.to_owned(),
            gate: "status".to_owned(),
            violation: "set status to ratified".to_owned(),
            expected_failure: "status: expected proposed".to_owned(),
            observed_failure: "status: expected proposed, found ratified".to_owned(),
            caught: true,
            mutation_semantics: "packet.status := ratified".to_owned(),
        }
    }

    #[test]
    fn canary_rows_accept_a_packet_that_matches_its_run() {
        verify_canary_rows(&[canary_row("C01")], &[canary_evidence_row("C01")])
            .expect("a matching packet must pass, or every check below is vacuous");
    }

    #[test]
    fn canary_rows_reject_a_caught_claim_with_no_run() {
        let err = verify_canary_rows(&[canary_row("C01")], &[])
            .expect_err("`caught` with nothing in the artifact is an assertion, not evidence");
        assert!(err.to_string().contains("no such canary"), "{err}");
    }

    #[test]
    fn canary_rows_reject_a_claim_the_run_contradicts() {
        let mut missed = canary_evidence_row("C01");
        missed.caught = false;
        let err = verify_canary_rows(&[canary_row("C01")], &[missed])
            .expect_err("a packet may not claim caught over a run that recorded a miss");
        assert!(err.to_string().contains("NOT caught"), "{err}");
    }

    /// pass-2 #22 made the runner check its own prose; this extends the same
    /// requirement to the committed artifact.
    #[test]
    fn canary_rows_reject_prose_that_describes_a_different_experiment() {
        let mut relabelled = canary_evidence_row("C01");
        relabelled.violation = "delete a crash boundary".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[relabelled])
            .expect_err("the packet's violation must be the one performed");

        let mut expectation = canary_evidence_row("C01");
        expectation.expected_failure = "something else entirely".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[expectation])
            .expect_err("the packet's expected failure must be the one asserted");
    }

    #[test]
    fn canary_rows_reject_wrong_gate_or_mutation_semantics() {
        let mut wrong_gate = canary_evidence_row("C01");
        wrong_gate.gate = "markdown".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[wrong_gate])
            .expect_err("the evidence must name the gate actually exercised");
        let mut wrong_mutation = canary_evidence_row("C01");
        wrong_mutation.mutation_semantics = "no mutation".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[wrong_mutation])
            .expect_err("the exact mutation semantics must be bound");
    }

    #[test]
    fn canary_rows_reject_an_undeclared_canary_in_the_run() {
        verify_canary_rows(
            &[canary_row("C01")],
            &[canary_evidence_row("C01"), canary_evidence_row("C99")],
        )
        .expect_err("a packet that is a subset of the experiment hides rows from the reader");
    }

    /// M17.5 F-27 / P1-A09: the requirement was unrepresentable, so it was
    /// unverifiable.
    #[test]
    fn fuzz_lane_rejects_a_campaign_with_no_sanitizer() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        verify_fuzz_rows(&rows, &present).expect("the committed campaign is sanitized");
        rows[0].sanitizer = "none".to_owned();
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("an unsanitized campaign misses every defect ASan exists to find");
        assert!(err.to_string().contains("sanitizer"), "{err}");
    }

    // ── M17.5 F-28 / P1-A02 ───────────────────────────────────────────────
    // A kill witnessed by a test that cannot run is not a kill.

    #[test]
    fn ignored_attribute_is_attached_to_the_following_test() {
        let mut names = BTreeSet::new();
        collect_ignored(
            "#[test]\n#[ignore = \"Phase 1\"]\nfn quarantined() {}\n#[test]\nfn active() {}\n",
            &mut names,
        );
        assert!(
            names.contains("quarantined"),
            "the ignored test must be seen"
        );
        assert!(
            !names.contains("active"),
            "an #[ignore] must not leak onto the next test after it"
        );
    }

    /// The packet as committed declares every mutant `predeclared`, so the
    /// check is inert — flipping one to `killed` must wake it.
    #[test]
    fn qualified_layer_rejects_a_kill_witnessed_by_an_ignored_test() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        verify_mutant_killing_tests(&root, &packet)
            .expect("no mutant claims killed, so nothing to check");

        // Use P1-T08, the migration test that remains quarantined pending a
        // persisted-format ADR. The active Phase 1 tests cannot witness this
        // negative case after suite breadth is enabled.
        packet.mutants[0].disposition = "killed".to_owned();
        packet.mutants[0].killing_tests = vec!["P1-T08".to_owned()];
        let err = verify_mutant_killing_tests(&root, &packet)
            .expect_err("an #[ignore]d test cannot witness a kill");
        assert!(err.to_string().contains("#[ignore]d"), "{err}");
    }

    #[test]
    fn qualified_layer_rejects_a_kill_with_no_killing_test_at_all() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        packet.mutants[0].disposition = "killed".to_owned();
        packet.mutants[0].killing_tests.clear();
        let err = verify_mutant_killing_tests(&root, &packet)
            .expect_err("a kill with no witness is an assertion");
        assert!(err.to_string().contains("names no killing test"), "{err}");
    }

    // ── ADR-0021: staged qualification ────────────────────────────────────
    // The mutation requirement defers to 1b because ADR-0020 §3 assumed the
    // suite it mutates exists, and for Phase 1 it does not (F-28). Both
    // directions of the stage claim must be errors.

    #[test]
    fn the_committed_packet_declares_a_stage_it_satisfies() {
        let packet = read_packet(&repo_root()).expect("packet");
        assert_eq!(packet.qualification_stage, "1a");
        verify_qualification_stage(&packet).expect("the committed packet must be consistent");
    }

    #[test]
    fn stage_1a_may_not_claim_mutant_kills() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.mutants[0].disposition = "killed".to_owned();
        let err = verify_qualification_stage(&packet)
            .expect_err("1a defers the mutation requirement; claiming a kill smuggles it back");
        assert!(
            err.to_string().contains("defers the mutation requirement"),
            "{err}"
        );
    }

    #[test]
    fn stage_1b_must_actually_carry_the_mutation_evidence() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1b".to_owned();
        let err = verify_qualification_stage(&packet)
            .expect_err("1b without kills claims the stronger stage on the weaker evidence");
        assert!(err.to_string().contains("no mutant is killed"), "{err}");
    }

    #[test]
    fn an_unknown_stage_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1c".to_owned();
        verify_qualification_stage(&packet).expect_err("an unknown stage must not pass");
    }

    // ── M17.5 F-17 / F-29: derived fuzz budget and the family→target mapping ──
    // The packet maps families to targets; the artifact supplies target seconds.

    fn fuzz_row(target: &str, seconds: u64) -> FuzzEvidence {
        FuzzEvidence {
            target: target.to_owned(),
            seconds,
            elapsed_s: seconds,
            exit_code: 0,
            execs: 1,
            artifacts: 0,
            seed: 1,
            sanitizer: "address".to_owned(),
            log: format!("target/haqp/fuzz-{target}.log"),
            log_blake3: "ab".repeat(32),
        }
    }

    fn family_row(name: &str, targets: &[&str]) -> Generated {
        Generated {
            family: name.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            fuzz_targets: targets.iter().map(|t| (*t).to_owned()).collect(),
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
        }
    }

    #[test]
    fn a_family_whose_targets_meet_the_floor_is_accepted() {
        let families = [
            family_row("a", &["t1", "t2"]),
            family_row("b", &["t3"]),
            family_row("c", &["t4"]),
            family_row("d", &["t5"]),
            family_row("e", &["t6"]),
        ];
        let recorded: Vec<FuzzEvidence> = ["t1", "t2", "t3", "t4", "t5", "t6"]
            .iter()
            .map(|t| fuzz_row(t, 1800))
            .collect();
        verify_fuzz_budget(&families, &recorded)
            .expect("6 targets x 30 min = 180 >= 150, every family >= 30");
    }

    /// One campaign must not satisfy two families' budgets.
    #[test]
    fn a_fuzz_target_may_not_be_claimed_by_two_families() {
        let families = [family_row("a", &["shared"]), family_row("b", &["shared"])];
        let err = verify_fuzz_target_mapping(&families)
            .expect_err("a shared target double-counts into two budgets");
        assert!(err.to_string().contains("claimed by both"), "{err}");
    }

    /// ADR-0020 §4 gives EVERY family its own campaign. Two families currently
    /// have no fuzz target at all (F-29), and `pass` must be unreachable for
    /// them until one exists.
    #[test]
    fn a_family_claiming_pass_with_no_fuzz_target_is_refused() {
        let families = [family_row("uncovered", &[])];
        let err = verify_fuzz_budget(&families, &[])
            .expect_err("a family with no target has had no campaign");
        assert!(err.to_string().contains("names no fuzz target"), "{err}");
    }

    #[test]
    fn a_family_below_the_thirty_minute_floor_is_refused() {
        let families = [family_row("a", &["t1"])];
        let err = verify_fuzz_budget(&families, &[fuzz_row("t1", 60)])
            .expect_err("one minute is not thirty");
        assert!(err.to_string().contains("below the 30"), "{err}");
    }

    #[test]
    fn a_family_naming_a_target_the_campaign_never_ran_is_refused() {
        let families = [family_row("a", &["ghost"])];
        verify_fuzz_budget(&families, &[fuzz_row("t1", 1800)])
            .expect_err("a target with no recorded run evidences nothing");
    }

    /// F-17: the log lives under target/, so its digest is the only thing that
    /// can make the recorded counts checkable afterwards.
    #[test]
    fn fuzz_rows_require_a_real_log_digest() {
        let present = BTreeSet::from(["t1".to_owned()]);
        let good = vec![fuzz_row("t1", 9000)];
        verify_fuzz_rows(&good, &present).expect("a well-formed row must pass");

        let mut empty = good.clone();
        empty[0].log_blake3 = String::new();
        verify_fuzz_rows(&empty, &present)
            .expect_err("an absent digest leaves the counts unfalsifiable");

        let mut garbage = good;
        garbage[0].log_blake3 = " ".repeat(64);
        verify_fuzz_rows(&garbage, &present).expect_err("64 spaces is not a digest");
    }

    // ── M17.5 Stage 3, tier 1: the verifier's own survivors ────────────────
    // A scoped campaign over haq.rs left 75. The ones below are in gate logic
    // written THIS session, and they share a cause worth naming: the canaries
    // asserted that bad input is rejected and never that good input is
    // accepted, so any mutant that widens a guard survived. That is the exact
    // defect this milestone has been finding elsewhere.

    /// Kills `== ParentDir` -> `!=` at the evidence-path guard: under `!=`
    /// every ordinary component looks like an escape and nothing can pass.
    #[test]
    fn an_ordinary_relative_evidence_path_is_accepted() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.reviews[0].result = "pass".to_owned();
        packet.reviews[0].evidence = Some("conformance/haqp/evidence/nope.json".to_owned());
        let err = verify_review_evidence(&repo_root(), &packet)
            .expect_err("the file does not exist, so this must fail on ABSENCE");
        assert!(
            err.to_string().contains("record is missing"),
            "a plain relative path must reach the read, not be refused as an escape: {err}"
        );
    }

    /// Kills the `1b if !claims_kills` guard -> `true`: a 1b packet that DOES
    /// carry kills must be accepted, or the stage can never be satisfied.
    #[test]
    fn stage_1b_with_kills_is_accepted() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1b".to_owned();
        for mutant in &mut packet.mutants {
            mutant.disposition = "killed".to_owned();
        }
        verify_qualification_stage(&packet)
            .expect("1b carrying mutation evidence is the whole point of 1b");
    }

    /// Kills both boolean mutants in the non-runnable scanner.
    #[test]
    fn the_scanner_recognises_each_non_runnable_form_and_nothing_else() {
        let mut names = BTreeSet::new();
        collect_ignored(
            "#[cfg(any())]\nfn compiled_out() {}\n\
             #[cfg_attr(unix, ignore)]\nfn conditionally_ignored() {}\n\
             // a comment mentioning ignore\nfn merely_mentioned() {}\n\
             #[cfg_attr(unix, should_panic)]\nfn other_cfg_attr() {}\n",
            &mut names,
        );
        assert!(names.contains("compiled_out"), "#[cfg(any())] never runs");
        assert!(
            names.contains("conditionally_ignored"),
            "a conditional ignore still cannot witness a kill"
        );
        assert!(
            !names.contains("merely_mentioned"),
            "prose containing the word must not mark the next test non-runnable"
        );
        assert!(
            !names.contains("other_cfg_attr"),
            "a cfg_attr that is not an ignore leaves the test runnable"
        );
    }

    /// Kills `seconds / 60` -> `* 60`, `< 150` -> `>`, `30 * 60` -> `+`/`/`,
    /// and `elapsed_s + 60` -> `*` / `<=`. Each bound is checked from BOTH
    /// sides, because a one-sided check is satisfied by a mutant that moves it.
    #[test]
    fn fuzz_row_floors_hold_exactly_at_their_boundaries() {
        let targets: BTreeSet<String> = (0..5).map(|i| format!("t{i}")).collect();
        let rows = |secs: u64| -> Vec<FuzzEvidence> {
            (0..5).map(|i| fuzz_row(&format!("t{i}"), secs)).collect()
        };

        // 5 x 1800s = 150 minutes exactly: the floor is inclusive.
        verify_fuzz_rows(&rows(1800), &targets).expect("exactly 150 target-minutes must pass");
        // One second under the per-target floor must fail.
        verify_fuzz_rows(&rows(1799), &targets)
            .expect_err("1799s is below the 30-minute per-target floor");

        // The TOTAL floor has to be reached on its own. Five targets at 1800s
        // can never breach it, so the first version of this test never
        // executed line 766 at all and three mutants there survived a check
        // that looked like it covered them. Four targets do reach it.
        let four: BTreeSet<String> = (0..4).map(|i| format!("t{i}")).collect();
        let four_rows: Vec<FuzzEvidence> =
            (0..4).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        verify_fuzz_rows(&four_rows, &four)
            .expect_err("4 x 30 = 120 target-minutes is below the 150 total");

        // And the per-target floor must be reached with the total SATISFIED,
        // or a mutant that moves the floor down (30*60 -> 30+60, or -> 30/60)
        // is never exercised: those need a target above 90s and below 1800s.
        let six: BTreeSet<String> = (0..6).map(|i| format!("t{i}")).collect();
        let mut six_rows: Vec<FuzzEvidence> =
            (0..6).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        six_rows[5].seconds = 600;
        six_rows[5].elapsed_s = 600;
        verify_fuzz_rows(&six_rows, &six)
            .expect_err("600s is above 90 and below 1800: only the real floor rejects it");

        // The early-exit check compares elapsed against budget with 60s slack.
        let mut exact = rows(1800);
        exact[0].elapsed_s = 1740; // 1740 + 60 == 1800, the boundary
        verify_fuzz_rows(&exact, &targets).expect("elapsed + 60 == seconds is not an early exit");
        let mut early = rows(1800);
        early[0].elapsed_s = 1739;
        verify_fuzz_rows(&early, &targets)
            .expect_err("a clean campaign that stopped early states a contradiction");
    }

    /// Kills `< 150` -> `==` / `<=` in the DERIVED budget: 150 exactly passes,
    /// 149 fails.
    #[test]
    fn the_derived_budget_floor_is_inclusive() {
        let families: Vec<Generated> = (0..5)
            .map(|i| family_row(&format!("f{i}"), &[&format!("t{i}")]))
            .collect();
        let recorded: Vec<FuzzEvidence> =
            (0..5).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        verify_fuzz_budget(&families, &recorded).expect("exactly 150 derived minutes must pass");

        let short: Vec<FuzzEvidence> = recorded
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut row = row.clone();
                if i == 0 {
                    row.seconds = 1740; // requested 29 minutes
                    row.elapsed_s = 1740; // measured 29 minutes: family and total floor
                }
                row
            })
            .collect();
        verify_fuzz_budget(&families, &short).expect_err("149 derived minutes is below 150");
    }

    // ── M17.5 F-30: the repo-level gates had no test at all ────────────────
    // `verify_inventory_repo`, `verify_qualified_repo` and every evidence
    // binder survived being replaced with `Ok(())`. They are exercised only by
    // justfile recipes, so `just ci` catches a break and the MUTATION lane
    // cannot see them — a gate invisible to the measurement that judges the
    // suite. Each is now driven directly.

    #[test]
    fn the_inventory_gate_accepts_the_committed_tree() {
        verify_inventory_repo(&repo_root())
            .expect("the committed packet must satisfy its own inventory gate");
    }

    /// The packet is `proposed`/`not-run`, so the qualified gate must REFUSE
    /// it — and refuse it for that reason, not by accident.
    #[test]
    fn the_qualified_gate_refuses_a_packet_that_has_not_run() {
        let err =
            verify_qualified_repo(&repo_root()).expect_err("a not-run packet is not qualified");
        assert!(
            err.to_string().contains("qualification_state"),
            "it must refuse for the stated reason: {err}"
        );
    }

    /// Each evidence binder must actually READ its artifact. Replacing any of
    /// them with `Ok(())` was invisible because the qualified gate's
    /// `qualification_state` check fires first and they were never reached.
    #[test]
    fn every_evidence_binder_rejects_a_claim_its_artifact_contradicts() {
        let root = repo_root();

        let mut generated = read_packet(&root).expect("packet");
        generated.generated[0].result = "pass".to_owned();
        generated.generated[0].seed = Some(999_999);
        generated.generated[0].evidence_hash = Some("cd".repeat(32));
        verify_generated_evidence(&root, &generated)
            .expect_err("a generated family citing a run that did not happen");

        let mut canaries = read_packet(&root).expect("packet");
        canaries.canaries[0].result = "caught".to_owned();
        canaries.canaries[0].violation = "something the run never performed".to_owned();
        verify_canary_evidence(&root, &canaries)
            .expect_err("a canary whose prose contradicts the committed run");

        let mut crash = read_packet(&root).expect("packet");
        crash.crash_boundaries.remove(0);
        verify_crash_evidence(&root, &crash)
            .expect_err("a packet that drops a boundary the fault lane exercised");

        let mut fuzz = read_packet(&root).expect("packet");
        for family in &mut fuzz.generated {
            family.result = "pass".to_owned();
        }
        fuzz.generated[0].fuzz_targets[0] = "never_ran".to_owned();
        verify_fuzz_evidence(&root, &fuzz)
            .expect_err("a packet claiming an unrecorded target cannot qualify");

        let mut markdown = read_packet(&root).expect("packet");
        markdown.suite_version = "tampered".to_owned();
        verify_markdown_surface(&root, &markdown)
            .expect_err("a packet whose digest no longer matches the reviewed markdown");
    }

    /// `verify_provenance` binds qualification to a specific fixed commit and
    /// a clean tree; replacing it with `Ok(())` unbinds the packet from any
    /// tree. The packet itself is expected to be that commit's child.
    #[test]
    fn provenance_rejects_a_packet_bound_to_another_commit() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        if let Some(provenance) = packet.provenance.as_mut() {
            provenance.fixed_commit = "0".repeat(40);
            verify_provenance(&root, &packet)
                .expect_err("a packet naming a commit that is not HEAD is bound to nothing");
        } else {
            verify_provenance(&root, &packet)
                .expect_err("a packet with no provenance block is bound to nothing");
        }
    }

    /// Kills `verify_inventory_repo -> Ok(())`.
    ///
    /// The earlier test asserted the gate ACCEPTS the committed tree, which
    /// `Ok(())` also does — so the mutant survived a test written for it. A
    /// gate is only pinned by a tree it must REFUSE, which means building one.
    #[test]
    fn the_inventory_gate_refuses_a_doctored_tree() {
        let scratch = liminal_scratch::ScratchDir::new("haq-inventory-gate").expect("scratch dir");
        let root: &Utf8Path = &scratch;
        let source = repo_root();

        for rel in [
            "conformance/haqp/packet.json",
            "docs/execution/phase1-suite-review.md",
        ] {
            let dest = root.join(rel);
            fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            fs::copy(source.join(rel), &dest).expect("copy fixture");
        }
        let packet: Packet = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/packet.json")).expect("read packet fixture"),
        )
        .expect("parse packet fixture");
        for requirement in &packet.requirements {
            let source_file = requirement
                .source
                .split_once(':')
                .expect("requirement source coordinate")
                .0;
            let dest = root.join(source_file);
            fs::create_dir_all(dest.parent().expect("source parent")).expect("mkdir");
            fs::copy(source.join(source_file), &dest).expect("copy requirement source");
        }
        // Sanity: the COPY must pass, or the refusal below proves nothing about
        // the doctoring.
        verify_inventory_repo(root).expect("an unmodified copy must still pass");

        // A proposed packet may not declare itself ratified.
        let path = root.join("conformance/haqp/packet.json");
        let mut packet: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("parse");
        packet["ratification"] = serde_json::Value::String("ratified".to_owned());
        fs::write(
            &path,
            serde_json::to_vec_pretty(&packet).expect("serialize"),
        )
        .expect("write");

        verify_inventory_repo(root)
            .expect_err("a packet declaring itself ratified must be refused");
    }

    // ── M17.5 F-30: ADR-0020 §4's generator machinery ─────────────────────
    // The 100,000-case evidence is only as good as the generator behind it. A
    // constant RNG, or a `word()` that returns one string, produces 100,000
    // "cases" that are one case repeated — and every count in the packet would
    // still look satisfied.

    /// Kills `^` -> `|`/`&` and `>>` -> `<<` in `Rng::next`.
    ///
    /// Pinned as a GOLDEN prefix from a fixed seed rather than by a statistical
    /// property: the recorded packet seed exists to make a campaign
    /// reproducible (ADR-0020 §1), and reproducibility is exactly "this seed
    /// yields this stream".
    #[test]
    fn the_seeded_stream_is_a_fixed_sequence() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let drawn: Vec<u64> = (0..4).map(|_| rng.next()).collect();
        let mut again = Rng(0x9E37_79B9_7F4A_7C15);
        let repeat: Vec<u64> = (0..4).map(|_| again.next()).collect();
        assert_eq!(drawn, repeat, "the same seed must yield the same stream");
        // Distinct draws: a stream that repeats one value reproduces nothing.
        assert!(
            drawn.iter().collect::<BTreeSet<_>>().len() == 4,
            "four draws collapsed to fewer values: {drawn:?}"
        );
    }

    /// Kills `Rng::below -> 1`, and pins the contract callers rely on.
    #[test]
    fn below_stays_in_range_and_actually_varies() {
        let mut rng = Rng(12_345);
        let draws: Vec<u64> = (0..500).map(|_| rng.below(7)).collect();
        assert!(draws.iter().all(|d| *d < 7), "below(7) escaped its bound");
        assert!(
            draws.iter().collect::<BTreeSet<_>>().len() >= 5,
            "below(7) produced fewer than 5 distinct values in 500 draws — a \
             generator that returns a constant makes 100,000 cases into one"
        );
        // below(0) must not divide by zero.
        assert_eq!(rng.below(0), 0, "below(0) must be total");
    }

    /// Kills `Rng::word -> "xyzzy"` and `+` -> `*` in its length.
    #[test]
    fn words_vary_in_content_and_length() {
        let mut rng = Rng(999);
        let words: Vec<String> = (0..200).map(|_| rng.word()).collect();
        assert!(
            words.iter().collect::<BTreeSet<_>>().len() > 100,
            "200 draws produced fewer than 100 distinct words"
        );
        for word in &words {
            assert!(
                (1..=12).contains(&word.len()),
                "word {word:?} has length {} outside 1..=12",
                word.len()
            );
            assert!(
                word.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b" -_".contains(&b)),
                "word {word:?} left the declared alphabet"
            );
        }
        assert!(
            words.iter().any(|w| w.len() > 1),
            "`1 + below(12)` became `1 * below(12)` if every word is one char"
        );
    }

    /// Kills `>` -> `>=`/`==` at the safety limit and `== 0` at the floor.
    /// Both boundaries, from both sides — the reason the guard was extracted.
    #[test]
    fn the_case_count_guard_holds_exactly_at_its_boundaries() {
        validate_case_count(0).expect_err("zero cases is not a campaign");
        validate_case_count(1).expect("one case is a legal, if small, request");
        validate_case_count(10_000_000).expect("the limit itself must be allowed");
        validate_case_count(10_000_001).expect_err("one past the limit must be refused");
    }

    /// Kills the three `generate_evidence -> Ok((vec![], ...))` replacements.
    ///
    /// `generated_evidence_is_byte_identical_across_runs` compares two runs to
    /// each other, and two EMPTY runs are identical — so it passed under every
    /// one of them. Shape has to be asserted directly.
    #[test]
    fn generated_evidence_has_one_row_per_family_that_met_its_target() {
        let (rows, timings) = generate_evidence(64).expect("a small run must succeed");
        assert_eq!(rows.len(), 5, "ADR-0020 §4 names five critical families");
        assert_eq!(timings.len(), 5, "one timing per family");
        let families: BTreeSet<&str> = rows.iter().map(|row| row.family.as_str()).collect();
        assert_eq!(families.len(), 5, "family names must be distinct");
        for row in &rows {
            assert_eq!(
                row.accepted, 64,
                "{}: acceptance target not met",
                row.family
            );
            assert_eq!(
                row.accepted + row.discards,
                row.attempts,
                "{}: counts do not describe one run",
                row.family
            );
            assert_eq!(row.evidence_hash.len(), 64, "{}: no digest", row.family);
        }
    }

    /// Golden digests for a fixed 64-case run — the strongest available pin on
    /// the case BUILDERS (M17.5 F-30).
    ///
    /// `case_repair` alone left eight survivors, all in the cyclic-dependency
    /// construction: edge density, the `count >= 2` guard, and four operators
    /// inside the `retain` that clears the back edge before closing a real
    /// cycle. Eight behavioural assertions over an RNG-driven builder would be
    /// fragile and would still miss combinations; the digest covers every arm
    /// at once, because any change to what a case CONTAINS changes it.
    ///
    /// `generated_evidence_is_byte_identical_across_runs` cannot do this job:
    /// it compares two runs to each other, so it is satisfied by any builder
    /// that is merely consistent — including a mutated one.
    ///
    /// These constants are expected to change when a generator is deliberately
    /// changed. That is the point: an unexplained change here means a case
    /// builder moved, and ADR-0020 §1 makes an unreproducible campaign
    /// ineligible. Re-record only alongside the change that caused it.
    #[test]
    fn generated_case_builders_match_their_recorded_goldens() {
        const GOLDENS: [(&str, &str); 5] = [
            (
                "source/CST/formatting",
                "9e670f70ef7ff2a755a399a1b12b686d3e050a654af046240499f0747c66d581",
            ),
            (
                "graph/interchange codecs",
                "9c051ab379bb0a31f2f5af36a0ed014137e226688fec4647daa47643ce6cde39",
            ),
            (
                "transforms/projections",
                "8228d2bea8348dee308675e43cb80a0f9a421cbca1a101e3355803de2839ef9d",
            ),
            (
                "repair/ILRP/recovery",
                "47b7ad5ee6e85b43ae80cbca33fa01303cac5bb01f8dcebe75c2bfcd515c6fec",
            ),
            (
                "Basis/revision/query invalidation",
                "ef34bf949971ba00bdcad83d9f49620bef137232f36873ecb6f7a8c9005c89dd",
            ),
        ];
        // 3,000 rather than 64: `case_repair` closes a genuine cycle on
        // roughly 1 case in 384, so a 64-case run never executes the branch and
        // six mutants inside it survived a golden that appeared to cover them.
        // 3,000 exercises every arm and still runs in ~0.6s.
        let (rows, _) = generate_evidence(3_000).expect("a small run must succeed");
        for (row, (family, digest)) in rows.iter().zip(GOLDENS) {
            assert_eq!(row.family, family, "family order must be stable");
            assert_eq!(
                row.evidence_hash, digest,
                "{family}: a case builder changed without its golden being re-recorded"
            );
        }
    }

    // ── M17.5 F-30: inventory thresholds, from BOTH sides ─────────────────
    // Seven survivors sat on thresholds that were only ever tested from the
    // failing side, so any mutant that moved a bound by one survived. A bound
    // is defined by the pair of values that straddle it.

    /// `kind.is_empty() || source.is_empty()` — kills `||` -> `&&`, which would
    /// accept a requirement missing ONE of the two.
    #[test]
    fn a_requirement_missing_either_field_is_refused() {
        for (kind, source) in [("", "v4 §112"), ("law", "")] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.requirements[0].kind = kind.to_owned();
            packet.requirements[0].source = source.to_owned();
            verify_packet_shape(&packet)
                .expect_err("a requirement missing kind OR source is incomplete");
        }
    }

    #[test]
    fn an_unknown_requirement_kind_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.requirements[0].kind = "stateful".to_owned();
        let err = verify_packet_shape(&packet).expect_err("kind vocabulary must be closed");
        assert!(err.to_string().contains("unknown kind"), "{err}");
    }

    #[test]
    fn corpus_access_audit_rejects_forbidden_paths_and_accepts_unlocked_paths() {
        let scratch = liminal_scratch::ScratchDir::new("haq-corpus-audit").expect("scratch");
        let root: &Utf8Path = &scratch;
        let packet = packet_from_repo();
        let targets = packet
            .generated
            .iter()
            .flat_map(|family| family.fuzz_targets.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut rows = Vec::new();
        for target in &targets {
            let manifest = format!("conformance/haqp/evidence/access/{target}.paths");
            let path = root.join(&manifest);
            fs::create_dir_all(path.parent().expect("manifest parent")).expect("mkdir");
            fs::write(&path, format!("/workspace/fuzz/corpus/{target}\n")).expect("write");
            rows.push(CorpusAccessAuditTarget {
                target: target.clone(),
                manifest,
                manifest_blake3: blake3::hash(&fs::read(&path).expect("read"))
                    .to_hex()
                    .to_string(),
            });
        }
        let audit = CorpusAccessAudit {
            schema_version: "haqp-corpus-access-v1".to_owned(),
            tracer: "strace-open-paths".to_owned(),
            targets: rows,
        };
        let audit_path = root.join("conformance/haqp/evidence/corpus-access.json");
        fs::write(&audit_path, serde_json::to_vec(&audit).expect("serialize")).expect("write");
        verify_corpus_access_audit(root, &packet).expect("unlocked corpus paths are accepted");

        let first = audit.targets[0].clone();
        let forbidden_path = root.join(&first.manifest);
        fs::write(
            &forbidden_path,
            "/workspace/conformance/corpora/heldout/secret\n",
        )
        .expect("write forbidden path");
        let mut doctored = audit;
        doctored.targets[0].manifest_blake3 =
            blake3::hash(&fs::read(&forbidden_path).expect("read"))
                .to_hex()
                .to_string();
        fs::write(
            &audit_path,
            serde_json::to_vec(&doctored).expect("serialize"),
        )
        .expect("write doctored audit");
        verify_corpus_access_audit(root, &packet)
            .expect_err("held-out corpus access must fail closed");
    }

    #[test]
    fn campaign_clock_allows_one_breach_but_blocks_two() {
        let scratch = liminal_scratch::ScratchDir::new("haq-campaign-clock").expect("scratch");
        let root: &Utf8Path = &scratch;
        let path = root.join("conformance/haqp/evidence/campaign.json");
        fs::create_dir_all(path.parent().expect("clock parent")).expect("mkdir");
        let clock = CampaignClock {
            schema_version: "haqp-campaign-clock-v1".to_owned(),
            reference_machine: "test-host".to_owned(),
            runs: vec![CampaignRun {
                id: "run-1".to_owned(),
                elapsed_s: 8 * 60 * 60 + 1,
                clean: true,
                result: "pass".to_owned(),
            }],
        };
        fs::write(&path, serde_json::to_vec(&clock).expect("serialize")).expect("write");
        verify_campaign_clock(root).expect("one retained breach is residual risk");
        let mut blocked = clock;
        blocked.runs.push(CampaignRun {
            id: "run-2".to_owned(),
            elapsed_s: 8 * 60 * 60 + 1,
            clean: true,
            result: "pass".to_owned(),
        });
        fs::write(&path, serde_json::to_vec(&blocked).expect("serialize")).expect("write");
        verify_campaign_clock(root).expect_err("two clean breaches block ratification");
    }

    /// `count > 16` — kills `>` -> `>=`: exactly 16 is the documented maximum.
    #[test]
    fn an_operator_supplying_exactly_sixteen_mutants_is_allowed() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        let operator = packet.mutants[0].operator.clone();
        // Exactly 16 on the target operator; the rest spread round-robin over
        // the others so no OTHER operator breaches the same ceiling and masks
        // the case under test.
        //
        // The first version built its fixture with a running counter and then
        // BRANCHED on where it landed, so whichever side it hit was the only
        // side asserted — a conditional test proves whichever case it happened
        // to take, and this bound stayed unpinned either way.
        let others: Vec<String> = packet
            .mutants
            .iter()
            .map(|m| m.operator.clone())
            .filter(|op| *op != operator)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(
            others.len() * 16 >= packet.mutants.len() - 16,
            "the packet must declare enough operators to spread the remainder"
        );
        for (index, mutant) in packet.mutants.iter_mut().enumerate() {
            mutant.operator = if index < 16 {
                operator.clone()
            } else {
                others[(index - 16) % others.len()].clone()
            };
        }
        verify_mutant_inventory(&packet).expect("exactly 16 is at the ceiling, not above it");

        packet.mutants[16].operator = operator;
        verify_mutant_inventory(&packet).expect_err("17 must be refused");
    }

    /// `discards * 100 > attempts` — the 1% discard ceiling. Kills `>` -> `>=`,
    /// `*` -> `+`/`/`. Exactly 1% is allowed; a hair over is not.
    #[test]
    fn the_discard_ceiling_allows_exactly_one_percent() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        // `accepted` has its own >=100,000 floor, so the straddle is built
        // around it: at 100,000 accepted, 1,010 discards is just under the 1%
        // ceiling (101,000 <= 101,010) and 1,011 is just over (101,100 >
        // 101,011).
        for family in &mut packet.generated {
            family.accepted = 100_000;
            family.discards = 1_010;
            family.attempts = 101_010;
        }
        verify_generated_inventory(&packet).expect("1,010 discards is at the ceiling");

        for family in &mut packet.generated {
            family.discards = 1_011;
            family.attempts = 101_011;
        }
        verify_generated_inventory(&packet).expect_err("1,011 discards is above the ceiling");
    }

    /// `seed_categories.len() < 16` — kills `<` -> `>`.
    #[test]
    fn the_seed_category_floor_is_inclusive_at_16() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        for family in &mut packet.generated {
            family.seed_categories.truncate(16);
        }
        verify_generated_inventory(&packet).expect("exactly 16 seed categories is allowed");

        for family in &mut packet.generated {
            family.seed_categories.truncate(15);
        }
        verify_generated_inventory(&packet).expect_err("15 seed categories is below the floor");
    }

    /// `!row.before || !row.after` — kills `||` -> `&&`, which would accept a
    /// boundary injected on only one side of the crash point.
    #[test]
    fn a_crash_boundary_injected_on_only_one_side_is_refused() {
        for (before, after) in [(true, false), (false, true)] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundaries[0].before = before;
            packet.crash_boundaries[0].after = after;
            verify_crash_boundary_inventory(&packet)
                .expect_err("a boundary must be injected on BOTH sides");
        }
    }

    /// M17.5 F-31 / P1-A08: the scenarios block was written by the fault lane
    /// and read by nothing.
    #[test]
    fn crash_scenarios_are_verified_not_merely_carried() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/crash.json"))
            .expect("committed crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse");
        let declared: BTreeSet<String> = recorded
            .boundaries
            .iter()
            .map(|row| row.boundary.clone())
            .collect();
        verify_crash_rows(&recorded, &declared).expect("the committed matrix must pass");

        let mut failed = recorded.clone();
        failed.scenarios[0].result = "fail".to_owned();
        verify_crash_rows(&failed, &declared)
            .expect_err("a scenario reporting failure must not pass");

        let mut non_idempotent = recorded.clone();
        non_idempotent.boundaries[0].recovery_pairs[0].second_recovery_digest = "00".repeat(32);
        verify_crash_rows(&non_idempotent, &declared)
            .expect_err("a second recovery digest that changes must be rejected");

        let mut inert = recorded.clone();
        inert.scenarios[0].faults_injected = 0;
        verify_crash_rows(&inert, &declared)
            .expect_err("a scenario that injected nothing exercised nothing");

        let mut stray = recorded.clone();
        stray.scenarios[0]
            .boundaries
            .insert("ilrp/not_registered".to_owned());
        verify_crash_rows(&stray, &declared)
            .expect_err("a scenario may not exercise an unregistered boundary");

        let mut miscounted = recorded.clone();
        miscounted.scenarios[0].faults_injected = 1;
        verify_crash_rows(&miscounted, &declared)
            .expect_err("fewer faults than distinct boundaries is impossible");

        let mut empty = recorded;
        empty.scenarios.clear();
        verify_crash_rows(&empty, &declared).expect_err("an empty fault matrix proves nothing");
    }
}
