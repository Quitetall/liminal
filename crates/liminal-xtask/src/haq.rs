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
    verify_fuzz_evidence(root, &packet)?;
    verify_test_names_exist(root, &packet)?;
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
fn verify_fuzz_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let bytes = std::fs::read(&path)
        .with_context(|| format!("{path}: committed fuzz evidence is required"))?;
    let recorded: Vec<FuzzEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    if recorded.len() != packet.generated.len() {
        anyhow::bail!(
            "fuzz evidence covers {} targets but the packet declares {} families",
            recorded.len(),
            packet.generated.len()
        );
    }
    let total_minutes: u64 = recorded.iter().map(|row| row.seconds / 60).sum();
    if total_minutes < 150 {
        anyhow::bail!("recorded fuzz budget {total_minutes} target-minutes is below 150");
    }
    for row in &recorded {
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
        if family.fuzz_minutes < 31 {
            anyhow::bail!("{} fuzz minutes below 31", family.family);
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
            if family
                .evidence_hash
                .as_ref()
                .is_none_or(|hash| hash.len() != 64)
            {
                anyhow::bail!(
                    "{} claims pass without a 64-hex evidence hash",
                    family.family
                );
            }
        }
    }
    let total_minutes: u64 = packet.generated.iter().map(|row| row.fuzz_minutes).sum();
    if total_minutes < 155 {
        anyhow::bail!("total fuzz minutes below 155: {total_minutes}");
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
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct FuzzEvidence {
    target: String,
    seconds: u64,
    exit_code: i32,
    execs: u64,
    artifacts: u64,
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
    unresolved_verified_findings: u64,
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
    let path = root.join("target/haqp/canaries.json");
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
            "fuzz minutes below 31"
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
    /// In-domain and every metamorphic relation held.
    Accepted,
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
    Ok(Case::Accepted)
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
        id: liminal_id::NodeId::new(),
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
    Ok(Case::Accepted)
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
    if let MergeOutcome::Disjoint { merged } = three_way(&base, &ours, &base) {
        anyhow::ensure!(
            merged.split_whitespace().eq(ours.split_whitespace()),
            "identity merge dropped content: {ours:?} -> {merged:?}"
        );
    }

    let forward = three_way(&base, &ours, &theirs);
    let swapped = three_way(&base, &theirs, &ours);
    let disjoint = |o: &MergeOutcome| matches!(o, MergeOutcome::Disjoint { .. });
    anyhow::ensure!(
        disjoint(&forward) == disjoint(&swapped),
        "merge disjointness is not symmetric under swapping sides"
    );
    Ok(Case::Accepted)
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
        .map(|_| liminal_id::RepairStepId::new())
        .collect();
    let steps = ids
        .iter()
        .map(|id| {
            (
                *id,
                ProposedMutation {
                    id: *id,
                    subject: liminal_id::JurisdictionSubject::Node(liminal_id::NodeId::new()),
                    operation: RepairOperation::WriteFile {
                        path: liminal_id::PathId("generated.md".into()),
                        contents: rng.word().into_bytes(),
                    },
                    expected_prestate: StatePredicate::Any,
                    expected_poststate: StatePredicate::Any,
                    idempotency_key: liminal_id::IdempotencyKey::new(),
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
        id: liminal_id::RepairId::new(),
        basis: liminal_revision::WorkspaceBasis {
            transaction: liminal_id::TransactionId::new(),
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
    Ok(Case::Accepted)
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
    deps.record(extra);
    for key in &expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "recording another read un-invalidated {key:?}"
        );
    }
    Ok(Case::Accepted)
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
    let path = root.join("target/haqp/generated.json");
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
                Case::Accepted => accepted += 1,
                Case::Discarded => discards += 1,
            }
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

#[derive(Debug, serde::Serialize)]
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
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
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
}
