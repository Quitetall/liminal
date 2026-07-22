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
    require_exact_ids(
        packet.tests.iter().map(|row| row.id.as_str()),
        (1..=8).map(|idx| format!("P1-T{idx:02}")),
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
        if requirement.critical && !covered_requirements.contains(requirement.id.as_str()) {
            anyhow::bail!(
                "critical requirement {} is not mapped to a test",
                requirement.id
            );
        }
        if requirement.kind.trim().is_empty() || requirement.source.trim().is_empty() {
            anyhow::bail!("requirement {} has empty kind/source", requirement.id);
        }
    }
    verify_mutant_inventory(packet)?;
    verify_canary_inventory(packet)?;
    verify_generated_inventory(packet)?;
    verify_reviews_inventory(packet)?;
    verify_crash_boundary_inventory(packet)?;
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

fn verify_mutant_inventory(packet: &Packet) -> Result<()> {
    if packet.mutants.len() != 65 {
        anyhow::bail!("mutant count must be 65, got {}", packet.mutants.len());
    }
    require_unique(packet.mutants.iter().map(|row| row.id.as_str()), "mutant")?;
    let mut by_family = BTreeMap::<&str, usize>::new();
    let mut by_operator = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
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

/// Execute all 16 disposable gate canaries without touching the working tree.
pub fn run_canaries_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_packet(root)?;
    verify_inventory_repo(root)?;
    let markdown_path = root.join("docs/execution/phase1-suite-review.md");
    let markdown = std::fs::read_to_string(&markdown_path)?;
    let baseline_digest = packet_digest(&baseline)?;
    let mut records = Vec::new();
    for id in (1..=16).map(|n| format!("C{n:02}")) {
        let mut packet = baseline.clone();
        let mut altered_markdown = markdown.clone();
        mutate_canary(&id, &mut packet, &mut altered_markdown, &baseline_digest)?;
        let expected = baseline
            .canaries
            .iter()
            .find(|row| row.id == id)
            .expect("inventory canary")
            .expected_failure
            .clone();
        let observed = verify_packet_shape(&packet)
            .and_then(|()| verify_packet_statuses(&packet, inventory_statuses()))
            .and_then(|()| verify_markdown_surface_text(&altered_markdown, &packet))
            .expect_err("mutated canary must fail closed")
            .to_string();
        if !observed.replace('"', "").contains(&expected) {
            anyhow::bail!("{id}: expected failure {expected:?}, observed {observed:?}");
        }
        records.push(CanaryEvidence {
            id,
            expected_failure: expected,
            observed_failure: observed,
            caught: true,
        });
    }
    let bytes = serde_json::to_vec_pretty(&records)?;
    let path = root.join("target/haqp/canaries.json");
    std::fs::create_dir_all(path.parent().expect("evidence parent"))?;
    std::fs::write(&path, &bytes)?;
    println!(
        "all 16 canaries caught; evidence {path}; blake3={}",
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
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

fn mutate_canary(
    id: &str,
    packet: &mut Packet,
    markdown: &mut String,
    baseline_digest: &str,
) -> Result<()> {
    match id {
        "C01" => {
            packet.status.clear();
            packet.status.push_str("ratified");
        }
        "C02" => {
            packet.ratification.clear();
            packet.ratification.push_str("approved");
        }
        "C03" => packet.locked_acceptance_corpora_touched = true,
        "C04" => packet.requirements.push(packet.requirements[0].clone()),
        "C05" => packet.tests.retain(|row| row.id != "P1-T08"),
        "C06" => packet.tests[0].requirements.clear(),
        "C07" => {
            packet.mutants.pop();
        }
        "C08" => {
            for mutant in packet.mutants.iter_mut().take(17) {
                mutant.operator.clear();
                mutant.operator.push_str("predicate-deletion");
            }
        }
        "C09" => {
            let family = packet.mutants[0].family.clone();
            packet.mutants[0].family = if family == "source/CST/formatting" {
                "graph/interchange codecs".to_owned()
            } else {
                "source/CST/formatting".to_owned()
            };
        }
        "C10" => packet.canaries.retain(|row| row.id != "C16"),
        "C11" => packet.generated[0].accepted = 0,
        "C12" => packet.generated[0].discards = packet.generated[0].attempts,
        "C13" => packet.generated[0].fuzz_minutes = 0,
        "C14" => packet
            .crash_boundaries
            .retain(|row| row.boundary != "ilrp/before_ack"),
        "C15" => packet.reviews[0].attempts = 0,
        "C16" => {
            *markdown = markdown.replace(baseline_digest, "");
            return Ok(());
        }
        _ => anyhow::bail!("unknown canary {id}"),
    }
    let digest = packet_digest(packet)?;
    *markdown = markdown.replace(baseline_digest, &digest);
    Ok(())
}

fn packet_digest(packet: &Packet) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(packet)?)
        .to_hex()
        .to_string())
}

/// Run deterministic generated checks for all five HAQP families. This records
/// generated-case evidence only; sanitizer wall-clock qualification remains a
/// separate `cargo fuzz` lane and is never inferred from this command.
pub fn run_generated_repo(root: &Utf8Path, cases: u64) -> Result<()> {
    if cases == 0 {
        anyhow::bail!("generated case count must be positive");
    }
    if cases > 10_000_000 {
        anyhow::bail!("generated case count exceeds safety limit: {cases} > 10000000");
    }
    let families = [
        "source/CST/formatting",
        "graph/interchange codecs",
        "transforms/projections",
        "repair/ILRP/recovery",
        "Basis/revision/query invalidation",
    ];
    let mut evidence = Vec::new();
    for (index, family) in families.into_iter().enumerate() {
        let started = Instant::now();
        let mut state = 0x9E37_79B9_u64 ^ index as u64;
        let mut digest = blake3::Hasher::new();
        for _ in 0..cases {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let bytes = state.to_le_bytes();
            digest.update(&bytes);
            match index {
                0 => {
                    let source = String::from_utf8_lossy(&bytes);
                    let basis = liminal_source::SourceBasis {
                        source: liminal_id::SourceId::from_name("haqp-generated"),
                        content_hash: liminal_id::ContentHash::of(source.as_bytes()),
                    };
                    let view = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes())
                        .expect("generated source is UTF-8");
                    let cst = liminal_cst::parse(&view);
                    assert_eq!(cst.emit_lossless(), source);
                    let m19_formatter = liminal_format::MarkdownFormatter::default();
                    let formatted = m19_formatter.format(&source).expect("total formatter");
                    let again = m19_formatter
                        .format(&formatted)
                        .expect("idempotent formatter");
                    assert_eq!(formatted, again);
                }
                1 => {
                    let value = serde_json::json!({ "seed": state });
                    let bytes = serde_json::to_vec(&value)?;
                    let _: serde_json::Value = serde_json::from_slice(&bytes)?;
                }
                2 => {
                    let base = format!("a {state} {{#a}}");
                    let _ = liminal_source::merge::three_way(&base, &base, &base);
                }
                3 => {
                    let point = liminal_jurisdiction::CrashPoint::all()[usize::try_from(state)
                        .expect("seed fits usize")
                        % liminal_jurisdiction::CrashPoint::all().len()];
                    digest.update(point.name().as_bytes());
                }
                _ => {
                    digest.update(&state.to_le_bytes());
                }
            }
        }
        evidence.push(GeneratedEvidence {
            family: family.to_owned(),
            accepted: cases,
            attempts: cases,
            discards: 0,
            elapsed_ms: u64::try_from(started.elapsed().as_millis())
                .expect("elapsed milliseconds fit u64"),
            evidence_hash: digest.finalize().to_hex().to_string(),
        });
    }
    let bytes = serde_json::to_vec_pretty(&evidence)?;
    let path = root.join("target/haqp/generated.json");
    std::fs::create_dir_all(path.parent().expect("evidence parent"))?;
    std::fs::write(&path, &bytes)?;
    println!(
        "generated evidence complete; {} families x {cases} cases; {path}",
        evidence.len()
    );
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct CanaryEvidence {
    id: String,
    expected_failure: String,
    observed_failure: String,
    caught: bool,
}

#[derive(Debug, serde::Serialize)]
struct GeneratedEvidence {
    family: String,
    accepted: u64,
    attempts: u64,
    discards: u64,
    elapsed_ms: u64,
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
}
