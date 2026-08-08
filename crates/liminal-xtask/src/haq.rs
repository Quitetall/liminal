//! HAQP-1 packet verification.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use anyhow::{Context, Result};
use camino::Utf8Path;
use liminal_format::Formatter;
use serde::Deserialize;

/// Verify committed HAQP inventories, packet status, and crash-boundary registry.
pub fn verify_inventory_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_packet_statuses(&packet, inventory_statuses())?;
    verify_markdown_surface(root, &packet)?;
    Ok(())
}

/// Verify completed HAQP qualification evidence. Planned or NOT_RUN packet rows
/// fail here; this is the command used by the M17 packet gate.
pub fn verify_qualified_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_markdown_surface(root, &packet)?;
    require_eq(
        "qualification_state",
        &packet.qualification_state,
        "complete",
    )?;
    verify_packet_statuses(
        &packet,
        PacketStatusExpectations {
            mutant: "killed",
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
    verify_fuzz_evidence(root)?;
    verify_crash_evidence(root, &packet)?;
    verify_qualification_stage(&packet)?;
    if packet.qualification_stage == "1b" {
        verify_mutant_killing_tests(root, &packet)?;
    }
    verify_canary_evidence(root, &packet)?;
    verify_generated_evidence(root, &packet)?;
    verify_review_evidence(root, &packet)?;
    verify_test_names_exist(root, &packet)?;
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
    let bytes = std::fs::read(&path).with_context(|| {
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
    let bytes = std::fs::read(&path).with_context(|| {
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
    let bytes = std::fs::read(&path).with_context(|| {
        format!("{path}: committed generated evidence is required; run `just haq-generated`")
    })?;
    let recorded: Vec<GeneratedEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    verify_generated_rows(&packet.generated, &recorded)
}

fn verify_generated_rows(declared: &[Generated], recorded: &[GeneratedEvidence]) -> Result<()> {
    let by_family: BTreeMap<&str, &GeneratedEvidence> = recorded
        .iter()
        .map(|row| (row.family.as_str(), row))
        .collect();
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
    for row in &recorded.boundaries {
        if row.occurrences_exercised == 0 {
            anyhow::bail!("crash boundary {} was never exercised", row.boundary);
        }
        for (field, value) in [
            ("result", &row.result),
            ("double_recovery", &row.double_recovery),
        ] {
            if value != "pass" {
                anyhow::bail!("crash boundary {} reports {field} {value:?}", row.boundary);
            }
        }
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
        let bytes = std::fs::read(&path).with_context(|| {
            format!(
                "{path}: {} claims pass but its record is missing",
                review.reviewer
            )
        })?;
        let record: ReviewRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
        verify_review_record(review, &record)?;
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
fn verify_review_record(review: &Review, record: &ReviewRecord) -> Result<()> {
    let who = &review.reviewer;
    if record.attempts.len() as u64 != review.attempts {
        anyhow::bail!(
            "{who} declares {} attempts but its record contains {}",
            review.attempts,
            record.attempts.len()
        );
    }
    // P1-A05: the attempts must be attempt RECORDS, not array padding.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for attempt in &record.attempts {
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
    }
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
    if record.findings.len() != review.findings.len() {
        anyhow::bail!(
            "{who} lists {} findings but its record contains {}",
            review.findings.len(),
            record.findings.len()
        );
    }
    // P1-A07: blindness is a property of how the pass was RUN. The runner emits
    // this proof; before F-27 nothing read it back.
    if record.blindness_proof.prior_pass_artifact_supplied {
        anyhow::bail!("{who} was shown a prior pass's artifacts; ADR-0020 §6 requires blindness");
    }
    if record.pass == 2 && !record.blindness_proof.pass_two_original_spec_only {
        anyhow::bail!("{who} is pass 2 but did not start from the original spec alone");
    }
    Ok(())
}

/// ADR-0020 §6's independence, checked rather than assumed (M17.5 F-27 / P1-A07).
///
/// The runner records `reviewer.model_family`, `reviewer.backend` and an
/// identity hash precisely so independence is auditable. The qualification
/// verifier never looked at any of them — this session BUILT that evidence and
/// then failed to check it, which is the same mistake as trusting a status
/// string. Two reviews from one family satisfy every other check in this file.
fn verify_reviewer_independence(records: &[(String, ReviewRecord)]) -> Result<()> {
    if records.len() < 2 {
        return Ok(());
    }
    let mut families: BTreeSet<&str> = BTreeSet::new();
    let mut backends: BTreeSet<&str> = BTreeSet::new();
    let mut identities: BTreeSet<&str> = BTreeSet::new();
    for (who, record) in records {
        if record.reviewer.model_family.trim().is_empty() {
            anyhow::bail!("{who}: record names no model family");
        }
        if record.reviewer.identity_hash.trim().is_empty() {
            anyhow::bail!("{who}: record carries no reviewer identity hash");
        }
        families.insert(record.reviewer.model_family.as_str());
        backends.insert(record.reviewer.backend.as_str());
        identities.insert(record.reviewer.identity_hash.as_str());
    }
    for (label, distinct) in [
        ("model families", families.len()),
        ("backends", backends.len()),
        ("reviewer identities", identities.len()),
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
    let claims_kills = packet
        .mutants
        .iter()
        .any(|mutant| mutant.disposition == "killed");
    match packet.qualification_stage.as_str() {
        "1a" if claims_kills => anyhow::bail!(
            "packet declares stage 1a but claims mutant kills; ADR-0021 defers the \
             mutation requirement to 1b at M24, when a killing test can actually run"
        ),
        "1b" if !claims_kills => anyhow::bail!(
            "packet declares stage 1b, which carries ADR-0020 §3's mutation \
             requirement, but no mutant is killed"
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

/// Leaf names of every `#[ignore]`d test in the tree.
fn ignored_test_names(root: &Utf8Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = std::fs::read_to_string(&path)
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
        if trimmed.starts_with("#[ignore") {
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
    let mut sources = String::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                sources.push_str(&text);
                sources.push('\n');
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
        if !sources.contains(&format!("fn {leaf}(")) {
            missing.push(format!("{} -> {}", test.id, test.name));
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

/// The recorded fuzz campaign is too long to re-run per verification (150
/// target-minutes), so its artifact is COMMITTED and the packet must agree
/// with it exactly. Cheap lanes are re-run instead — see `haq verify-full`.
/// The packet is deliberately NOT consulted here, and that is a recorded gap
/// rather than an oversight (M17.5 pass-2 #17, remaining half). The packet
/// declares `generated[*].fuzz_minutes` per FAMILY while this artifact records
/// `seconds` per TARGET, and no mapping between the two is declared anywhere in
/// ADR-0020 or the packet. Inventing one here would be improvising qualification
/// semantics, so the two independent claims about one campaign stay
/// unreconciled until that mapping is decided.
fn verify_fuzz_evidence(root: &Utf8Path) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let bytes = std::fs::read(&path)
        .with_context(|| format!("{path}: committed fuzz evidence is required"))?;
    let recorded: Vec<FuzzEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;

    // M17.5 pass-2 #17: the count was compared against `packet.generated.len()`,
    // which is a category error — fuzz TARGETS are not generated FAMILIES. The
    // two happen to both be 5, so the check passed while comparing unrelated
    // things, and a row naming a target that does not exist was accepted.
    // Bind to the targets that actually exist in the tree instead.
    let mut present = BTreeSet::new();
    let targets_dir = root.join("fuzz/fuzz_targets");
    let entries = std::fs::read_dir(&targets_dir)
        .with_context(|| format!("{targets_dir}: fuzz targets are required to verify the lane"))?;
    for entry in entries.flatten() {
        let Ok(file) = camino::Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if file.extension() == Some("rs")
            && let Some(stem) = file.file_stem()
        {
            present.insert(stem.to_owned());
        }
    }
    verify_fuzz_rows(&recorded, &present)
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
        // 30 minutes, NOT the 31 that `verify_generated_inventory` demands of the
        // packet. That discrepancy is real and is recorded as F-18; this check
        // deliberately follows the ADR rather than the other verifier.
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

/// ADR-0020 §1: the qualification lane runs from ONE fixed clean commit and
/// tree. Without this binding the packet describes no particular state of the
/// repository (pass-2 finding #21).
fn verify_provenance(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let Some(provenance) = &packet.provenance else {
        anyhow::bail!("packet has no provenance block; qualification is unbound to any tree");
    };
    let git = |args: &[&str]| -> Result<String> {
        let out = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()?;
        anyhow::ensure!(out.status.success(), "git {args:?} failed");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    let head = git(&["rev-parse", "HEAD"])?;
    require_eq("provenance.commit", &provenance.commit, &head)?;
    let dirty = git(&["status", "--porcelain"])?;
    anyhow::ensure!(
        dirty.is_empty(),
        "qualification requires a clean tree; {} path(s) are dirty",
        dirty.lines().count()
    );
    let lockfile = std::fs::read(root.join("Cargo.lock"))?;
    let digest = blake3::hash(&lockfile).to_hex().to_string();
    require_eq(
        "provenance.lockfile_blake3",
        &provenance.lockfile_blake3,
        &digest,
    )?;
    Ok(())
}

fn read_packet(root: &Utf8Path) -> Result<Packet> {
    let path = root.join("conformance/haqp/packet.json");
    let bytes = std::fs::read(&path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

fn verify_packet_shape(packet: &Packet) -> Result<()> {
    require_eq(
        "suite_version",
        &packet.suite_version,
        "phase1-haqp1-proposed-v1",
    )?;
    require_eq("status", &packet.status, "proposed")?;
    require_eq("ratification", &packet.ratification, "unratified")?;
    if packet.locked_acceptance_corpora_touched {
        anyhow::bail!("locked_acceptance_corpora_touched must be false");
    }
    if packet.requirements.len() < 24 {
        anyhow::bail!(
            "requirements inventory too small: {}",
            packet.requirements.len()
        );
    }
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
        if requirement.kind == "fault" {
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
        require_eq("mutant.disposition", &mutant.disposition, expected.mutant)?;
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
        *by_family.entry(mutant.family.as_str()).or_default() += 1;
        *by_operator.entry(mutant.operator.as_str()).or_default() += 1;
        if mutant.killing_tests.is_empty() {
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
    }
    Ok(())
}

fn verify_generated_inventory(packet: &Packet) -> Result<()> {
    if packet.generated.len() != 5 {
        anyhow::bail!(
            "generated family count must be 5, got {}",
            packet.generated.len()
        );
    }
    for family in &packet.generated {
        if family.accepted < 100_000 {
            anyhow::bail!("{} accepted cases below 100000", family.family);
        }
        if family.discards * 100 > family.attempts {
            anyhow::bail!("{} discard rate exceeds 1%", family.family);
        }
        // ADR-0020 line 90: one 30-minute sanitizer campaign per family. This
        // demanded 31, and packet.json was populated with 31 to match — so the
        // packet claimed 155 target-minutes while the campaign performed 150
        // (M17.5 F-18). The ADR is canonical; a verifier does not get to
        // redefine the protocol it checks.
        if family.fuzz_minutes < 30 {
            anyhow::bail!("{} fuzz minutes below 30", family.family);
        }
        if family.seed_categories.len() < 16 {
            anyhow::bail!("{} has fewer than 16 seed categories", family.family);
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
    let total_minutes: u64 = packet.generated.iter().map(|row| row.fuzz_minutes).sum();
    // ADR-0020 line 92: "Total fuzz budget is at least 150 target-minutes."
    if total_minutes < 150 {
        anyhow::bail!("total fuzz minutes below 150: {total_minutes}");
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
    let text = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
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
    locked_acceptance_corpora_touched: bool,
    requirements: Vec<Requirement>,
    tests: Vec<Test>,
    mutants: Vec<Mutant>,
    canaries: Vec<Canary>,
    generated: Vec<Generated>,
    crash_boundaries: Vec<CrashBoundary>,
    reviews: Vec<Review>,
    /// ADR-0020 §1 fixed-base binding. Absent until the qualification lane
    /// runs from one clean tree.
    #[serde(default)]
    provenance: Option<Provenance>,
}

/// The fixed base a qualification run was taken from.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Provenance {
    commit: String,
    lockfile_blake3: String,
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
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct Requirement {
    id: String,
    kind: String,
    source: String,
    critical: bool,
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
    fuzz_minutes: u64,
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
struct ReviewRecord {
    pass: u8,
    reviewer: ReviewRecordReviewer,
    attempts: Vec<ReviewRecordAttempt>,
    #[serde(default)]
    findings: Vec<serde_json::Value>,
    unresolved_verified_findings: u64,
    result: String,
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
    classification: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct ReviewRecordBlindness {
    prior_pass_artifact_supplied: bool,
    #[serde(default)]
    pass_two_original_spec_only: bool,
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
    scenarios: Vec<serde_json::Value>,
    boundaries: Vec<CrashEvidenceBoundary>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidenceBoundary {
    boundary: String,
    occurrences_exercised: u64,
    double_recovery: String,
    staged_residue: String,
    result: String,
}

/// Execute every declared disposable gate canary without touching the working
/// tree.
pub fn run_canaries_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_packet(root)?;
    verify_inventory_repo(root)?;
    let markdown_path = root.join("docs/execution/phase1-suite-review.md");
    let markdown = std::fs::read_to_string(&markdown_path)?;
    let records = run_canary_suite(&baseline, &markdown)?;
    let bytes = serde_json::to_vec_pretty(&records)?;
    let path = root.join("conformance/haqp/evidence/canaries.json");
    std::fs::create_dir_all(path.parent().expect("evidence parent"))?;
    std::fs::write(&path, &bytes)?;
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
        if !observed.replace('"', "").contains(&row.expected_failure) {
            anyhow::bail!(
                "{}: expected failure {:?}, observed {observed:?}",
                row.id,
                row.expected_failure
            );
        }
        records.push(CanaryEvidence {
            id: row.id.clone(),
            violation: row.violation.clone(),
            expected_failure: row.expected_failure.clone(),
            observed_failure: observed,
            caught: true,
        });
    }
    Ok(records)
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
            packet.generated[0].fuzz_minutes = 0;
            "fuzz minutes below 30"
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
    Discarded,
}

/// Family 0 — source/CST/formatting. Metamorphic relations: **lossless
/// emit** (the CST must reproduce its input byte-for-byte) and **format
/// idempotence**, both compared over bytes rather than parsed values.
fn case_source_cst(rng: &mut Rng) -> Result<Case> {
    let source = rng.word();
    // Out of domain: the compact surface has no all-whitespace document.
    if source.trim().is_empty() {
        return Ok(Case::Discarded);
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
        return Ok(Case::Discarded);
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
        return Ok(Case::Discarded);
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
        // Cyclic plans are correctly refused; not acceptance evidence.
        return Ok(Case::Discarded);
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
        return Ok(Case::Discarded);
    }

    for key in &expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "a recorded read did not invalidate: {key:?}"
        );
    }
    let unrelated = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
        format!("unread/{}", rng.word()).into(),
    ));
    if !expected.contains(&unrelated) {
        anyhow::ensure!(
            !deps.invalidated_by(&unrelated),
            "an unread key invalidated the computation: {unrelated:?}"
        );
    }
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
    let bytes = serde_json::to_vec_pretty(&evidence)?;
    let path = root.join("conformance/haqp/evidence/generated.json");
    std::fs::create_dir_all(path.parent().expect("evidence parent"))?;
    std::fs::write(&path, &bytes)?;
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
fn generate_evidence(cases: u64) -> Result<(Vec<GeneratedEvidence>, Vec<u64>)> {
    if cases == 0 {
        anyhow::bail!("generated case count must be positive");
    }
    if cases > 10_000_000 {
        anyhow::bail!("generated case count exceeds safety limit: {cases} > 10000000");
    }
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
                Case::Discarded => {
                    discards += 1;
                    digest.update(b"D");
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
    /// The mutation that was actually performed, checked against the packet's
    /// own prose (M17.5 pass-2 #22).
    violation: String,
    expected_failure: String,
    observed_failure: String,
    caught: bool,
}

/// One family's generated-evidence row.
///
/// Every field here must be a function of the seed and the code under test and
/// of NOTHING ELSE. Wall-clock timing used to live here as `elapsed_ms`, which
/// made the artifact unhashable — two runs of the same seed differed (50→57,
/// 94→110, 12→15 ms), so the artifact could never be compared against a
/// recorded digest (M17.5 pass-2 #18). Timing is operational telemetry, not
/// evidence; it is printed to stdout instead.
#[derive(Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> camino::Utf8PathBuf {
        camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
        packet.requirements.push(Requirement {
            id: "P1-R900".to_owned(),
            kind: "law".to_owned(),
            source: "v4 §112".to_owned(),
            critical: false,
        });
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
        let markdown = std::fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let rows: Vec<FuzzEvidence> =
            serde_json::from_slice(&bytes).expect("every recorded field must be declared");
        assert_eq!(rows.len(), 5, "five targets");
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();

        // The honest artifact must PASS, or the rejection below proves nothing.
        // This assertion used to expect failure, because the committed evidence
        // still carried F-09's crash; the campaign of 2026-07-30 is clean
        // (5 targets, 150 target-minutes, 0 crashes, 0 artifacts), so the
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/crash.json"))
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
            attack_class: "vacuity".to_owned(),
            target: "laws.rs".to_owned(),
            attempt: "did a thing".to_owned(),
            observed_result: "saw a thing".to_owned(),
            classification: "caught_violation".to_owned(),
        }
    }

    fn review_record(pass: u8, family: &str, backend: &str) -> ReviewRecord {
        ReviewRecord {
            pass,
            reviewer: ReviewRecordReviewer {
                model_family: family.to_owned(),
                identity_hash: format!("hash-of-{family}"),
                backend: backend.to_owned(),
            },
            attempts: (0..12).map(|i| attempt(&format!("A{i}"))).collect(),
            findings: Vec::new(),
            unresolved_verified_findings: 0,
            result: "pass".to_owned(),
            blindness_proof: ReviewRecordBlindness {
                prior_pass_artifact_supplied: false,
                pass_two_original_spec_only: pass == 2,
            },
        }
    }

    fn review_row() -> Review {
        Review {
            reviewer: "r".to_owned(),
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
        verify_review_record(&review_row(), &review_record(1, "openai", "codex"))
            .expect("a complete record must pass, or every check below is vacuous");
    }

    /// P1-A05: array padding is not a set of attempts.
    #[test]
    fn review_record_rejects_empty_attempt_records() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[3].observed_result = "   ".to_owned();
        let err = verify_review_record(&review_row(), &record)
            .expect_err("an attempt with no observed result is not an attempt");
        assert!(err.to_string().contains("empty observed_result"), "{err}");
    }

    #[test]
    fn review_record_rejects_unknown_classifications_and_duplicate_ids() {
        let mut unknown = review_record(1, "openai", "codex");
        unknown.attempts[0].classification = "inconclusive".to_owned();
        verify_review_record(&review_row(), &unknown)
            .expect_err("a classification outside the declared set must be rejected");
        let mut duplicated = review_record(1, "openai", "codex");
        duplicated.attempts[1].id = duplicated.attempts[0].id.clone();
        verify_review_record(&review_row(), &duplicated)
            .expect_err("twelve attempts must be twelve DISTINCT attempts");
    }

    /// P1-A06: the packet may not summarize the record more kindly than the
    /// record summarizes itself.
    #[test]
    fn review_record_rejects_a_packet_row_that_contradicts_it() {
        let mut failed = review_record(1, "openai", "codex");
        failed.result = "fail".to_owned();
        let err = verify_review_record(&review_row(), &failed)
            .expect_err("a pass row over a failing record must be rejected");
        assert!(err.to_string().contains("record says"), "{err}");

        let mut unresolved = review_record(1, "openai", "codex");
        unresolved.unresolved_verified_findings = 9;
        verify_review_record(&review_row(), &unresolved)
            .expect_err("a row claiming zero unresolved findings over a record counting nine");
    }

    /// P1-A07: blindness is a property of the run, and it is recorded.
    #[test]
    fn review_record_rejects_a_pass_that_saw_prior_artifacts() {
        let mut leaked = review_record(1, "openai", "codex");
        leaked.blindness_proof.prior_pass_artifact_supplied = true;
        verify_review_record(&review_row(), &leaked)
            .expect_err("a reviewer shown the prior pass is not blind");

        let mut informed = review_record(2, "xiaomi", "lamu");
        informed.blindness_proof.pass_two_original_spec_only = false;
        verify_review_record(&review_row(), &informed)
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
            fuzz_minutes: 30,
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
            violation: "set status to ratified".to_owned(),
            expected_failure: "status: expected proposed".to_owned(),
            observed_failure: "status: expected proposed, found ratified".to_owned(),
            caught: true,
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
        let bytes = std::fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
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

        // P1-M001's killing tests are P1-T09 and P1-T01; T01 is
        // `laws::formatter_idempotence_law_holds`, quarantined by AM-17.2.
        packet.mutants[0].disposition = "killed".to_owned();
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
}
