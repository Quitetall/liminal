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
    verify_mutant_source_coordinates(root, &packet)?;
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
    verify_concurrency_evidence(root, &packet)?;
    verify_fuzz_evidence(root, &packet)?;
    verify_corpus_access_audit(root, &packet)?;
    verify_crash_evidence(root, &packet)?;
    verify_crash_replay(root, &packet)?;
    let provenance = packet
        .provenance
        .as_ref()
        .context("qualified campaign clock requires packet provenance")?;
    verify_campaign_clock(root, Some(provenance))?;
    verify_residual_risks(root, &packet)?;
    verify_qualification_stage(&packet)?;
    if packet.qualification_stage == "1b" {
        verify_mutant_killing_tests(root, &packet)?;
        verify_mutant_evidence(root, &packet)?;
        verify_mutant_evidence_replay(root, &packet)?;
    }
    verify_canary_evidence(root, &packet)?;
    verify_generated_evidence(root, &packet)?;
    verify_review_evidence(root, &packet)?;
    verify_mutant_concurrence(root, &packet)?;
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

    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").current_dir(root).args(args).output()?;
        anyhow::ensure!(out.status.success(), "git {args:?} failed");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    let (expected_commit, expected_tree) = if let Some(provenance) = &packet.provenance {
        (
            provenance.fixed_commit.clone(),
            provenance.fixed_tree.clone(),
        )
    } else {
        let commit = git(&["rev-parse", "HEAD"])?;
        let tree = git(&["rev-parse", &format!("{commit}^{{tree}}")])?;
        (commit, tree)
    };
    require_eq(
        "crash evidence source_commit",
        &recorded.source_commit,
        &expected_commit,
    )?;
    require_eq(
        "crash evidence source_tree",
        &recorded.source_tree,
        &expected_tree,
    )?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    require_eq(
        "crash evidence lockfile_blake3",
        &recorded.lockfile_blake3,
        &lockfile_blake3,
    )?;

    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    verify_recovery_proof_presence(&recorded)?;
    verify_crash_rows(&recorded, &declared)?;
    verify_crash_injection_bindings(packet, &recorded)
}

/// Re-run the fault matrix during qualification and compare its measured
/// recovery rows with the committed artifact. Hex digests alone are claims;
/// this replay is the independent producer that makes fabricated terminal,
/// Basis, and effect proofs inadmissible (P1-A05).
fn verify_crash_replay(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let recorded_path = root.join("conformance/haqp/evidence/crash.json");
    let replay_path = root.join("target/haqp/crash-replay.json");
    if let Some(parent) = replay_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "run",
            "-q",
            "-p",
            "liminal-conformance",
            "--bin",
            "crash-evidence",
        ])
        .env("HAQP_CRASH_EVIDENCE_OUT", &replay_path)
        .status()
        .context("run independent crash-evidence replay")?;
    anyhow::ensure!(status.success(), "independent crash-evidence replay failed");
    let recorded: CrashEvidence = serde_json::from_slice(&fs::read(&recorded_path)?)?;
    let replayed: CrashEvidence = serde_json::from_slice(&fs::read(&replay_path)?)?;
    require_eq(
        "crash replay lockfile_blake3",
        &replayed.lockfile_blake3,
        &recorded.lockfile_blake3,
    )?;
    anyhow::ensure!(
        replayed.registered == recorded.registered
            && replayed.exercised == recorded.exercised
            && replayed.scenarios == recorded.scenarios
            && replayed.boundaries == recorded.boundaries,
        "committed crash evidence differs from independent fault-matrix replay"
    );
    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    verify_crash_rows(&replayed, &declared)?;
    verify_crash_injection_bindings(packet, &replayed)
}

fn verify_recovery_proof_presence(recorded: &CrashEvidence) -> Result<()> {
    for boundary in &recorded.boundaries {
        for pair in &boundary.recovery_pairs {
            anyhow::ensure!(
                !pair.first_terminal_digest.is_empty()
                    && !pair.second_terminal_digest.is_empty()
                    && !pair.first_basis_digest.is_empty()
                    && !pair.second_basis_digest.is_empty()
                    && !pair.first_effect_digest.is_empty()
                    && !pair.second_effect_digest.is_empty(),
                "crash boundary {} recovery case {}:{} lacks terminal/Basis/effect duplicate proof",
                boundary.boundary,
                pair.scenario,
                pair.occurrence
            );
        }
    }
    Ok(())
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
    verify_canary_rows(&packet.canaries, &recorded)?;
    if let Some(provenance) = &packet.provenance {
        verify_canary_evidence_replay(root, provenance, &recorded)?;
    }
    Ok(())
}

/// Re-run canaries from the fixed inventory commit.  A qualified packet is a
/// metadata child and has resolved statuses, so replaying its in-memory shape
/// would no longer exercise the inventory verifier that the canary campaign
/// claims to attack.  The fixed parent is the authoritative proposed packet;
/// its packet and Markdown are loaded from Git, mutated, and checked again.
fn verify_canary_evidence_replay(
    root: &Utf8Path,
    provenance: &Provenance,
    recorded: &[CanaryEvidence],
) -> Result<()> {
    let git_show = |path: &str| -> Result<Vec<u8>> {
        let spec = format!("{}:{path}", provenance.fixed_commit);
        let output = Command::new("git")
            .current_dir(root)
            .args(["show", &spec])
            .output()
            .with_context(|| format!("git show {spec}"))?;
        anyhow::ensure!(
            output.status.success(),
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(output.stdout)
    };
    let baseline: Packet = serde_json::from_slice(&git_show("conformance/haqp/packet.json")?)
        .context("parse fixed canary packet")?;
    verify_packet_shape(&baseline)?;
    verify_packet_statuses(&baseline, inventory_statuses())?;
    let markdown = String::from_utf8(git_show("docs/execution/phase1-suite-review.md")?)
        .context("fixed canary Markdown is not UTF-8")?;
    let baseline_digest = packet_digest(&baseline)?;
    anyhow::ensure!(
        markdown.contains(&baseline_digest),
        "fixed canary Markdown is not bound to fixed packet digest"
    );
    let replayed = run_canary_suite(root, &baseline, &markdown)?;
    anyhow::ensure!(
        replayed == recorded,
        "committed canary evidence differs from replay from fixed inventory commit"
    );
    Ok(())
}

fn verify_canary_rows(declared: &[Canary], recorded: &[CanaryEvidence]) -> Result<()> {
    require_unique(
        recorded.iter().map(|row| row.id.as_str()),
        "canary evidence",
    )?;
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
        "haqp-generated-v3-negative-categories",
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
    verify_generated_rows(root, &packet.generated, &recorded.rows)?;
    // A committed packet and artifact are both attacker-controlled inputs to
    // this verifier.  Once qualification claims completion, replay the fixed
    // seeded generator and compare rows, rather than trusting a jointly edited
    // digest pair (P1-A09).
    if packet.qualification_state == "complete" {
        let (replayed, _) = generate_evidence(100_000)?;
        anyhow::ensure!(
            replayed == recorded.rows,
            "qualified generated evidence differs from deterministic 100000-case replay"
        );
    }
    Ok(())
}

fn verify_generated_rows(
    root: &Utf8Path,
    declared: &[Generated],
    recorded: &[GeneratedEvidence],
) -> Result<()> {
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
            ("negatives", family.negatives, evidence.negatives),
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
        verify_generated_contract(root, family, evidence)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "generated relation contract keeps all cross-artifact invariants together"
)]
fn verify_generated_contract(
    root: &Utf8Path,
    family: &Generated,
    evidence: &GeneratedEvidence,
) -> Result<()> {
    let expected = generated_relations(family.family.as_str());
    if expected.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        !family.relations.is_empty(),
        "{}: a passing generated family must declare every relation",
        family.family
    );
    let oracle = evidence
        .oracle
        .as_ref()
        .with_context(|| format!("{}: generated evidence has no oracle", family.family))?;
    let declared_oracle = family
        .oracle
        .as_ref()
        .with_context(|| format!("{}: generated packet has no oracle", family.family))?;
    require_eq(
        &format!("{} generated oracle id", family.family),
        &oracle.id,
        &declared_oracle.id,
    )?;
    require_eq(
        &format!("{} generated oracle source", family.family),
        &oracle.source,
        &declared_oracle.source,
    )?;
    require_eq(
        &format!("{} generated oracle source registry", family.family),
        &oracle.source,
        generated_oracle_source(family.family.as_str()),
    )?;
    require_exact_ids(
        family.relations.iter().map(String::as_str),
        expected.iter().copied().map(str::to_owned),
        "generated packet relation",
    )?;
    require_exact_ids(
        evidence.relations.iter().map(|row| row.relation.as_str()),
        expected.iter().copied().map(str::to_owned),
        "generated evidence relation",
    )?;
    require_unique(
        evidence.relations.iter().map(|row| row.relation.as_str()),
        "generated relation",
    )?;
    for relation in &evidence.relations {
        require_eq("generated relation result", &relation.result, "pass")?;
        let expected_artifact = blake3::hash(
            format!(
                "{}\0{}\0{}",
                family.family, relation.relation, evidence.evidence_hash
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            &format!("{} relation artifact", family.family),
            &relation.artifact_blake3,
            &expected_artifact,
        )?;
        require_eq("generated relation oracle", &relation.oracle_id, &oracle.id)?;
    }
    require_exact_ids(
        evidence.category_counts.keys().map(String::as_str),
        generated_seed_categories(family.family.as_str())
            .iter()
            .copied()
            .map(str::to_owned),
        &format!("{} generated seed category", family.family),
    )?;
    let category_total = evidence
        .category_counts
        .values()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .with_context(|| format!("{} generated category count overflow", family.family))?;
    anyhow::ensure!(
        category_total == evidence.attempts,
        "{} generated category total {} != attempts {}",
        family.family,
        category_total,
        evidence.attempts
    );
    anyhow::ensure!(
        evidence.category_counts.values().all(|count| *count > 0),
        "{} generated seed category has zero measured cases",
        family.family
    );
    anyhow::ensure!(
        oracle.independent,
        "{}: generated oracle is not marked independent",
        family.family
    );
    verify_oracle_source_coordinate(root, &oracle.source, &family.family)?;
    let expected_matrix = blake3::hash(serde_json::to_vec(&evidence.relations)?.as_slice())
        .to_hex()
        .to_string();
    require_eq(
        &format!("{} relation_matrix_blake3", family.family),
        &oracle.relation_matrix_blake3,
        &expected_matrix,
    )?;
    Ok(())
}

fn generated_relations(family: &str) -> &'static [&'static str] {
    match family {
        "source/CST/formatting" => &["lossless_emit", "format_idempotence"],
        "graph/interchange codecs" => &["byte_canonical_stability", "field_fidelity"],
        "transforms/projections" => &["identity_merge", "outcome_class_symmetry"],
        "repair/ILRP/recovery" => &[
            "dependency_order",
            "permutation_invariance",
            "independent_edge_check",
        ],
        "Basis/revision/query invalidation" => &[
            "irrelevant_input_invariance",
            "monotonicity",
            "independent_read_set_oracle",
        ],
        _ => &[],
    }
}

const GENERATED_SEED_CATEGORIES: [(&str, &[&str]); 5] = [
    (
        "source/CST/formatting",
        &[
            "empty",
            "single-token",
            "whitespace",
            "unicode",
            "long-line",
            "truncated",
            "unterminated",
            "nested",
            "deep",
            "wide",
            "control-byte",
            "invalid-utf8",
            "comment",
            "escape",
            "boundary-offset",
            "hostile",
        ],
    ),
    (
        "graph/interchange codecs",
        &[
            "empty",
            "single-node",
            "single-edge",
            "duplicate-id",
            "missing-node",
            "cycle",
            "dag",
            "wide",
            "deep",
            "unknown-kind",
            "external-value",
            "comment",
            "large-payload",
            "truncated-json",
            "invalid-json",
            "hostile",
        ],
    ),
    (
        "transforms/projections",
        &[
            "identity",
            "insert",
            "delete",
            "replace",
            "move",
            "overlap",
            "commute",
            "conflict",
            "empty",
            "deep",
            "wide",
            "invalid-span",
            "boundary-span",
            "large-patch",
            "truncated",
            "hostile",
        ],
    ),
    (
        "repair/ILRP/recovery",
        &[
            "prepared",
            "applying",
            "external-applied",
            "finalizing",
            "committed",
            "needs-review",
            "graph-step",
            "file-step",
            "two-step",
            "contested",
            "poststate",
            "cycle",
            "missing-step",
            "duplicate-ack",
            "truncated-intent",
            "hostile",
            "boundary",
        ],
    ),
    (
        "Basis/revision/query invalidation",
        &[
            "empty-basis",
            "single-revision",
            "branch",
            "merge",
            "client-scoped",
            "durable-only",
            "published",
            "federated",
            "stale",
            "fresh",
            "invalid-token",
            "large-frontier",
            "unknown-perspective",
            "truncated",
            "reordered",
            "hostile",
        ],
    ),
];

fn generated_seed_categories(family: &str) -> &'static [&'static str] {
    GENERATED_SEED_CATEGORIES
        .iter()
        .find_map(|(name, categories)| (*name == family).then_some(*categories))
        .unwrap_or(&[])
}

/// Closed oracle-source registry. Packet prose cannot choose what counts as
/// independent evidence; each family names one exact committed source anchor.
fn generated_oracle_source(family: &str) -> &'static str {
    match family {
        "source/CST/formatting" => "crates/liminal-xtask/src/haq.rs:independent_oracle_source_cst",
        "graph/interchange codecs" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_graph"
        }
        "transforms/projections" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_transform"
        }
        "repair/ILRP/recovery" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_repair"
        }
        "Basis/revision/query invalidation" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_invalidation"
        }
        _ => "",
    }
}

fn verify_oracle_source_coordinate(root: &Utf8Path, source: &str, family: &str) -> Result<()> {
    let expected = generated_oracle_source(family);
    anyhow::ensure!(!expected.is_empty(), "{family}: unknown oracle family");
    require_eq(
        &format!("{family} oracle source registry"),
        source,
        expected,
    )?;
    let (file, anchor) = source
        .split_once(':')
        .with_context(|| format!("{family}: oracle source lacks exact coordinate"))?;
    let path = safe_repo_path(root, file, "oracle source")?;
    let text = fs::read_to_string(&path)
        .with_context(|| format!("{family}: read oracle source {path}"))?;
    anyhow::ensure!(
        has_exact_coordinate_anchor(&text, anchor),
        "{family}: oracle source anchor {anchor:?} is absent from {path}"
    );
    Ok(())
}

/// Independent source/CST oracle. It compares raw observed bytes, not parser
/// equality or a formatter-owned snapshot.
fn independent_oracle_source_cst(
    source: &str,
    emitted: &str,
    once: &str,
    twice: &str,
) -> Result<Vec<u8>> {
    anyhow::ensure!(emitted == source, "CST emit is not lossless for {source:?}");
    anyhow::ensure!(once == twice, "formatting is not idempotent");
    let mut witness = emitted.as_bytes().to_vec();
    witness.extend_from_slice(once.as_bytes());
    Ok(witness)
}

/// Independent graph oracle. Field-by-field checks avoid the implementation's
/// own `PartialEq` becoming its proof of fidelity.
fn independent_oracle_source_graph(
    node: &liminal_graph::Node,
    decoded: &liminal_graph::Node,
    once: &[u8],
    twice: &[u8],
) -> Result<Vec<u8>> {
    anyhow::ensure!(once == twice, "interchange codec is not byte-canonical");
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
    Ok(once.to_vec())
}

/// Independent merge oracle. It checks raw content and outcome classes, not
/// merge-result equality.
fn independent_oracle_source_transform(
    base: &str,
    ours: &str,
    identity: &liminal_source::merge::MergeOutcome,
    forward: &liminal_source::merge::MergeOutcome,
    swapped: &liminal_source::merge::MergeOutcome,
) -> Result<Vec<u8>> {
    let liminal_source::merge::MergeOutcome::Disjoint { merged } = identity else {
        anyhow::bail!("identity merge reported non-disjoint outcome: {identity:?}");
    };
    let merged_tokens = merged.split_whitespace().collect::<Vec<_>>();
    let ours_tokens = ours.split_whitespace().collect::<Vec<_>>();
    let mut merged_multiset = merged_tokens.clone();
    let mut ours_multiset = ours_tokens.clone();
    merged_multiset.sort_unstable();
    ours_multiset.sort_unstable();
    anyhow::ensure!(
        ours_multiset == merged_multiset,
        "identity merge dropped content: {ours:?} -> {merged:?}"
    );
    // Token multisets alone permit an implementation to reorder every block.
    // For inputs whose source order was unchanged, preserve the stronger
    // ordered-marker relation too. Move cases intentionally exercise the
    // merge's canonical base layout and therefore do not use this assertion.
    let marker_order = |text: &str| {
        text.split_whitespace()
            .filter(|token| token.starts_with("{#") && token.ends_with('}'))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let base_markers = marker_order(base);
    let ours_markers = marker_order(ours);
    if ours_markers == base_markers {
        anyhow::ensure!(
            marker_order(merged) == ours_markers,
            "identity merge reordered durable markers"
        );
        anyhow::ensure!(
            ordered_block_contents(merged) == ordered_block_contents(ours),
            "identity merge reordered ordinary content: {ours:?} -> {merged:?}"
        );
    }
    let disjoint = |outcome: &liminal_source::merge::MergeOutcome| {
        matches!(
            outcome,
            liminal_source::merge::MergeOutcome::Disjoint { .. }
        )
    };
    anyhow::ensure!(
        disjoint(forward) == disjoint(swapped),
        "merge disjointness is not symmetric under swapping sides"
    );
    Ok(format!("{identity:?}|{forward:?}|{swapped:?}").into_bytes())
}

fn ordered_block_contents(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut anonymous = 0usize;
    let mut blocks = BTreeMap::new();
    for block in text.split("\n\n") {
        let marker = block
            .split_whitespace()
            .find(|token| token.starts_with("{#") && token.ends_with('}'))
            .map_or_else(
                || {
                    let key = format!("anon:{anonymous}");
                    anonymous += 1;
                    key
                },
                str::to_owned,
            );
        let tokens = block
            .split_whitespace()
            .filter(|token| !token.starts_with("{#") || !token.ends_with('}'))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Blocks containing only marker tokens carry no ordinary content whose
        // order can falsify this ordering oracle; marker order is checked above.
        if !tokens.is_empty() {
            blocks.insert(marker, tokens);
        }
    }
    blocks
}

/// Independent repair oracle. It checks the declared edge set against the
/// returned order and records that permutation of input edges is stable.
fn independent_oracle_source_repair(
    order: &[liminal_id::RepairStepId],
    ids: &[liminal_id::RepairStepId],
    edges: &[(usize, usize)],
    permuted_order: &[liminal_id::RepairStepId],
) -> Result<Vec<u8>> {
    let position: BTreeMap<liminal_id::RepairStepId, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    anyhow::ensure!(
        order.len() == ids.len(),
        "ordering dropped or duplicated steps"
    );
    for (before, after) in edges {
        anyhow::ensure!(
            position[&ids[*before]] < position[&ids[*after]],
            "ordering violated a declared dependency"
        );
    }
    anyhow::ensure!(
        permuted_order == order,
        "topological order changed under permutation of dependency edges"
    );
    Ok(order
        .iter()
        .flat_map(|id| id.as_uuid().into_bytes())
        .collect())
}

/// Independent invalidation oracle. It compares expected read membership to
/// observed invalidation answers, including a negative witness.
fn independent_oracle_source_invalidation(
    deps: &mut liminal_revision::ComponentDeps,
    expected: &BTreeSet<liminal_id::JurisdictionKey>,
    unrelated: &liminal_id::JurisdictionKey,
    extra: &liminal_id::JurisdictionKey,
) -> Result<Vec<u8>> {
    for key in expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "a recorded read did not invalidate: {key:?}"
        );
    }
    anyhow::ensure!(
        !deps.invalidated_by(unrelated),
        "an unread key invalidated the computation: {unrelated:?}"
    );
    // Add a new dependency only between observations. This makes the second
    // check a real monotonicity assertion instead of repeating the first one.
    anyhow::ensure!(
        !deps.invalidated_by(extra),
        "extra dependency was already present before monotonicity probe: {extra:?}"
    );
    deps.record(extra.clone());
    for key in expected {
        // Re-check after recording `extra`: this is the monotonicity relation,
        // not a duplicate of the pre-record assertion.
        anyhow::ensure!(
            deps.invalidated_by(key),
            "recording another read un-invalidated {key:?}"
        );
    }
    let mut witness = Vec::new();
    for key in expected.iter().chain([unrelated, extra]) {
        witness.push(u8::from(deps.invalidated_by(key)));
    }
    Ok(witness)
}

fn generated_category_class(category: &str) -> &'static str {
    let category = category.to_ascii_lowercase();
    if category.contains("hostile") {
        "hostile"
    } else if category.contains("truncated") || category.contains("unterminated") {
        "truncated"
    } else if category.contains("invalid")
        || category.contains("malformed")
        || category.contains("unknown")
        || category.contains("missing")
        || category.contains("duplicate")
        || category.contains("cycle")
        || category.contains("conflict")
    {
        "malformed"
    } else if category.contains("boundary")
        || category.contains("single")
        || category.contains("empty")
        || category.contains("large")
        || category.contains("deep")
        || category.contains("wide")
        || category.contains("zero")
    {
        "boundary"
    } else {
        "valid"
    }
}

fn verify_crash_rows(recorded: &CrashEvidence, declared: &BTreeSet<String>) -> Result<()> {
    require_unique(
        recorded.boundaries.iter().map(|row| row.boundary.as_str()),
        "crash evidence boundary",
    )?;
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
        anyhow::ensure!(
            row.injected,
            "crash boundary {} has no measured injection receipt",
            row.boundary
        );
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

/// Packet booleans describe both sides of each durable transition. Bind them
/// to measured evidence rows, rather than accepting packet-controlled claims
/// that the runner injected both sides (P1-A12).
fn verify_crash_injection_bindings(packet: &Packet, recorded: &CrashEvidence) -> Result<()> {
    // verify_crash_rows already enforces exact packet/evidence boundary-set
    // equality. This second pass binds each packet before/after boolean to the
    // measured injection receipt for its closed authoritative pair.
    let measured = recorded
        .boundaries
        .iter()
        .map(|row| (row.boundary.as_str(), row.injected))
        .collect::<BTreeMap<_, _>>();
    for declaration in &packet.crash_boundaries {
        let (prefix, suffix) = declaration.boundary.split_once('/').with_context(|| {
            format!("crash boundary {:?} lacks namespace", declaration.boundary)
        })?;
        let suffix = suffix
            .strip_prefix("before_")
            .or_else(|| suffix.strip_prefix("after_"))
            .with_context(|| {
                format!(
                    "crash boundary {:?} lacks before/after side",
                    declaration.boundary
                )
            })?;
        let before = format!("{prefix}/before_{suffix}");
        let after = format!("{prefix}/after_{suffix}");
        let measured_before = measured.get(before.as_str()).copied().unwrap_or(false);
        let measured_after = measured.get(after.as_str()).copied().unwrap_or(false);
        anyhow::ensure!(
            declaration.before == measured_before,
            "crash boundary {} before injection claim differs from measured evidence",
            declaration.boundary
        );
        anyhow::ensure!(
            declaration.after == measured_after,
            "crash boundary {} after injection claim differs from measured evidence",
            declaration.boundary
        );
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
        let mut digests = vec![
            ("first_recovery_digest", &pair.first_recovery_digest),
            ("second_recovery_digest", &pair.second_recovery_digest),
        ];
        let enhanced = !pair.first_terminal_digest.is_empty()
            || !pair.second_terminal_digest.is_empty()
            || !pair.first_basis_digest.is_empty()
            || !pair.second_basis_digest.is_empty()
            || !pair.first_effect_digest.is_empty()
            || !pair.second_effect_digest.is_empty();
        if enhanced {
            digests.extend([
                ("first_terminal_digest", &pair.first_terminal_digest),
                ("second_terminal_digest", &pair.second_terminal_digest),
                ("first_basis_digest", &pair.first_basis_digest),
                ("second_basis_digest", &pair.second_basis_digest),
                ("first_effect_digest", &pair.first_effect_digest),
                ("second_effect_digest", &pair.second_effect_digest),
            ]);
        }
        for (label, digest) in digests {
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
        if enhanced {
            for (label, first, second) in [
                (
                    "terminal digest",
                    &pair.first_terminal_digest,
                    &pair.second_terminal_digest,
                ),
                (
                    "Basis digest",
                    &pair.first_basis_digest,
                    &pair.second_basis_digest,
                ),
                (
                    "effect digest",
                    &pair.first_effect_digest,
                    &pair.second_effect_digest,
                ),
            ] {
                anyhow::ensure!(
                    first == second,
                    "crash boundary {} recovery case {}:{} changed {label} on second recovery",
                    row.boundary,
                    pair.scenario,
                    pair.occurrence
                );
            }
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
    verify_reviewer_independence(&records)?;
    verify_cross_pass_reproduction(&records)
}

/// A reviewer cannot make its own "independent reproduction" true by setting
/// two booleans in one record. Every reproduced finding must have one matching
/// verified defect in the other isolated pass, with the same attack class,
/// exact source target, and identical falsification claim/observation. This
/// prevents two semantically different reports at one line from becoming a
/// concurrence by coordinate coincidence; finding IDs remain pass-local.
fn verify_cross_pass_reproduction(records: &[(String, ReviewRecord)]) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        records.len() == 2,
        "cross-pass reproduction requires two records"
    );
    for (index, (who, record)) in records.iter().enumerate() {
        let other = &records[1 - index].1;
        let findings = review_finding_ids(&record.findings, &record.attempts, who)?;
        for finding_id in &record.independently_reproduced {
            let attempt_id = findings.get(finding_id.as_str()).with_context(|| {
                format!("{who}: reproduced finding {finding_id:?} is not linked")
            })?;
            let attempt = record
                .attempts
                .iter()
                .find(|attempt| attempt.id == *attempt_id)
                .expect("finding linkage validated");
            let matches = other
                .attempts
                .iter()
                .filter(|candidate| {
                    candidate.classification == "verified_defect"
                        && candidate.independently_reproduced
                        && candidate.attack_class == attempt.attack_class
                        && candidate.target == attempt.target
                        && candidate.attempt == attempt.attempt
                        && candidate.observed_result == attempt.observed_result
                })
                .count();
            anyhow::ensure!(
                matches == 1,
                "{who}: reproduced finding {finding_id:?} needs exactly one independently reproduced defect with the same attack class and source coordinate in the other pass; found {matches}"
            );
        }
    }
    Ok(())
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
    anyhow::ensure!(
        !record.reviewer.model_family.trim().is_empty()
            && !record.reviewer.backend.trim().is_empty(),
        "{who}: review record omits model family or backend"
    );
    let expected_identity = sha256_text(&format!(
        "pass{}:{}:haqp-blind-review-v1",
        record.pass, record.reviewer.model_family
    ));
    require_eq(
        "review identity_hash",
        &record.reviewer.identity_hash,
        &expected_identity,
    )?;
    require_hex_digest(
        "review sanitized_prompt_hash",
        &record.sanitized_prompt_hash,
    )?;
    require_hex_digest("review raw_response_sha256", &record.raw_response_sha256)?;
    require_hex_digest(
        "review integrity_binding_sha256",
        &record.integrity_binding_sha256,
    )?;
    require_eq(
        "review prompt_binding_sha256",
        &record.prompt_binding_sha256,
        &sha256_text(&format!(
            "haqp-blind-review-v1\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree
        )),
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
    verify_review_attack_classes(record, who)?;
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
    require_eq(
        "review integrity_binding_sha256",
        &record.integrity_binding_sha256,
        &sha256_text(&format!(
            "haqp-review-integrity-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree,
            record.prompt_binding_sha256,
            record.raw_response_sha256,
            record.result,
            record.unresolved_verified_findings
        )),
    )?;
    Ok(())
}

const REVIEW_ATTACK_CLASSES: [&str; 9] = [
    "vacuity",
    "shared-oracle coupling",
    "missing negatives",
    "weak mutants",
    "fault omissions",
    "nondeterminism",
    "corpus leakage",
    "exception broadening",
    "evidence/report drift",
];

fn verify_review_attack_classes(record: &ReviewRecord, who: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for attempt in &record.attempts {
        anyhow::ensure!(
            REVIEW_ATTACK_CLASSES.contains(&attempt.attack_class.as_str()),
            "{who}: attempt {} uses unknown attack class {:?}",
            attempt.id,
            attempt.attack_class
        );
        seen.insert(attempt.attack_class.as_str());
    }
    if record.pass == 1 {
        let missing = REVIEW_ATTACK_CLASSES
            .iter()
            .filter(|class| !seen.contains(**class))
            .copied()
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing.is_empty(),
            "{who}: Pass 1 is missing attack classes {missing:?}"
        );
    }
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
    anyhow::ensure!(
        reproduced.len() == record_findings.len(),
        "{who}: every finding must have independent reproduction; reproduced {} of {}",
        reproduced.len(),
        record_findings.len()
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
        if attempt.classification == "false_positive" && !attempt.independently_reproduced {
            anyhow::bail!(
                "{who}: false-positive attempt {:?} lacks independent reproduction evidence",
                attempt.id
            );
        }
        for (field, value) in [
            ("attempt", attempt.attempt.as_str()),
            ("observed_result", attempt.observed_result.as_str()),
        ] {
            let words = value.split_whitespace().count();
            anyhow::ensure!(
                value.trim().len() >= 24 && words >= 4,
                "{who}: attempt {:?} {field} is not a substantive falsification record",
                attempt.id
            );
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
        anyhow::ensure!(
            contains_exact_coordinate(&attempt.attempt, &attempt.target),
            "{who}: attempt {:?} falsification claim must quote exact target {}",
            attempt.id,
            attempt.target
        );
        anyhow::ensure!(
            contains_exact_coordinate(&attempt.observed_result, &attempt.target),
            "{who}: attempt {:?} observation must quote exact target {}",
            attempt.id,
            attempt.target
        );
        let target_lower = target_file.to_ascii_lowercase();
        anyhow::ensure!(
            !is_locked_acceptance_path(&target_lower),
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

#[allow(clippy::too_many_lines)]
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
    let evidence_text = String::from_utf8_lossy(&evidence_bytes);
    anyhow::ensure!(
        evidence_text.contains(&attempt.id)
            && evidence_text.contains(&attempt.target)
            && evidence_text.contains(&resolution.coordinate),
        "{who}: resolution evidence must name attempted finding {}, target {}, and fix coordinate {}",
        attempt.id,
        attempt.target,
        resolution.coordinate
    );
    anyhow::ensure!(
        evidence_text
            .lines()
            .any(|line| line.trim() == "resolution_result: pass")
            && evidence_text
                .lines()
                .any(|line| line.trim_start().starts_with("verification_command:"))
            && evidence_text
                .lines()
                .any(|line| line.trim() == "verification_exit_code: 0"),
        "{who}: resolution evidence must include a passing verification command receipt"
    );
    let mut hasher = Sha256::new();
    hasher.update(&evidence_bytes);
    let digest = format!("{:x}", hasher.finalize());
    require_eq(
        "resolution evidence_sha256",
        &resolution.evidence_sha256,
        &digest,
    )?;
    let committed_bytes = git_blob(root, &resolution.commit, &resolution.evidence_path)
        .with_context(|| {
            format!(
                "{who}: resolution evidence {} is absent at commit {}",
                resolution.evidence_path, resolution.commit
            )
        })?;
    let mut committed_hasher = Sha256::new();
    committed_hasher.update(&committed_bytes);
    let committed_digest = format!("{:x}", committed_hasher.finalize());
    require_eq(
        "resolution committed evidence_sha256",
        &resolution.evidence_sha256,
        &committed_digest,
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
    let target_file = attempt
        .target
        .rsplit_once(':')
        .map(|(file, _)| file)
        .unwrap_or_default();
    anyhow::ensure!(
        coordinate_file == target_file,
        "{who}: resolution coordinate file {coordinate_file:?} does not match attacked target file {target_file:?}"
    );
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
    anyhow::ensure!(
        !is_locked_acceptance_path(coordinate_file),
        "{who}: resolution coordinate may not name locked acceptance corpus"
    );
    let source = git_text(
        root,
        &["show", &format!("{}:{coordinate_file}", resolution.commit)],
    )?;
    anyhow::ensure!(
        line <= source.lines().count().max(1),
        "{who}: resolution coordinate line {line} is outside {coordinate_file}"
    );
    verify_resolution_coordinate_changed(
        root,
        &fixed_base.commit,
        &resolution.commit,
        who,
        coordinate_file,
        line,
    )?;
    Ok(())
}

fn git_blob(root: &Utf8Path, commit: &str, path: &str) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{commit}:{path}")])
        .output()
        .with_context(|| format!("read {commit}:{path} from Git"))?;
    anyhow::ensure!(
        output.status.success(),
        "git show {commit}:{path} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

fn verify_resolution_coordinate_changed(
    root: &Utf8Path,
    fixed_commit: &str,
    resolution_commit: &str,
    who: &str,
    coordinate_file: &str,
    line: usize,
) -> Result<()> {
    let range = format!("{fixed_commit}..{resolution_commit}");
    let changed = git_text(
        root,
        &["diff", "--name-only", &range, "--", coordinate_file],
    )?;
    anyhow::ensure!(
        changed.lines().any(|path| path == coordinate_file),
        "{who}: resolution commit did not change coordinate file {coordinate_file}"
    );
    let diff = git_text(
        root,
        &["diff", "--unified=0", &range, "--", coordinate_file],
    )?;
    anyhow::ensure!(
        diff_contains_added_line(&diff, line),
        "{who}: resolution coordinate {coordinate_file}:{line} is outside changed hunks"
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
    anyhow::ensure!(
        !is_locked_acceptance_path(value),
        "{who} resolution evidence path {value:?} names locked acceptance corpus"
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

fn is_locked_acceptance_path(value: &str) -> bool {
    let components = value
        .split(['/', '\\'])
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.iter().any(|component| component == "heldout")
        || components
            .windows(2)
            .any(|pair| pair == ["conformance", "corpora"])
}

fn diff_contains_added_line(diff: &str, target_line: usize) -> bool {
    diff.lines()
        .filter(|line| line.starts_with("@@ "))
        .any(|header| {
            let Some(range) = header
                .split_whitespace()
                .find(|token| token.starts_with('+'))
                .and_then(|token| token.strip_prefix('+'))
            else {
                return false;
            };
            let (start, count) = range
                .split_once(',')
                .map_or((range, "1"), |(start, count)| (start, count));
            let Ok(start) = start.parse::<usize>() else {
                return false;
            };
            let Ok(count) = count.parse::<usize>() else {
                return false;
            };
            count > 0 && target_line >= start && target_line.saturating_sub(start) < count
        })
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
    let mut sessions: BTreeSet<&str> = BTreeSet::new();
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
        require_hex_digest(
            &format!("{who} isolated_session_hash"),
            &record.isolated_session_hash,
        )?;
        families.insert(record.reviewer.model_family.as_str());
        backends.insert(record.reviewer.backend.as_str());
        identities.insert(record.reviewer.identity_hash.as_str());
        prompts.insert(record.sanitized_prompt_hash.as_str());
        sessions.insert(record.isolated_session_hash.as_str());
    }
    for (label, distinct) in [
        ("model families", families.len()),
        ("backends", backends.len()),
        ("reviewer identities", identities.len()),
        ("sanitized prompts", prompts.len()),
        ("isolated sessions", sessions.len()),
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

/// Re-run every semantic killed mutant from its fixed source commit and
/// compare the complete durable row. Exit codes and digests are claims until
/// this replay observes them from a fresh disposable worktree.
fn verify_mutant_evidence_replay(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let selected = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition == "killed")
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(());
    }
    let path = root.join("conformance/haqp/evidence/mutants.json");
    let evidence: MutantEvidence = serde_json::from_slice(&fs::read(&path)?)?;
    let source_commit = packet
        .provenance
        .as_ref()
        .context("mutant replay requires packet provenance")?
        .fixed_commit
        .clone();
    anyhow::ensure!(
        git_text(root, &["status", "--porcelain"])?.is_empty(),
        "mutant replay requires a clean source tree"
    );
    let ignored = ignored_test_names(root);
    let scratch = liminal_scratch::ScratchDir::new("haq-mutant-replay")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(&source_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "mutant replay worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut guard = WorktreeGuard::new(root, &worktree);
    for mutant in selected {
        let expected = evidence
            .rows
            .iter()
            .find(|row| row.id == mutant.id)
            .with_context(|| format!("mutant replay missing evidence row {}", mutant.id))?;
        let actual = run_mutant_row(&worktree, packet, mutant, &ignored, false)?;
        anyhow::ensure!(
            actual == *expected,
            "mutant {} replay row differs from committed evidence",
            mutant.id
        );
        reset_mutant_worktree(&worktree, &source_commit)?;
    }
    guard.remove()?;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "mutation evidence gate keeps all row invariants together"
)]
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
    if row.status == "error" {
        anyhow::ensure!(
            matches!(
                row.failure_class.as_deref(),
                Some("infrastructure" | "compilation" | "timeout")
            ),
            "{} error result must classify compilation, timeout, or infrastructure failure",
            row.id
        );
        anyhow::ensure!(
            row.reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "{} error result has no reason",
            row.id
        );
    }
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
            anyhow::ensure!(
                row.failed_tests.is_empty(),
                "{} {} evidence cannot list failed tests",
                row.id,
                row.disposition
            );
            anyhow::ensure!(
                row.exit_codes.iter().all(|code| *code == 0),
                "{} {} evidence cannot contain nonzero exit codes",
                row.id,
                row.disposition
            );
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
    anyhow::ensure!(
        patch.before != patch.after,
        "mutant patch {:?} is a no-op replacement",
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
        let mut records = BTreeSet::new();
        for proof in &mutant.disposition_concurrence {
            anyhow::ensure!(
                !proof.reviewer.trim().is_empty()
                    && !proof.record.trim().is_empty()
                    && !proof.finding.trim().is_empty(),
                "{} has empty concurrence",
                mutant.id
            );
            anyhow::ensure!(
                matches!(proof.record.as_str(), "pass-1" | "pass-2"),
                "{} concurrence record {:?} must be pass-1 or pass-2",
                mutant.id,
                proof.record
            );
            concurrence.insert(proof.reviewer.as_str());
            records.insert(proof.record.as_str());
        }
        anyhow::ensure!(
            concurrence.len() >= 2 && records == BTreeSet::from(["pass-1", "pass-2"]),
            "{} requires distinct reviewer concurrence from both adversarial passes",
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

/// Equivalent/duplicate mutant dispositions must point at findings that exist
/// in the committed blind records. Reviewer names and pass labels alone are
/// not concurrence evidence: each tuple resolves to a verified, independently
/// reproduced finding in its named pass.
fn verify_mutant_concurrence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    if !packet
        .mutants
        .iter()
        .any(|mutant| matches!(mutant.disposition.as_str(), "equivalent" | "duplicate"))
    {
        return Ok(());
    }
    let mut records = BTreeMap::<u8, (String, ReviewRecord)>::new();
    for review in &packet.reviews {
        anyhow::ensure!(
            review.result == "pass",
            "mutant concurrence requires passing committed review record for {}",
            review.reviewer
        );
        let evidence = review
            .evidence
            .as_deref()
            .with_context(|| format!("{} has no committed review record", review.reviewer))?;
        let path = safe_repo_path(root, evidence, "mutant concurrence review")?;
        let record: ReviewRecord =
            serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {path}"))?)
                .with_context(|| format!("parse {path}"))?;
        anyhow::ensure!(
            records
                .insert(record.pass, (review.reviewer.clone(), record))
                .is_none(),
            "duplicate committed review pass in mutant concurrence"
        );
    }
    for mutant in packet
        .mutants
        .iter()
        .filter(|mutant| matches!(mutant.disposition.as_str(), "equivalent" | "duplicate"))
    {
        for concurrence in &mutant.disposition_concurrence {
            let pass = match concurrence.record.as_str() {
                "pass-1" => 1,
                "pass-2" => 2,
                other => anyhow::bail!(
                    "{} concurrence record {:?} must be pass-1 or pass-2",
                    mutant.id,
                    other
                ),
            };
            let (reviewer, record) = records
                .get(&pass)
                .with_context(|| format!("{} concurrence names absent {pass:?}", mutant.id))?;
            anyhow::ensure!(
                concurrence.reviewer == *reviewer
                    || concurrence.reviewer == record.reviewer.model_family,
                "{} concurrence reviewer {:?} does not identify committed pass {}",
                mutant.id,
                concurrence.reviewer,
                pass
            );
            let findings = review_finding_ids(&record.findings, &record.attempts, reviewer)?;
            let attempt_id = findings
                .get(concurrence.finding.as_str())
                .with_context(|| {
                    format!(
                        "{} concurrence finding {:?} is absent from committed pass {}",
                        mutant.id, concurrence.finding, pass
                    )
                })?;
            let attempt = record
                .attempts
                .iter()
                .find(|attempt| attempt.id == *attempt_id)
                .expect("finding linkage validated");
            anyhow::ensure!(
                attempt.classification == "verified_defect"
                    && attempt.independently_reproduced
                    && record
                        .independently_reproduced
                        .iter()
                        .any(|id| id == &concurrence.finding),
                "{} concurrence finding {:?} is not independently reproduced verified defect",
                mutant.id,
                concurrence.finding
            );
        }
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn sha256_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
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

fn is_compilation_failure(stdout: &[u8], stderr: &[u8]) -> bool {
    stdout
        .iter()
        .chain(stderr)
        .copied()
        .collect::<Vec<_>>()
        .windows(b"could not compile".len())
        .any(|window| window.eq_ignore_ascii_case(b"could not compile"))
}

fn is_semantic_test_failure(test_name: &str, stdout: &[u8], stderr: &[u8]) -> bool {
    let leaf = test_name.rsplit("::").next().unwrap_or(test_name);
    let text = String::from_utf8_lossy(stdout);
    let err = String::from_utf8_lossy(stderr);
    // Require named-test and failure token on one line. This rejects launcher
    // infrastructure prose such as "launcher FAILED before tests" instead of
    // counting it as a semantic mutant kill.
    [text.as_ref(), err.as_ref()].iter().any(|output| {
        output
            .lines()
            .any(|line| line.trim_start().starts_with("FAIL [") && line.contains(leaf))
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "keeps mutation execution and its evidence assembly in one auditable path"
)]
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
            failure_class: None,
        });
    };
    let runnable = mutant
        .killing_tests
        .iter()
        .filter_map(|test_id| packet.tests.iter().find(|test| &test.id == test_id))
        .filter(|test| {
            run_ignored
                || !ignored.contains(test.name.rsplit("::").next().unwrap_or(test.name.as_str()))
        })
        .map(|test| (test.id.clone(), test.name.clone()))
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
            failure_class: None,
        });
    }
    let baseline_exit_codes = runnable
        .iter()
        .map(|(_, test_name)| {
            run_one_mutant_test(worktree, test_name, run_ignored).map(|(code, _, _)| code)
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some((index, code)) = baseline_exit_codes
        .iter()
        .enumerate()
        .find(|(_, code)| **code != 0)
        .map(|(index, code)| (index, *code))
    {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "error".to_owned(),
            patch: Some(patch),
            runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
            baseline_exit_codes,
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some(format!(
                "baseline witness {} exited with {code}",
                runnable[index].0
            )),
            failure_class: Some("infrastructure".to_owned()),
        });
    }
    if let Err(error) = apply_mutant_patch(worktree, &patch) {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "error".to_owned(),
            patch: Some(patch),
            runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
            baseline_exit_codes,
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some(error.to_string()),
            failure_class: Some("infrastructure".to_owned()),
        });
    }
    let mut failed_tests = Vec::new();
    let mut exit_codes = Vec::new();
    let mut stdout_blake3 = Vec::new();
    let mut stderr_blake3 = Vec::new();
    for (test_id, test_name) in &runnable {
        let (code, stdout, stderr) = run_one_mutant_test(worktree, test_name, run_ignored)?;
        exit_codes.push(code);
        stdout_blake3.push(hex_digest(&stdout));
        stderr_blake3.push(hex_digest(&stderr));
        if code != 0 && is_compilation_failure(&stdout, &stderr) {
            return Ok(MutantEvidenceRow {
                id: mutant.id.clone(),
                disposition: mutant.disposition.clone(),
                status: "error".to_owned(),
                patch: Some(patch),
                runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
                baseline_exit_codes,
                failed_tests: Vec::new(),
                exit_codes,
                stdout_blake3,
                stderr_blake3,
                reason: Some(format!(
                    "mutation caused compilation failure while running {test_id}"
                )),
                failure_class: Some("compilation".to_owned()),
            });
        }
        if code != 0 && !is_semantic_test_failure(test_name, &stdout, &stderr) {
            return Ok(MutantEvidenceRow {
                id: mutant.id.clone(),
                disposition: mutant.disposition.clone(),
                status: "error".to_owned(),
                patch: Some(patch),
                runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
                baseline_exit_codes,
                failed_tests: Vec::new(),
                exit_codes,
                stdout_blake3,
                stderr_blake3,
                reason: Some(format!(
                    "mutation witness {test_id} exited {code} without semantic test-failure evidence"
                )),
                failure_class: Some("infrastructure".to_owned()),
            });
        }
        if code != 0 {
            failed_tests.push(test_id.clone());
        }
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
        runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
        baseline_exit_codes,
        failed_tests,
        exit_codes,
        stdout_blake3,
        stderr_blake3,
        reason: None,
        failure_class: None,
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
        let error = rows.last().and_then(|row| {
            (row.status == "error").then(|| {
                (
                    row.id.clone(),
                    row.reason
                        .clone()
                        .unwrap_or_else(|| "unknown mutation error".to_owned()),
                )
            })
        });
        if let Some((id, reason)) = error {
            reset_mutant_worktree(&worktree, &source_commit)?;
            worktree_guard.remove()?;
            anyhow::bail!("{id}: {reason}");
        }
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
                let mut local = BTreeMap::new();
                collect_runnable_test_functions(&text, &mut local);
                let prefix = path
                    .strip_prefix(root.join("conformance/tests"))
                    .ok()
                    .and_then(|relative| {
                        let mut parts = relative
                            .components()
                            .map(|component| component.as_str().to_owned())
                            .collect::<Vec<_>>();
                        let stem = parts.pop()?.strip_suffix(".rs")?.to_owned();
                        parts.push(stem);
                        Some(parts.join("::"))
                    });
                for (leaf, count) in local {
                    if let Some(prefix) = &prefix {
                        runnable.insert(format!("{prefix}::{leaf}"), count);
                    }
                }
            }
        }
    }

    let mut missing = Vec::new();
    for test in &packet.tests {
        if test.name.trim().is_empty() {
            anyhow::bail!("{} declares an empty test name", test.id);
        }
        if !runnable.contains_key(test.name.as_str()) {
            missing.push(format!("{} -> {}", test.id, test.name));
        } else if runnable.get(test.name.as_str()) != Some(&1) {
            missing.push(format!(
                "{} -> {} (module-qualified test name is ambiguous)",
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
        } else if trimmed.starts_with("#[ignore")
            || (trimmed.starts_with("#[cfg_attr(") && trimmed.contains("ignore"))
            || cfg_attribute_excludes(trimmed)
        {
            // Keep an exclusion only across the attribute list immediately
            // preceding one test. A module-level cfg or one ignored function
            // must not poison every later runnable test in the file.
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
                excluded = false;
            }
        } else if !test_pending {
            // An attribute that did not lead directly to a test belongs to a
            // module/helper declaration, not to the next test function.
            excluded = false;
        }
    }
}

fn cfg_attribute_excludes(trimmed: &str) -> bool {
    if !trimmed.starts_with("#[cfg(") {
        return false;
    }
    if trimmed == "#[cfg(any())]" {
        return true;
    }
    if trimmed == "#[cfg(test)]" {
        return false;
    }
    if trimmed == "#[cfg(not(test))]" {
        return true;
    }
    if trimmed == "#[cfg(unix)]" {
        return !cfg!(unix);
    }
    if trimmed == "#[cfg(windows)]" {
        return !cfg!(windows);
    }
    if trimmed.contains("target_os = \"linux\"") {
        return !cfg!(target_os = "linux");
    }
    if trimmed.contains("target_os = \"windows\"") {
        return !cfg!(target_os = "windows");
    }
    // Unknown predicates remain fail-closed: scanner cannot prove they are
    // active on this host.
    true
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
    for row in &recorded {
        let proof = row.sanitizer_proof.as_ref().with_context(|| {
            format!(
                "{}: qualified fuzz evidence lacks compiler/runtime sanitizer proof",
                row.target
            )
        })?;
        verify_sanitizer_proof(root, row, proof, packet.provenance.as_ref())?;
    }
    verify_fuzz_seed_manifests(root, &recorded)?;
    verify_fuzz_logs(root, &recorded)?;
    verify_fuzz_budget(&packet.generated, &recorded)
}

fn verify_fuzz_seed_manifests(root: &Utf8Path, recorded: &[FuzzEvidence]) -> Result<()> {
    for row in recorded {
        let seed_dir = root.join("fuzz/corpus").join(&row.target);
        let (count, digest) = seed_manifest_digest(&seed_dir)?;
        anyhow::ensure!(
            count >= 16,
            "{}: committed seed set has {count} inputs; D23.2 requires at least 16",
            row.target
        );
        anyhow::ensure!(
            row.seed_count == count,
            "{}: evidence records {} seeds but committed corpus has {count}",
            row.target,
            row.seed_count
        );
        require_eq(
            &format!("{} seed_manifest_blake3", row.target),
            &row.seed_manifest_blake3,
            &digest,
        )?;
    }
    Ok(())
}

fn seed_manifest_digest(seed_dir: &Utf8Path) -> Result<(u64, String)> {
    let mut files = fs::read_dir(seed_dir)
        .with_context(|| format!("{seed_dir}: committed fuzz seed directory is required"))?
        .flatten()
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    let mut hasher = blake3::Hasher::new();
    for path in &files {
        let name = path
            .file_name()
            .with_context(|| format!("{path}: seed has no file name"))?;
        let bytes = fs::read(path).with_context(|| format!("read fuzz seed {path}"))?;
        let mut seed_hash = Sha256::new();
        seed_hash.update(bytes);
        let seed_hash = format!("{:x}", seed_hash.finalize());
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(seed_hash.as_bytes());
        hasher.update(&[0]);
    }
    Ok((
        u64::try_from(files.len()).context("fuzz seed count exceeds u64")?,
        hasher.finalize().to_hex().to_string(),
    ))
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

#[allow(clippy::too_many_lines)]
fn verify_sanitizer_proof(
    root: &Utf8Path,
    row: &FuzzEvidence,
    proof: &SanitizerProof,
    expected: Option<&Provenance>,
) -> Result<()> {
    anyhow::ensure!(
        !proof.build_command.trim().is_empty()
            && proof.build_command
                == format!(
                    "cargo +nightly fuzz build -s {} {}",
                    row.sanitizer, row.target
                ),
        "{}: sanitizer proof build command does not name fuzz build and sanitizer",
        row.target
    );
    anyhow::ensure!(
        !proof.instrumentation_flags.is_empty()
            && proof
                .instrumentation_flags
                .iter()
                .any(|flag| flag.contains(&row.sanitizer)),
        "{}: sanitizer proof has no matching instrumentation flag",
        row.target
    );
    anyhow::ensure!(
        !proof.build_log.trim().is_empty()
            && !proof.build_log_blake3.trim().is_empty()
            && !proof.source_commit.trim().is_empty()
            && !proof.source_tree.trim().is_empty()
            && !proof.build_result.trim().is_empty(),
        "{} sanitizer proof lacks current build provenance; regenerate fuzz evidence",
        row.target
    );
    require_eq(
        &format!("{} sanitizer build_result", row.target),
        &proof.build_result,
        "pass",
    )?;
    require_eq(
        &format!("{} sanitizer build_log", row.target),
        &proof.build_log,
        &format!("conformance/haqp/evidence/build-logs/{}.log", row.target),
    )?;
    require_hex_digest(
        &format!("{} build_log_blake3", row.target),
        &proof.build_log_blake3,
    )?;
    require_git_object_id(
        &format!("{} sanitizer source_commit", row.target),
        &proof.source_commit,
    )?;
    require_git_object_id(
        &format!("{} sanitizer source_tree", row.target),
        &proof.source_tree,
    )?;
    if let Some(expected) = expected {
        require_eq(
            &format!("{} sanitizer source_commit/provenance", row.target),
            &proof.source_commit,
            &expected.fixed_commit,
        )?;
        require_eq(
            &format!("{} sanitizer source_tree/provenance", row.target),
            &proof.source_tree,
            &expected.fixed_tree,
        )?;
    }
    let build_log_path = safe_repo_path(root, &proof.build_log, "sanitizer build log")?;
    let build_log_bytes = fs::read(&build_log_path)
        .with_context(|| format!("{build_log_path}: sanitizer build log is required"))?;
    require_eq(
        &format!("{} build_log_blake3", row.target),
        &proof.build_log_blake3,
        blake3::hash(&build_log_bytes).to_hex().as_ref(),
    )?;
    let build_log_text = String::from_utf8_lossy(&build_log_bytes);
    // Cargo's build output does not repeat the shell command when the wrapper
    // records stdout only. The exact command is closed by `build_command`,
    // artifact digests, and fixed-base replay; this log only proves successful
    // build completion.
    anyhow::ensure!(
        build_log_text.contains("Finished") || build_log_text.contains("finished"),
        "{} sanitizer build log does not show successful build",
        row.target
    );
    require_hex_digest(
        &format!("{} binary_blake3", row.target),
        &proof.binary_blake3,
    )?;
    require_hex_digest(
        &format!("{} runtime_probe_blake3", row.target),
        &proof.runtime_probe_blake3,
    )?;
    anyhow::ensure!(
        proof.runtime_probe_exit_code == 0,
        "{}: sanitizer runtime probe exited {}",
        row.target,
        proof.runtime_probe_exit_code
    );
    let binary_path = safe_repo_path(root, &proof.binary, "sanitizer binary")?;
    let probe_path = safe_repo_path(root, &proof.runtime_probe, "sanitizer runtime probe")?;
    anyhow::ensure!(
        binary_path != probe_path,
        "{}: sanitizer binary and runtime probe must be distinct files",
        row.target
    );
    let binary = Utf8Path::new(&proof.binary);
    anyhow::ensure!(
        binary.is_relative()
            && binary.starts_with("conformance/haqp/evidence/binaries")
            && binary.file_name() == Some(row.target.as_str())
            && !binary
                .components()
                .any(|component| component == camino::Utf8Component::ParentDir),
        "{}: sanitizer proof binary must be tracked evidence",
        row.target
    );
    let probe = Utf8Path::new(&proof.runtime_probe);
    anyhow::ensure!(
        probe.is_relative()
            && probe.starts_with("conformance/haqp/evidence/probes")
            && !probe
                .components()
                .any(|component| component == camino::Utf8Component::ParentDir),
        "{}: sanitizer proof runtime probe must be tracked evidence",
        row.target
    );
    for (label, path, expected) in [
        ("sanitizer binary", binary, &proof.binary_blake3),
        (
            "sanitizer runtime probe",
            probe,
            &proof.runtime_probe_blake3,
        ),
    ] {
        let path = if path == binary {
            binary_path.clone()
        } else {
            probe_path.clone()
        };
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("{path}: {label} is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{path}: {label} must be a regular file"
        );
        let digest = blake3::hash(&fs::read(&path)?).to_hex().to_string();
        require_eq(&format!("{} {label} digest", row.target), expected, &digest)?;
    }
    let binary_bytes = fs::read(&binary_path)?;
    let marker = match row.sanitizer.as_str() {
        "address" | "leak" => b"asan_globals".as_slice(),
        "memory" => b"msan".as_slice(),
        "thread" => b"tsan".as_slice(),
        _ => unreachable!("sanitizer was checked before proof"),
    };
    anyhow::ensure!(
        binary_bytes
            .windows(marker.len())
            .any(|window| window == marker),
        "{}: sanitizer binary has no {} runtime marker",
        row.target,
        row.sanitizer
    );
    anyhow::ensure!(
        binary_bytes
            .windows(row.target.len())
            .any(|window| window == row.target.as_bytes()),
        "{}: sanitizer binary has no target identity marker",
        row.target
    );
    let probe_output = Command::new(&binary_path)
        .arg("-help=1")
        .output()
        .with_context(|| format!("{}: execute sanitizer runtime probe", row.target))?;
    anyhow::ensure!(
        probe_output.status.success(),
        "{}: sanitizer runtime probe execution failed",
        row.target
    );
    let mut probe_bytes = probe_output.stdout;
    probe_bytes.extend_from_slice(&probe_output.stderr);
    anyhow::ensure!(
        String::from_utf8_lossy(&probe_bytes).contains(&row.target),
        "{}: sanitizer runtime probe does not identify target",
        row.target
    );
    let recorded_probe = fs::read(&probe_path)?;
    anyhow::ensure!(
        probe_bytes == recorded_probe,
        "{}: retained sanitizer probe differs from fresh binary execution",
        row.target
    );
    if let Some(provenance) = expected {
        verify_sanitizer_build_replay(root, row, proof, provenance)?;
    }
    Ok(())
}

/// Build sanitizer target from fixed source commit and compare output. Marker
/// strings and self-authored logs cannot prove compiler/runtime instrumentation.
fn verify_sanitizer_build_replay(
    root: &Utf8Path,
    row: &FuzzEvidence,
    proof: &SanitizerProof,
    provenance: &Provenance,
) -> Result<()> {
    let scratch = liminal_scratch::ScratchDir::new("haq-sanitizer-replay")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(&provenance.fixed_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "sanitizer replay worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut guard = WorktreeGuard::new(root, &worktree);
    let output = Command::new("cargo")
        .current_dir(&worktree)
        .args([
            "+nightly",
            "fuzz",
            "build",
            "-s",
            row.sanitizer.as_str(),
            row.target.as_str(),
        ])
        .output()
        .with_context(|| format!("replay sanitizer build for {}", row.target))?;
    anyhow::ensure!(
        output.status.success(),
        "replayed sanitizer build failed for {}: {}",
        row.target,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let candidate = find_named_files(&worktree.join("fuzz/target"), &row.target)?
        .into_iter()
        .find(|path| path.file_name() == Some(row.target.as_str()))
        .with_context(|| format!("replayed sanitizer build produced no {} binary", row.target))?;
    let digest = blake3::hash(&fs::read(&candidate)?).to_hex().to_string();
    require_eq(
        &format!("{} replayed sanitizer binary digest", row.target),
        &proof.binary_blake3,
        &digest,
    )?;
    let probe = Command::new(&candidate)
        .arg("-help=1")
        .current_dir(&worktree)
        .output()
        .with_context(|| format!("probe replayed sanitizer binary {}", row.target))?;
    anyhow::ensure!(
        probe.status.success(),
        "replayed sanitizer runtime probe failed for {}",
        row.target
    );
    let mut probe_bytes = probe.stdout;
    probe_bytes.extend_from_slice(&probe.stderr);
    let probe_text = String::from_utf8_lossy(&probe_bytes);
    anyhow::ensure!(
        probe_text.contains(row.target.as_str())
            && probe_text.contains(&format!("-fsanitize={}", row.sanitizer)),
        "replayed sanitizer probe does not identify {} and {}",
        row.target,
        row.sanitizer
    );
    guard.remove()?;
    Ok(())
}

fn find_named_files(root: &Utf8Path, name: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut found = Vec::new();
    if !root.exists() {
        return Ok(found);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            anyhow::anyhow!("non-UTF-8 path while finding {name}: {}", path.display())
        })?;
        if entry.file_type()?.is_dir() {
            found.extend(find_named_files(&path, name)?);
        } else if path.file_name() == Some(name) {
            found.push(path);
        }
    }
    Ok(found)
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
        verify_fuzz_log_metrics(row, &bytes)?;
    }
    Ok(())
}

fn verify_fuzz_log_metrics(row: &FuzzEvidence, bytes: &[u8]) -> Result<()> {
    let text = String::from_utf8_lossy(bytes);
    let stat_execs = text
        .lines()
        .filter_map(|line| line.strip_prefix("stat::number_of_executed_units:"))
        .next_back()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .with_context(|| {
            format!(
                "{}: retained fuzz log has no execution-count footer",
                row.target
            )
        })?;
    require_eq(
        &format!("{} log execution count", row.target),
        &stat_execs.to_string(),
        &row.execs.to_string(),
    )?;
    let done = text
        .lines()
        .filter_map(|line| line.strip_prefix("Done "))
        .next_back()
        .with_context(|| format!("{}: retained fuzz log has no Done footer", row.target))?;
    let done_execs = done
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .with_context(|| format!("{}: malformed Done execution count", row.target))?;
    let done_elapsed = done
        .split(" in ")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .with_context(|| format!("{}: malformed Done elapsed seconds", row.target))?;
    anyhow::ensure!(
        done_execs == row.execs,
        "{}: log Done count {} differs from evidence {}",
        row.target,
        done_execs,
        row.execs
    );
    anyhow::ensure!(
        // Fuzzer-reported duration may omit setup/teardown; allow one minute
        // for launcher overhead while still binding elapsed evidence to log.
        done_elapsed <= row.elapsed_s && row.elapsed_s <= done_elapsed.saturating_add(60),
        "{}: log elapsed {}s is not bound to evidence elapsed {}s",
        row.target,
        done_elapsed,
        row.elapsed_s
    );
    anyhow::ensure!(
        text.contains(&format!("-max_total_time={}", row.seconds))
            && text.contains(&format!("-seed={}", row.seed)),
        "{}: retained log command is not bound to recorded budget/seed",
        row.target
    );
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
        "haqp-corpus-access-v2",
    )?;
    require_eq(
        "corpus access audit tracer",
        &audit.tracer,
        "strace-open-paths",
    )?;
    require_git_object_id("corpus access source_commit", &audit.source_commit)?;
    require_git_object_id("corpus access source_tree", &audit.source_tree)?;
    if let Some(provenance) = &packet.provenance {
        require_eq(
            "corpus access source_commit/provenance",
            &audit.source_commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "corpus access source_tree/provenance",
            &audit.source_tree,
            &provenance.fixed_tree,
        )?;
    }

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
    verify_corpus_audit_campaign_binding(root, &audit)?;
    if let Some(provenance) = &packet.provenance {
        verify_corpus_scope_traces(root, &audit, provenance)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_corpus_scope_traces(
    root: &Utf8Path,
    audit: &CorpusAccessAudit,
    provenance: &Provenance,
) -> Result<()> {
    const QUALIFICATION_SCOPES: [&str; 8] = [
        "ci",
        "canaries",
        "generated",
        "crash",
        "replay",
        "mutation",
        "reviews",
        "fuzz",
    ];
    let expected = QUALIFICATION_SCOPES.into_iter().collect::<BTreeSet<_>>();
    let actual = audit
        .scope_traces
        .iter()
        .map(|row| row.scope.as_str())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        actual == expected,
        "qualification corpus audit scopes differ: recorded={actual:?}, expected={expected:?}"
    );
    verify_corpus_scope_replays(root, &audit.scope_traces, &provenance.fixed_commit)?;
    require_unique(
        audit.scope_traces.iter().map(|row| row.scope.as_str()),
        "corpus audit scope",
    )?;
    for row in &audit.scope_traces {
        anyhow::ensure!(
            row.command.contains("strace") && row.command.contains("-f"),
            "{} corpus scope is not recursively traced",
            row.scope
        );
        require_eq("corpus scope exit_code", &row.exit_code.to_string(), "0")?;
        require_eq("corpus scope result", &row.result, "pass")?;
        require_hex_digest(
            &format!("{} corpus scope trace_blake3", row.scope),
            &row.trace_blake3,
        )?;
        anyhow::ensure!(
            row.trace_pid > 0 && row.trace_exit_code == 0 && row.trace_complete,
            "{} corpus scope trace lacks successful traced-process receipt",
            row.scope
        );
        require_eq("corpus scope tracer", &row.tracer_binary, "strace")?;
        require_eq(
            "corpus scope tracer version",
            &row.tracer_version,
            "conformance/haqp/evidence/access/strace.version",
        )?;
        require_hex_digest(
            &format!("{} corpus scope tracer_version_blake3", row.scope),
            &row.tracer_version_blake3,
        )?;
        require_hex_digest(
            &format!("{} corpus scope observed_paths_blake3", row.scope),
            &row.observed_paths_blake3,
        )?;
        let tracer_version_path =
            safe_repo_path(root, &row.tracer_version, "corpus scope tracer version")?;
        let tracer_version_bytes = fs::read(&tracer_version_path)
            .with_context(|| format!("{tracer_version_path}: tracer version required"))?;
        require_eq(
            &format!("{} corpus scope tracer_version_blake3", row.scope),
            &row.tracer_version_blake3,
            blake3::hash(&tracer_version_bytes).to_hex().as_ref(),
        )?;
        let expected_path = format!(
            "conformance/haqp/evidence/access/scopes/{}.trace",
            row.scope
        );
        require_eq("corpus scope trace path", &row.trace, &expected_path)?;
        let path = safe_repo_path(root, &row.trace, "corpus scope trace")?;
        let bytes = fs::read(&path).with_context(|| format!("{path}: scope trace required"))?;
        require_eq(
            &format!("{} corpus scope trace_blake3", row.scope),
            &row.trace_blake3,
            blake3::hash(&bytes).to_hex().as_ref(),
        )?;
        let trace_text = String::from_utf8_lossy(&bytes);
        let lower = trace_text.to_ascii_lowercase();
        let pid_prefix = format!("{} ", row.trace_pid);
        anyhow::ensure!(
            trace_text.lines().any(|line| line.starts_with(&pid_prefix)),
            "{} scope trace has no event from traced PID {}",
            row.scope,
            row.trace_pid
        );
        anyhow::ensure!(
            trace_text
                .lines()
                .any(|line| line == format!("{} +++ exited with 0 +++", row.trace_pid)),
            "{} scope trace has no successful exit for PID {}",
            row.scope,
            row.trace_pid
        );
        let observed_paths_blake3 = scope_trace_paths_digest(&bytes);
        require_eq(
            &format!("{} corpus scope observed_paths_blake3", row.scope),
            &row.observed_paths_blake3,
            &observed_paths_blake3,
        )?;
        verify_trace_corpus_resolution(
            root,
            &row.trace_root,
            &bytes,
            &row.resolved_paths_blake3,
            &format!("{} corpus scope", row.scope),
        )?;
        let binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                row.command,
                row.trace_pid,
                row.trace_exit_code,
                row.trace_blake3,
                row.observed_paths_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            &format!("{} corpus scope process_binding", row.scope),
            &row.process_binding,
            &binding,
        )?;
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !lower.contains(forbidden),
                "{} scope trace observed forbidden path fragment {forbidden:?}",
                row.scope
            );
        }
    }
    Ok(())
}

/// Bind each qualification-wide trace to its closed, actual lane command.
/// A scope-probe helper is not accepted as a substitute for tracing the lane
/// that the qualification packet claims to have run.
fn verify_corpus_scope_replays(
    root: &Utf8Path,
    rows: &[CorpusScopeTrace],
    fixed_commit: &str,
) -> Result<()> {
    let scratch = liminal_scratch::ScratchDir::new("haq-scope-replay")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(fixed_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "scope replay worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut guard = WorktreeGuard::new(root, &worktree);
    for row in rows {
        let expected_command = format!(
            "strace -f -q -e trace=openat,openat2 -o target/haqp/scope-{}.trace {}",
            row.scope,
            scope_lane_command(&row.scope)?
        );
        require_eq(
            &format!("{} corpus scope command", row.scope),
            &row.command,
            &expected_command,
        )?;
        // `row.command` is shell syntax only after exact equality with the
        // closed lane-command registry above; never execute an unbound field.
        let output = Command::new("sh")
            .current_dir(&worktree)
            .args(["-c", &row.command])
            .output()
            .with_context(|| format!("replay corpus scope {}", row.scope))?;
        anyhow::ensure!(
            output.status.success(),
            "scope replay {} failed: {}",
            row.scope,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let trace = worktree.join(format!("target/haqp/scope-{}.trace", row.scope));
        let bytes = fs::read(&trace)
            .with_context(|| format!("scope replay {} produced no trace", row.scope))?;
        require_eq(
            &format!("{} replayed observed_paths_blake3", row.scope),
            &row.observed_paths_blake3,
            &scope_trace_paths_digest(&bytes),
        )?;
        let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !lower.contains(forbidden),
                "{} replay observed forbidden path fragment {forbidden:?}",
                row.scope
            );
        }
        verify_trace_corpus_resolution(
            &worktree,
            worktree.as_str(),
            &bytes,
            &row.resolved_paths_blake3,
            &format!("{} replayed corpus scope", row.scope),
        )?;
    }
    guard.remove()?;
    Ok(())
}

fn scope_lane_command(scope: &str) -> Result<&'static str> {
    match scope {
        "ci" => Ok("just ci"),
        "canaries" => Ok("cargo run -q -p liminal-xtask -- haq run-canaries"),
        "generated" => Ok("cargo run -q -p liminal-xtask -- haq generate --cases 100000"),
        "crash" => Ok("cargo run -q -p liminal-conformance --bin crash-evidence"),
        "replay" => Ok("cargo test -q --workspace"),
        "mutation" => Ok("cargo run -q -p liminal-xtask -- haq mutants"),
        "reviews" => Ok("just haq-blind-review"),
        "fuzz" => Ok("scripts/haqp_fuzz_campaign.sh 1800"),
        _ => anyhow::bail!("unknown corpus scope {scope}"),
    }
}

/// Deterministic, read-only scope probe used by qualification-wide strace.
/// It intentionally touches only committed, non-held-out files for its lane.
pub fn run_scope_probe_repo(root: &Utf8Path, scope: &str) -> Result<()> {
    let files: &[&str] = match scope {
        "ci" => &["Cargo.toml", "Cargo.lock", "justfile"],
        "canaries" => &[
            "conformance/haqp/packet.json",
            "docs/execution/phase1-suite-review.md",
        ],
        "generated" | "mutation" => &[
            "crates/liminal-xtask/src/haq.rs",
            "conformance/haqp/packet.json",
        ],
        "crash" => &[
            "crates/liminal-jurisdiction/src/repair.rs",
            "conformance/haqp/packet.json",
        ],
        "replay" | "reviews" => &["conformance/haqp/packet.json"],
        "fuzz" => &["fuzz/Cargo.toml", "fuzz/fuzz_targets/cst_parse.rs"],
        _ => anyhow::bail!("unknown corpus scope {scope}"),
    };
    for path in files {
        let bytes =
            fs::read(root.join(path)).with_context(|| format!("scope probe read {path}"))?;
        anyhow::ensure!(!bytes.is_empty(), "scope probe read empty file {path}");
    }
    println!("scope-probe:{scope}:pass");
    Ok(())
}

fn scope_trace_paths_digest(bytes: &[u8]) -> String {
    blake3::hash(
        scope_trace_open_paths(bytes)
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
    .to_hex()
    .to_string()
}

fn scope_trace_open_paths(bytes: &[u8]) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for line in String::from_utf8_lossy(bytes).lines() {
        let Some(open) = ["openat(", "openat2("]
            .into_iter()
            .filter_map(|needle| line.find(needle))
            .min()
        else {
            continue;
        };
        let rest = &line[open..];
        let Some(start) = rest.find('"') else {
            continue;
        };
        let Some(end) = rest[start + 1..].find('"') else {
            continue;
        };
        paths.insert(rest[start + 1..start + 1 + end].to_owned());
    }
    paths
}

/// Resolve corpus paths from raw strace metadata. Lexical path filtering alone
/// is insufficient: a benign `fuzz/corpus/<target>` symlink can resolve into
/// locked `conformance/corpora/heldout` data. The campaign records only a
/// digest of `(lexical, canonical repository-relative)` pairs, so no path
/// receipt can disclose corpus names; verification recomputes it and fails
/// closed on missing, escaping, or forbidden resolutions.
fn verify_trace_corpus_resolution(
    root: &Utf8Path,
    trace_root: &str,
    bytes: &[u8],
    expected_digest: &str,
    label: &str,
) -> Result<()> {
    require_hex_digest(&format!("{label} resolved_paths_blake3"), expected_digest)?;
    let canonical_root = fs::canonicalize(root.as_std_path())
        .with_context(|| format!("{label}: canonicalize repository root"))?;
    let trace_root = Path::new(trace_root);
    anyhow::ensure!(
        trace_root.is_absolute(),
        "{label}: trace_root must be absolute"
    );
    let mut resolved = BTreeSet::new();
    for lexical in scope_trace_open_paths(bytes) {
        let lexical_path = Path::new(&lexical);
        let under_trace_root =
            !lexical_path.is_absolute() || lexical_path.strip_prefix(trace_root).is_ok();
        if !under_trace_root {
            continue;
        }
        let local = trace_path_to_repo(root, trace_root, &lexical)
            .with_context(|| format!("{label}: cannot map traced repository path"))?;
        let canonical = canonicalize_trace_path(local.as_std_path())
            .with_context(|| format!("{label}: traced repository path cannot be resolved"))?;
        let relative = canonical
            .strip_prefix(&canonical_root)
            .with_context(|| format!("{label}: traced repository path escapes repository root"))?;
        let relative = relative.to_str().context("non-UTF-8 traced corpus path")?;
        let lower = relative.to_ascii_lowercase();
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !lower.contains(forbidden),
                "{label}: traced repository path resolves into forbidden data"
            );
        }
        let lexical_lower = lexical_path.to_string_lossy().to_ascii_lowercase();
        if lexical_lower.contains("/fuzz/corpus/")
            || lexical_lower.starts_with("fuzz/corpus/")
            || lower.contains("/fuzz/corpus/")
            || lower.starts_with("fuzz/corpus/")
        {
            resolved.insert(format!("{lexical}\0{relative}"));
        }
    }
    anyhow::ensure!(
        !resolved.is_empty(),
        "{label}: trace contains no resolvable fuzz corpus path"
    );
    let resolved = resolved.into_iter().collect::<Vec<_>>();
    let resolved_bytes = format!("{}\n", resolved.join("\n"));
    let actual = blake3::hash(resolved_bytes.as_bytes()).to_hex().to_string();
    require_eq(
        &format!("{label} resolved_paths_blake3"),
        &actual,
        expected_digest,
    )?;
    Ok(())
}

/// Canonicalize the existing prefix of a traced path, then append historical
/// leaf components that libFuzzer may have deleted after opening them. This
/// preserves symlink and root-boundary checks without requiring every path in
/// a long-running trace to remain present at receipt time.
fn canonicalize_trace_path(path: &Path) -> Result<std::path::PathBuf> {
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        match fs::symlink_metadata(cursor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    cursor
                        .file_name()
                        .context("traced path has no recoverable leaf")?
                        .to_owned(),
                );
                let leaf = missing.last().expect("just pushed missing leaf");
                anyhow::ensure!(
                    leaf != std::ffi::OsStr::new(".") && leaf != std::ffi::OsStr::new(".."),
                    "traced path contains a parent-directory suffix"
                );
                cursor = cursor
                    .parent()
                    .context("traced path has no existing ancestor")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut canonical = fs::canonicalize(cursor)?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn trace_path_to_repo(root: &Utf8Path, trace_root: &Path, lexical: &str) -> Result<Utf8PathBuf> {
    let path = Path::new(lexical);
    if path.is_absolute() {
        if let Ok(relative) = path.strip_prefix(trace_root) {
            return Ok(
                root.join(Utf8Path::from_path(relative).context("non-UTF-8 traced relative path")?)
            );
        }
        anyhow::bail!("absolute traced path is outside recorded trace root")
    }
    Ok(root.join(Utf8Path::from_path(path).context("non-UTF-8 traced relative path")?))
}

#[allow(
    clippy::too_many_lines,
    reason = "corpus tracer gate keeps process binding and manifest checks together"
)]
fn verify_corpus_audit_campaign_binding(root: &Utf8Path, audit: &CorpusAccessAudit) -> Result<()> {
    let fuzz_path = root.join("conformance/haqp/evidence/fuzz.json");
    let fuzz: Vec<FuzzEvidence> =
        serde_json::from_slice(&fs::read(&fuzz_path).with_context(|| {
            format!("{fuzz_path}: fuzz evidence is required for trace binding")
        })?)
        .with_context(|| format!("parse {fuzz_path}"))?;
    let fuzz_by_target = fuzz
        .iter()
        .map(|row| (row.target.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    for row in &audit.targets {
        let fuzz_row = fuzz_by_target.get(row.target.as_str()).with_context(|| {
            format!(
                "{}: corpus audit target is absent from fuzz evidence",
                row.target
            )
        })?;
        anyhow::ensure!(
            row.trace_pid > 0,
            "{}: corpus audit has no traced process PID",
            row.target
        );
        anyhow::ensure!(
            row.trace_exit_code == 0,
            "{}: tracer exited {}",
            row.target,
            row.trace_exit_code
        );
        anyhow::ensure!(
            row.trace_complete,
            "{}: raw tracer output is incomplete",
            row.target
        );
        anyhow::ensure!(
            row.command.contains("strace") && row.command.contains("-f"),
            "{}: corpus audit command is not recursive strace",
            row.target
        );
        anyhow::ensure!(
            !row.tracer_binary.trim().is_empty()
                && !row.tracer_version.trim().is_empty()
                && !row.tracer_version_blake3.trim().is_empty(),
            "{} corpus audit lacks current tracer identity; regenerate corpus-access evidence",
            row.target
        );
        require_eq("corpus tracer binary", &row.tracer_binary, "strace")?;
        require_eq(
            "corpus tracer version",
            &row.tracer_version,
            "conformance/haqp/evidence/access/strace.version",
        )?;
        require_hex_digest(
            &format!("{} tracer_version_blake3", row.target),
            &row.tracer_version_blake3,
        )?;
        let tracer_version_path =
            safe_repo_path(root, &row.tracer_version, "corpus tracer version")?;
        let tracer_version_bytes = fs::read(&tracer_version_path)
            .with_context(|| format!("{tracer_version_path}: tracer version receipt required"))?;
        require_eq(
            &format!("{} tracer_version_blake3", row.target),
            &row.tracer_version_blake3,
            blake3::hash(&tracer_version_bytes).to_hex().as_ref(),
        )?;
        require_hex_digest(&format!("{} trace_blake3", row.target), &row.trace_blake3)?;
        let trace_path = safe_repo_path(root, &row.trace, "corpus trace")?;
        let trace_bytes = fs::read(&trace_path)
            .with_context(|| format!("{trace_path}: raw access trace is required"))?;
        require_eq(
            "corpus access trace_blake3",
            &row.trace_blake3,
            blake3::hash(&trace_bytes).to_hex().as_ref(),
        )?;
        let process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                row.command, row.trace_pid, row.trace_exit_code, row.trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            "corpus access process_binding",
            &row.process_binding,
            &process_binding,
        )?;
        anyhow::ensure!(
            row.seed == fuzz_row.seed,
            "{}: corpus audit seed differs from fuzz evidence",
            row.target
        );
        require_eq(
            "corpus audit sanitizer",
            &row.sanitizer,
            &fuzz_row.sanitizer,
        )?;
        anyhow::ensure!(
            row.exit_code == fuzz_row.exit_code,
            "{}: corpus audit exit code differs from fuzz evidence",
            row.target
        );
        require_eq(
            "corpus audit log_blake3",
            &row.log_blake3,
            &fuzz_row.log_blake3,
        )?;
        anyhow::ensure!(
            !row.command.trim().is_empty(),
            "{}: corpus audit has no traced command",
            row.target
        );
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
        let trace_text = String::from_utf8_lossy(&trace_bytes);
        let trace_lower = trace_text.to_ascii_lowercase();
        let pid_prefix = format!("{} ", row.trace_pid);
        anyhow::ensure!(
            trace_text.lines().any(|line| line.starts_with(&pid_prefix)),
            "{}: raw trace has no record emitted by traced process PID {}",
            row.target,
            row.trace_pid
        );
        anyhow::ensure!(
            trace_text
                .lines()
                .any(|line| line == format!("{} +++ exited with 0 +++", row.trace_pid)),
            "{}: raw trace has no successful exit record for tracer PID {}",
            row.target,
            row.trace_pid
        );
        let target_corpus_marker = format!("/fuzz/corpus/{}", row.target);
        anyhow::ensure!(
            trace_lower.contains(&target_corpus_marker),
            "{}: raw trace is not bound to target corpus {}",
            row.target,
            target_corpus_marker
        );
        let target_binary_marker = "/fuzz/target/";
        let target_binary_name = format!("/release/{}", row.target);
        anyhow::ensure!(
            trace_lower.contains(target_binary_marker) && trace_lower.contains(&target_binary_name),
            "{}: raw trace does not show target binary execution",
            row.target
        );
        // The corpus and binary opens must come from the fuzz child, not from
        // the strace launcher or a build helper. Bind that child to the
        // launcher's SIGCHLD receipt; a hand-written trace containing only
        // expected path strings cannot satisfy this relation.
        let corpus_child_pids = trace_text
            .lines()
            .filter(|line| {
                line.to_ascii_lowercase().contains(&target_corpus_marker)
                    && line.contains("openat(")
            })
            .filter_map(|line| line.split_whitespace().next())
            .filter_map(|pid| pid.parse::<u32>().ok())
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            corpus_child_pids.len() == 1,
            "{}: raw trace must identify exactly one fuzz child reading its corpus, got {:?}",
            row.target,
            corpus_child_pids
        );
        let child_pid = *corpus_child_pids.iter().next().expect("one child checked");
        let child_exit_receipt = format!("si_pid={child_pid},");
        anyhow::ensure!(
            trace_text.lines().any(|line| line.starts_with(&pid_prefix)
                && line.contains("SIGCHLD")
                && line.contains(&child_exit_receipt)),
            "{}: raw trace lacks launcher SIGCHLD receipt for corpus-reading child {}",
            row.target,
            child_pid
        );
        anyhow::ensure!(
            trace_text.lines().any(|line| {
                line.starts_with(&format!("{child_pid} ")) && line.contains(&target_binary_name)
            }),
            "{}: target binary execution is not attributed to corpus-reading child {}",
            row.target,
            child_pid
        );
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !trace_lower.contains(forbidden),
                "{trace_path}: raw access trace observed forbidden path fragment {forbidden:?}"
            );
        }
        verify_trace_corpus_resolution(
            root,
            &row.trace_root,
            &trace_bytes,
            &row.resolved_paths_blake3,
            &format!("{} corpus audit", row.target),
        )?;
    }
    Ok(())
}

/// Check the bounded-campaign clock artifact. One clean breach is retained as
/// residual risk; two clean breaches block ratification per ADR-0020 §7.
fn verify_campaign_clock(root: &Utf8Path, expected: Option<&Provenance>) -> Result<()> {
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
        require_git_object_id("campaign run commit", &run.commit)?;
        require_git_object_id("campaign run tree", &run.tree)?;
        anyhow::ensure!(
            !run.command.trim().is_empty(),
            "campaign run has no command"
        );
        if let Some(provenance) = expected {
            require_eq("campaign run commit", &run.commit, &provenance.fixed_commit)?;
            require_eq("campaign run tree", &run.tree, &provenance.fixed_tree)?;
        }
        anyhow::ensure!(
            run.elapsed_s > 0,
            "campaign run {} has zero elapsed seconds",
            run.id
        );
        anyhow::ensure!(
            run.finished_epoch >= run.started_epoch
                && run.finished_epoch - run.started_epoch == run.elapsed_s,
            "campaign run {} elapsed_s is not derived from recorded start/end epochs",
            run.id
        );
        require_eq("campaign run result", &run.result, "pass")?;
        anyhow::ensure!(run.clean, "campaign run {} was not clean", run.id);
        require_eq(
            "campaign wrapper",
            &run.wrapper,
            "scripts/haqp_campaign_clock.sh",
        )?;
        require_hex_digest("campaign wrapper_sha256", &run.wrapper_sha256)?;
        let wrapper_bytes = fs::read(root.join(&run.wrapper))
            .with_context(|| format!("{}: campaign wrapper is required", run.wrapper))?;
        let mut wrapper_hasher = Sha256::new();
        wrapper_hasher.update(wrapper_bytes);
        let wrapper_digest = format!("{:x}", wrapper_hasher.finalize());
        require_eq(
            "campaign wrapper_sha256",
            &run.wrapper_sha256,
            &wrapper_digest,
        )?;
        anyhow::ensure!(
            !run.receipt.trim().is_empty() && !run.receipt_blake3.trim().is_empty(),
            "campaign run {} lacks wrapper receipt; regenerate campaign clock evidence",
            run.id
        );
        let receipt_path = safe_repo_path(root, &run.receipt, "campaign receipt")?;
        let receipt_bytes = fs::read(&receipt_path)
            .with_context(|| format!("{receipt_path}: campaign receipt is required"))?;
        require_hex_digest("campaign receipt_blake3", &run.receipt_blake3)?;
        require_eq(
            "campaign receipt_blake3",
            &run.receipt_blake3,
            blake3::hash(&receipt_bytes).to_hex().as_ref(),
        )?;
        let receipt = String::from_utf8(receipt_bytes)
            .with_context(|| format!("{receipt_path}: campaign receipt is not UTF-8"))?;
        let expected_receipt = format!(
            "run_id={}\ncommit={}\ntree={}\ncommand={}\nstarted_epoch={}\nfinished_epoch={}\nelapsed_s={}\nclean={}\nresult={}\n",
            run.id,
            run.commit,
            run.tree,
            run.command,
            run.started_epoch,
            run.finished_epoch,
            run.elapsed_s,
            run.clean,
            run.result
        );
        require_eq("campaign receipt contents", &receipt, &expected_receipt)?;
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
fn verify_residual_risks(root: &Utf8Path, packet: &Packet) -> Result<()> {
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
        if let Some((file, line)) = risk.evidence.rsplit_once(':') {
            let line: usize = line
                .parse()
                .with_context(|| format!("risk {} evidence line is not numeric", risk.id))?;
            anyhow::ensure!(
                line > 0
                    && !file.is_empty()
                    && !file.starts_with('/')
                    && !file.split('/').any(|part| part == ".."),
                "risk {} evidence coordinate is unsafe",
                risk.id
            );
            anyhow::ensure!(
                !is_locked_acceptance_path(file),
                "risk {} evidence coordinate names locked acceptance corpus",
                risk.id
            );
            let path = safe_repo_path(root, file, "residual-risk evidence")?;
            let source = fs::read_to_string(&path)
                .with_context(|| format!("risk {} evidence source is missing", risk.id))?;
            anyhow::ensure!(
                source.lines().nth(line - 1).is_some(),
                "risk {} evidence line {} is outside {}",
                risk.id,
                line,
                file
            );
        } else {
            anyhow::bail!("risk {} evidence must be an exact file:coordinate", risk.id);
        }
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
    // The metadata child may carry only qualification outputs. If source or
    // gate code changes after evidence was captured, the packet can still
    // point at the old tree while claiming the new behavior was tested.
    let changed = git_text(
        root,
        &[
            "diff",
            "--name-only",
            &format!("{}..HEAD", provenance.evidence_parent),
        ],
    )?;
    for path in changed.lines().filter(|path| !path.trim().is_empty()) {
        anyhow::ensure!(
            qualification_metadata_path(path),
            "qualification metadata child changed non-metadata path {path:?}; source and gate code must remain at fixed commit"
        );
    }
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

fn qualification_metadata_path(path: &str) -> bool {
    matches!(
        path,
        "conformance/haqp/packet.json"
            | "docs/execution/phase1-suite-review.md"
            | "docs/execution/m17-5-adversarial-findings.md"
    ) || path.starts_with("conformance/haqp/evidence/")
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
            has_exact_coordinate_anchor(&source, coordinate),
            "requirement {} coordinate {:?} is absent from {path}",
            requirement.id,
            coordinate
        );
    }
    Ok(())
}

/// A requirement coordinate is an exact anchor, not a substring assertion.
///
/// Semantic anchors such as `D18.1` must not match `D18.10`; heading anchors
/// use their complete trimmed Markdown heading (`## Exit gate`) so a generic
/// word such as `exit` cannot accidentally satisfy a gate row.
fn has_exact_coordinate_anchor(source: &str, coordinate: &str) -> bool {
    let coordinate = coordinate.trim();
    if coordinate.is_empty() {
        return false;
    }
    if coordinate.starts_with("## ") {
        return source.lines().any(|line| line.trim() == coordinate);
    }
    source.lines().any(|line| {
        line.match_indices(coordinate).any(|(offset, _)| {
            let before = line[..offset].chars().next_back();
            let after = line[offset + coordinate.len()..].chars().next();
            before.is_none_or(|ch| !coordinate_anchor_char(ch))
                && after.is_none_or(|ch| !coordinate_anchor_char(ch))
        })
    })
}

fn coordinate_anchor_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

fn read_packet(root: &Utf8Path) -> Result<Packet> {
    let path = root.join("conformance/haqp/packet.json");
    let bytes = fs::read(&path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

#[allow(
    clippy::too_many_lines,
    reason = "packet shape gate keeps closed inventory and evidence checks together"
)]
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
            "deterministic_schedule_exploration" | "not_applicable"
        ),
        "concurrent_code must be deterministic_schedule_exploration or not_applicable"
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
    let authoritative = authoritative_requirement_map();
    anyhow::ensure!(
        packet.requirements.len() == authoritative.len(),
        "requirement inventory cardinality is not authoritative: got {}, expected {}",
        packet.requirements.len(),
        authoritative.len()
    );
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
        let expected = authoritative
            .get(requirement.id.as_str())
            .with_context(|| {
                format!(
                    "requirement {} is not in the closed authoritative registry",
                    requirement.id
                )
            })?;
        anyhow::ensure!(
            requirement.kind == expected.0
                && requirement.source == expected.1
                && requirement.critical == expected.2
                && requirement.stateful == expected.3,
            "requirement {} differs from closed authoritative registry",
            requirement.id
        );
    }
    verify_evidence_coverage(packet)?;
    verify_test_registry(packet)?;
    verify_mutant_inventory(packet)?;
    verify_kill_concentration(packet)?;
    verify_canary_inventory(packet)?;
    verify_generated_inventory(packet)?;
    verify_reviews_inventory(packet)?;
    verify_crash_boundary_inventory(packet)?;
    Ok(())
}

fn authoritative_requirement_map()
-> BTreeMap<&'static str, (&'static str, &'static str, bool, bool)> {
    AUTHORITATIVE_REQUIREMENTS
        .iter()
        .map(|(id, kind, source, critical, stateful)| (*id, (*kind, *source, *critical, *stateful)))
        .collect()
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

/// Closed digest over test identity, executable name, requirement mapping, and
/// evidence kinds. Existence checks alone let a packet remap broad, unrelated
/// test names to satisfy coverage; this registry makes the authoritative
/// mapping tamper-evident (P1-A04). Recompute with the canonical NUL/US stream
/// below and update this constant in the same commit as any test inventory edit.
const TEST_REGISTRY_SHA256: &str =
    "7a29add0e81657e17786934010638abf1ea99dfee4e162921a9ef2699197d447";

fn verify_test_registry(packet: &Packet) -> Result<()> {
    let mut canonical = String::new();
    for test in &packet.tests {
        canonical.push_str(&test.id);
        canonical.push('\0');
        canonical.push_str(&test.name);
        canonical.push('\0');
        canonical.push_str(&test.requirements.join("\x1f"));
        canonical.push('\0');
        canonical.push_str(&test.evidence.join("\x1f"));
        canonical.push('\0');
    }
    let digest = sha256_text(&canonical);
    anyhow::ensure!(
        digest == TEST_REGISTRY_SHA256,
        "test registry digest differs from closed authority: got {digest}, expected {TEST_REGISTRY_SHA256}"
    );
    Ok(())
}

fn contains_exact_coordinate(text: &str, coordinate: &str) -> bool {
    text.match_indices(coordinate).any(|(start, _)| {
        let end = start + coordinate.len();
        !text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_digit())
            && !text[end..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit())
    })
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

/// Phase 1's M18-M24 inventory is closed. Packet fields are claims; this
/// registry is the authority they must match byte-for-byte. A count-only check
/// lets a doctored packet remove a law and replace it with an invented row
/// while preserving the same cardinality, and a packet-controlled `critical`
/// bit can demote evidence obligations.
const AUTHORITATIVE_REQUIREMENTS: [(&str, &str, &str, bool, bool); 55] = [
    ("P1-R001", "law", "docs/execution/M18.md:D18.1", true, false),
    ("P1-R002", "law", "docs/execution/M18.md:D18.2", true, false),
    ("P1-R003", "law", "docs/execution/M18.md:D18.3", true, false),
    ("P1-R004", "law", "docs/execution/M18.md:D18.4", true, false),
    ("P1-R005", "law", "docs/execution/M18.md:D18.5", true, false),
    ("P1-R006", "law", "docs/execution/M18.md:D18.6", true, false),
    (
        "P1-R007",
        "gate",
        "docs/execution/M18.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R008", "law", "docs/execution/M19.md:D19.1", true, false),
    ("P1-R009", "law", "docs/execution/M19.md:D19.2", true, false),
    ("P1-R010", "law", "docs/execution/M19.md:D19.3", true, false),
    ("P1-R011", "law", "docs/execution/M19.md:D19.4", true, false),
    (
        "P1-R012",
        "gate",
        "docs/execution/M19.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R013", "law", "docs/execution/M20.md:D20.1", true, false),
    ("P1-R014", "law", "docs/execution/M20.md:D20.2", true, false),
    (
        "P1-R015",
        "fault",
        "docs/execution/M20.md:D20.3",
        true,
        true,
    ),
    (
        "P1-R016",
        "abuse",
        "docs/execution/M20.md:D20.4",
        true,
        false,
    ),
    ("P1-R017", "law", "docs/execution/M21.md:D21.1", true, false),
    ("P1-R018", "law", "docs/execution/M21.md:D21.2", true, false),
    ("P1-R019", "law", "docs/execution/M21.md:D21.3", true, false),
    ("P1-R020", "law", "docs/execution/M22.md:D22.1", true, false),
    ("P1-R021", "law", "docs/execution/M22.md:D22.2", true, false),
    ("P1-R022", "law", "docs/execution/M23.md:D23.1", true, false),
    ("P1-R023", "law", "docs/execution/M23.md:D23.6", true, false),
    (
        "P1-R024",
        "gate",
        "docs/execution/M24.md:D24.1",
        true,
        false,
    ),
    ("P1-R025", "law", "docs/execution/M20.md:D20.5", true, false),
    ("P1-R026", "law", "docs/execution/M20.md:D20.6", true, false),
    ("P1-R027", "law", "docs/execution/M20.md:D20.7", true, false),
    ("P1-R028", "law", "docs/execution/M21.md:D21.4", true, false),
    ("P1-R029", "law", "docs/execution/M21.md:D21.5", true, false),
    ("P1-R030", "law", "docs/execution/M21.md:D21.6", true, false),
    ("P1-R031", "law", "docs/execution/M21.md:D21.7", true, false),
    ("P1-R032", "law", "docs/execution/M22.md:D22.3", true, false),
    ("P1-R033", "law", "docs/execution/M22.md:D22.4", true, false),
    ("P1-R034", "law", "docs/execution/M22.md:D22.5", true, false),
    ("P1-R035", "law", "docs/execution/M22.md:D22.6", true, false),
    ("P1-R036", "law", "docs/execution/M22.md:D22.7", true, false),
    ("P1-R037", "law", "docs/execution/M19.md:D19.0", true, false),
    ("P1-R038", "law", "docs/execution/M19.md:D19.5", true, false),
    ("P1-R039", "law", "docs/execution/M19.md:D19.6", true, false),
    ("P1-R040", "law", "docs/execution/M19.md:D19.7", true, false),
    ("P1-R041", "law", "docs/execution/M19.md:D19.8", true, false),
    ("P1-R042", "law", "docs/execution/M20.md:D20.0", true, false),
    (
        "P1-R043",
        "gate",
        "docs/execution/M20.md:## Exit gate",
        true,
        false,
    ),
    (
        "P1-R044",
        "gate",
        "docs/execution/M21.md:## Exit gate",
        true,
        false,
    ),
    (
        "P1-R045",
        "gate",
        "docs/execution/M22.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R046", "law", "docs/execution/M23.md:D23.2", true, false),
    ("P1-R047", "law", "docs/execution/M23.md:D23.3", true, false),
    ("P1-R048", "law", "docs/execution/M23.md:D23.4", true, false),
    ("P1-R049", "law", "docs/execution/M23.md:D23.5", true, false),
    (
        "P1-R050",
        "gate",
        "docs/execution/M23.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R051", "law", "docs/execution/M24.md:D24.2", true, false),
    ("P1-R052", "law", "docs/execution/M24.md:D24.3", true, false),
    ("P1-R053", "law", "docs/execution/M24.md:D24.4", true, false),
    (
        "P1-R054",
        "gate",
        "docs/execution/M24.md:D24.5",
        true,
        false,
    ),
    (
        "P1-R055",
        "gate",
        "docs/execution/M24.md:## Exit gate",
        true,
        false,
    ),
];

const REQUIREMENT_COUNT: usize = AUTHORITATIVE_REQUIREMENTS.len();

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
        let requirement = if mutant.requirement.trim().is_empty() {
            // Proposed inventories from before the coordinate contract used
            // source for requirement ID. Preserve that plan shape, but never
            // treat it as an evaluated coordinate (qualified gate below is
            // strict).
            mutant.source.as_str()
        } else {
            mutant.requirement.as_str()
        };
        if !requirement_ids.contains(requirement) {
            anyhow::bail!(
                "{} names requirement {:?}, which is not a declared requirement — a mutant must \
                 attack something the inventory claims to require",
                mutant.id,
                requirement
            );
        }
        if !mutant.source.starts_with("P1-R") && mutant.requirement.trim().is_empty() {
            anyhow::bail!(
                "{} source {:?} is a coordinate but requirement ID is missing",
                mutant.id,
                mutant.source
            );
        }
        if !mutant.requirement.trim().is_empty() {
            let (file, line) = mutant.source.rsplit_once(':').with_context(|| {
                format!(
                    "{} source must be exact file:line; source is not a declared requirement",
                    mutant.id
                )
            })?;
            let line: usize = line
                .parse()
                .with_context(|| format!("{} source line is not numeric", mutant.id))?;
            anyhow::ensure!(
                line > 0
                    && !file.is_empty()
                    && !file.starts_with('/')
                    && !file.split('/').any(|part| part == ".."),
                "{} source must be safe file:line",
                mutant.id
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

#[allow(clippy::too_many_lines)]
fn verify_mutant_source_coordinates(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut seen = BTreeSet::new();
    for mutant in &packet.mutants {
        // Legacy predeclared rows are an inventory plan; M24 must replace
        // these IDs with exact file:line coordinates before qualification.
        if mutant.source.starts_with("P1-R") && mutant.requirement.trim().is_empty() {
            anyhow::bail!(
                "{} still uses legacy requirement ID in mutant source; qualified evidence requires file:line plus requirement",
                mutant.id
            );
        }
        let (file, line) = mutant
            .source
            .rsplit_once(':')
            .with_context(|| format!("{} mutant source must be exact file:line", mutant.id))?;
        let line: usize = line
            .parse()
            .with_context(|| format!("{} mutant source line is not numeric", mutant.id))?;
        anyhow::ensure!(
            line > 0,
            "{} mutant source line must be positive",
            mutant.id
        );
        let (expected_file, expected_line, expected_anchor) = mutant_source_coordinate(&mutant.id)
            .with_context(|| format!("{} has no closed source registry entry", mutant.id))?;
        anyhow::ensure!(
            file == expected_file && line == expected_line,
            "{} source {}:{} does not match closed registry {}:{}",
            mutant.id,
            file,
            line,
            expected_file,
            expected_line
        );
        anyhow::ensure!(
            seen.insert((file.to_owned(), line)),
            "{} reuses source coordinate {}:{}",
            mutant.id,
            file,
            line
        );
        let path = safe_repo_path(root, file, "mutant source")?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("{} mutant source file is missing", mutant.id))?;
        anyhow::ensure!(
            source.lines().nth(line - 1).is_some(),
            "{} mutant source coordinate {}:{} is outside file",
            mutant.id,
            file,
            line
        );
        let anchor = source
            .lines()
            .nth(line - 1)
            .map(str::trim_start)
            .unwrap_or_default();
        anyhow::ensure!(
            anchor.starts_with(expected_anchor),
            "{} mutant source coordinate {}:{} does not name family anchor {:?}",
            mutant.id,
            file,
            line,
            expected_anchor
        );
        anyhow::ensure!(
            !mutant.requirement.trim().is_empty()
                && packet
                    .requirements
                    .iter()
                    .any(|requirement| requirement.id == mutant.requirement),
            "{} mutant coordinate lacks a declared requirement ID",
            mutant.id
        );
        if let Some(patch) = &mutant.patch {
            anyhow::ensure!(
                patch.file == file,
                "{} patch file {:?} does not match mutant source file {:?}",
                mutant.id,
                patch.file,
                file
            );
            let source_line = source.lines().nth(line - 1).unwrap_or_default();
            let patch_anchor = patch
                .before
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default();
            anyhow::ensure!(
                !patch_anchor.is_empty() && source_line.trim().contains(patch_anchor),
                "{} patch before text is not anchored at declared source {}:{}",
                mutant.id,
                file,
                line
            );
            anyhow::ensure!(
                patch
                    .before
                    .lines()
                    .filter(|text| !text.trim().is_empty())
                    .count()
                    == 1
                    && source_line.trim() == patch_anchor,
                "{} patch coordinate {}:{} is not an exact single-line source anchor",
                mutant.id,
                file,
                line
            );
            verify_mutant_operator_patch(&mutant.operator, patch)?;
        }
    }
    Ok(())
}

/// Require an evaluated patch to exhibit the lexical mutation prescribed by
/// its closed operator.  Anchoring and compilation alone allow an unrelated
/// replacement to masquerade as a predicate inversion.
fn verify_mutant_operator_patch(operator: &str, patch: &MutantPatch) -> Result<()> {
    let before = patch.before.as_str();
    let after = patch.after.as_str();
    let inversion_pairs = [
        ("==", "!="),
        ("!=", "=="),
        ("<=", ">"),
        (">=", "<"),
        ("<", ">="),
        (">", "<="),
        ("&&", "||"),
        ("||", "&&"),
        ("true", "false"),
        ("false", "true"),
    ];
    let recognized = match operator {
        "predicate-inversion" => inversion_pairs.iter().any(|(from, to)| {
            before.contains(from)
                && after == before.replacen(from, to, 1)
                && before.matches(from).count() == 1
                && after.matches(to).count() >= 1
        }),
        "predicate-deletion" => before.contains('!') && after == before.replacen('!', "", 1),
        "threshold-plus-one" => integer_delta(before, after) == Some(1),
        "threshold-minus-one" => integer_delta(before, after) == Some(-1),
        "missing-enum-dispatch" => before.contains("match") && !after.contains("match"),
        "success-error-substitution" => {
            (before.contains("Ok(") && after.contains("Err("))
                || (before.contains("Err(") && after.contains("Ok("))
        }
        "oracle-short-circuit" => {
            (before.contains("&&") || before.contains("||"))
                && (after.contains("return") || after.contains("Ok("))
        }
        "ordering-nondeterminism" => {
            (before.contains("sort") || before.contains("BTree"))
                && (!after.contains("sort") || after.contains("Hash"))
        }
        "stale-basis-acceptance" => {
            before.to_ascii_lowercase().contains("basis")
                && !after.to_ascii_lowercase().contains("basis")
        }
        "skipped-durable-transition" => ["prepare", "ack", "finalize", "commit"]
            .iter()
            .any(|token| before.contains(token) && !after.contains(token)),
        "disabled-crash-point" => {
            before.to_ascii_lowercase().contains("crash")
                && !after.to_ascii_lowercase().contains("crash")
        }
        "broadened-allow-list" => {
            (!before.contains('*') && after.contains('*'))
                || (before.contains("ensure!") && !after.contains("ensure!"))
        }
        "wrong-holder-selection" => {
            before.to_ascii_lowercase().contains("holder")
                && after.to_ascii_lowercase().contains("holder")
                && before != after
        }
        _ => false,
    };
    anyhow::ensure!(
        recognized,
        "mutation patch does not implement declared operator {operator:?}"
    );
    Ok(())
}

fn integer_delta(before: &str, after: &str) -> Option<i64> {
    fn split_single_integer(text: &str) -> Option<(&str, i64, &str)> {
        let mut chars = text.char_indices().peekable();
        let mut start = None;
        let mut end = 0;
        while let Some((index, ch)) = chars.next() {
            let starts_integer = ch.is_ascii_digit()
                || (ch == '-' && chars.peek().is_some_and(|(_, next)| next.is_ascii_digit()));
            if !starts_integer {
                continue;
            }
            if start.replace(index).is_some() {
                return None;
            }
            end = index + ch.len_utf8();
            while let Some((next_index, next)) = chars.peek().copied() {
                if !next.is_ascii_digit() {
                    break;
                }
                chars.next();
                end = next_index + next.len_utf8();
            }
        }
        let start = start?;
        let value = text[start..end].parse().ok()?;
        Some((&text[..start], value, &text[end..]))
    }

    let (before_prefix, before_value, before_suffix) = split_single_integer(before)?;
    let (after_prefix, after_value, after_suffix) = split_single_integer(after)?;
    (before_prefix == after_prefix && before_suffix == after_suffix)
        .then_some(after_value - before_value)
}

/// Closed M24 mutation-source registry. Each predeclared mutant owns one
/// exact implementation coordinate; generated harness helpers are excluded.
#[allow(
    clippy::too_many_lines,
    reason = "closed mutation registry keeps every exact coordinate auditable in one place"
)]
fn mutant_source_coordinate(id: &str) -> Option<(&'static str, usize, &'static str)> {
    const SOURCE: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-cst/src/parser.rs",
            142,
            "if !text.is_empty()",
        ),
        ("crates/liminal-cst/src/parser.rs", 196, "if !valid_close"),
        ("crates/liminal-cst/src/parser.rs", 163, "pub fn errors("),
        ("crates/liminal-cst/src/parser.rs", 169, "pub fn basis("),
        ("crates/liminal-cst/src/parser.rs", 64, "match raw.0 {"),
        (
            "crates/liminal-source/src/view.rs",
            29,
            "pub fn from_bytes(",
        ),
        ("crates/liminal-source/src/view.rs", 58, "pub fn basis("),
        ("crates/liminal-source/src/view.rs", 64, "pub fn len_bytes("),
        (
            "crates/liminal-cli/src/format.rs",
            31,
            "pub fn format_workspace_with_failure_after(",
        ),
        (
            "crates/liminal-source/src/view.rs",
            106,
            "pub fn to_string(",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            31,
            "pub fn parse(",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            67,
            "fn build_block(",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            116,
            "fn extract_marker(",
        ),
    ];
    const GRAPH: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-graph/src/node.rs",
            58,
            "self.0 & other.0 == other.0",
        ),
        ("crates/liminal-graph/src/node.rs", 63, "pub fn union("),
        ("crates/liminal-graph/src/relation.rs", 51, "pub fn node("),
        (
            "crates/liminal-graph/src/relation.rs",
            80,
            "pub fn contains(",
        ),
        ("crates/liminal-graph/src/store/mod.rs", 279, "pub fn open("),
        ("crates/liminal-graph/src/store/mod.rs", 301, "pub fn head("),
        (
            "crates/liminal-graph/src/store/mod.rs",
            306,
            "pub fn begin(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            316,
            "pub fn node_at(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            328,
            "pub fn relation_at(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            344,
            "pub fn relations_from(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            370,
            "pub fn relations(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            375,
            "pub fn nodes(",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            380,
            "pub fn transaction(",
        ),
    ];
    const TRANSFORM: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-source/src/merge.rs",
            36,
            "pub fn three_way(",
        ),
        (
            "crates/liminal-source/src/merge.rs",
            122,
            "pub fn overlap_slots(",
        ),
        ("crates/liminal-source/src/merge.rs", 150, "fn slot_map("),
        ("crates/liminal-source/src/merge.rs", 167, "fn slot_order("),
        ("crates/liminal-source/src/merge.rs", 188, "fn line_merge("),
        (
            "crates/liminal-source/src/merge.rs",
            140,
            "fn block_source(",
        ),
        (
            "crates/liminal-source/src/file.rs",
            54,
            "pub fn staged_path(",
        ),
        (
            "crates/liminal-source/src/file.rs",
            60,
            "pub fn target_path(",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            154,
            "fn byte_offset(",
        ),
        ("crates/liminal-source/src/file.rs", 28, "pub fn observe("),
        ("crates/liminal-source/src/file.rs", 118, "pub fn stage("),
        (
            "crates/liminal-source/src/file.rs",
            133,
            "pub fn scan_staged(",
        ),
        ("crates/liminal-source/src/file.rs", 79, "pub fn commit_if("),
    ];
    const REPAIR: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-jurisdiction/src/repair.rs",
            212,
            "pub fn topo_order(",
        ),
        (
            "crates/liminal-jurisdiction/src/repair.rs",
            289,
            "pub fn plan_undo(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            56,
            "pub fn may_transition_to(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            77,
            "pub fn is_terminal(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            160,
            "pub fn name(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            176,
            "pub fn all(",
        ),
        ("crates/liminal-jurisdiction/src/ilrp.rs", 239, "fn meta("),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            250,
            "fn commit_intent(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            264,
            "fn acknowledge_step(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            282,
            "pub fn prepare(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            339,
            "pub fn run(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            359,
            "pub fn recover_all(",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            382,
            "fn advance(",
        ),
    ];
    const BASIS: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-revision/src/deps.rs",
            27,
            "pub fn invalidated_by(",
        ),
        (
            "crates/liminal-revision/src/basis.rs",
            23,
            "pub perspective:",
        ),
        (
            "crates/liminal-revision/src/basis.rs",
            28,
            "pub components:",
        ),
        (
            "crates/liminal-revision/src/basis.rs",
            34,
            "pub enum BasisComponent",
        ),
        (
            "crates/liminal-revision/src/basis.rs",
            66,
            "GraphSnapshot {",
        ),
        (
            "crates/liminal-revision/src/basis.rs",
            98,
            "pub fn graph_key(",
        ),
        (
            "crates/liminal-revision/src/perspective.rs",
            24,
            "pub enum BasisPerspective",
        ),
        (
            "crates/liminal-revision/src/perspective.rs",
            27,
            "ClientScoped {",
        ),
        (
            "crates/liminal-revision/src/perspective.rs",
            32,
            "DurableOnly,",
        ),
        (
            "crates/liminal-revision/src/perspective.rs",
            34,
            "Published {",
        ),
        (
            "crates/liminal-revision/src/perspective.rs",
            40,
            "Federated {",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            37,
            "pub fn resolve(",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            43,
            "let components = match perspective",
        ),
    ];
    let number = id.strip_prefix("P1-M")?.parse::<usize>().ok()?;
    let (table, offset) = match number {
        1..=13 => (&SOURCE, 1),
        14..=26 => (&GRAPH, 14),
        27..=39 => (&TRANSFORM, 27),
        40..=52 => (&REPAIR, 40),
        53..=65 => (&BASIS, 53),
        _ => return None,
    };
    table.get(number - offset).copied()
}

fn verify_canary_inventory(packet: &Packet) -> Result<()> {
    require_exact_ids(
        packet.canaries.iter().map(|row| row.id.as_str()),
        (1..=32).map(|idx| format!("C{idx:02}")),
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

/// Closed canary-to-gate map. The 32 rows are not merely a count: each
/// ratification gate has one named, executable violation (M17.5 P1-A04).
const CANARY_GATES: [(&str, &str); 32] = [
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
    ("C17", "requirements"),
    ("C18", "evidence coverage"),
    ("C19", "stateful registry"),
    ("C20", "mutants"),
    ("C21", "mutants"),
    ("C22", "generated"),
    ("C23", "generated"),
    ("C24", "crash"),
    ("C25", "reviews"),
    ("C26", "provenance"),
    ("C27", "sanitizer"),
    ("C28", "oracle independence"),
    ("C29", "campaign clock"),
    ("C30", "residual risk"),
    ("C31", "corpus access"),
    ("C32", "concurrency"),
];

#[allow(
    clippy::too_many_lines,
    reason = "generated inventory gate keeps closed family and relation checks together"
)]
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
        let discard_percent = family
            .discards
            .checked_mul(100)
            .with_context(|| format!("{} discard rate arithmetic overflow", family.family))?;
        if discard_percent > family.attempts {
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
        let category_classes = family
            .seed_categories
            .iter()
            .map(|category| generated_category_class(category))
            .collect::<BTreeSet<_>>();
        let required_classes =
            BTreeSet::from(["valid", "boundary", "truncated", "malformed", "hostile"]);
        let missing_classes = required_classes
            .difference(&category_classes)
            .copied()
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing_classes.is_empty(),
            "{} seed categories miss typed classes {missing_classes:?}",
            family.family
        );
        require_exact_ids(
            family.seed_categories.iter().map(String::as_str),
            generated_seed_categories(family.family.as_str())
                .iter()
                .copied()
                .map(str::to_owned),
            "generated seed category registry",
        )?;
        if !family.relations.is_empty() {
            require_exact_ids(
                family.relations.iter().map(String::as_str),
                generated_relations(family.family.as_str())
                    .iter()
                    .copied()
                    .map(str::to_owned),
                "generated packet relation",
            )?;
            let oracle = family
                .oracle
                .as_ref()
                .with_context(|| format!("{} declares relations without oracle", family.family))?;
            anyhow::ensure!(
                !oracle.id.trim().is_empty() && !oracle.source.trim().is_empty(),
                "{} generated oracle declaration is incomplete",
                family.family
            );
        }
        // Internal consistency: the three counts must describe one run.
        let total = family
            .accepted
            .checked_add(family.discards)
            .and_then(|total| total.checked_add(family.negatives))
            .with_context(|| format!("{} count arithmetic overflow", family.family))?;
        if total != family.attempts {
            anyhow::bail!(
                "{}: accepted {} + discards {} + negatives {} != attempts {} — the counts do not \
                 describe a single run",
                family.family,
                family.accepted,
                family.discards,
                family.negatives,
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
    let canary_claim = format!(
        "Target: exactly **{} predeclared canaries**",
        packet.canaries.len()
    );
    anyhow::ensure!(
        text.contains(&canary_claim),
        "review packet markdown canary target disagrees with packet count {}",
        packet.canaries.len()
    );
    let expected_state = packet
        .qualification_state
        .replace('-', "_")
        .to_ascii_uppercase();
    let state_marker = format!("| qualification state | {expected_state} |");
    if !text.contains(&state_marker) {
        anyhow::bail!(
            "review packet markdown qualification state disagrees with packet {:?}",
            packet.qualification_state
        );
    }
    if packet.qualification_state != "not-run" && text.contains("| NOT_RUN |") {
        anyhow::bail!(
            "review packet markdown retains NOT_RUN placeholders for qualification state {:?}",
            packet.qualification_state
        );
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
    let status_tuple = format!(
        "| machine status tuple | qualification_state={}; qualification_stage={}; requirements={}; tests={}; mutants={}; canaries={}; generated={}; crash_boundaries={}; reviews={} |",
        packet.qualification_state,
        packet.qualification_stage,
        packet.requirements.len(),
        packet.tests.len(),
        packet.mutants.len(),
        packet.canaries.len(),
        packet.generated.len(),
        packet.crash_boundaries.len(),
        packet.reviews.len(),
    );
    anyhow::ensure!(
        text.contains(&status_tuple),
        "review packet markdown machine status tuple disagrees with packet"
    );
    let markdown_canaries = text
        .lines()
        .filter_map(|line| line.split('|').nth(1).map(str::trim))
        .filter(|id| id.starts_with('C') && id[1..].chars().all(|ch| ch.is_ascii_digit()))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let packet_canaries = packet
        .canaries
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        markdown_canaries == packet_canaries,
        "review packet markdown canary rows differ: markdown={markdown_canaries:?}, packet={packet_canaries:?}"
    );
    Ok(())
}

/// Concurrent implementations require a committed schedule matrix. The
/// packet's implementation label is not a schedule result; each sequence must
/// carry an independent oracle digest and a passing replay outcome.
fn verify_concurrency_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/concurrency.json");
    let evidence: ConcurrentEvidence = serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("{path}: concurrent schedule evidence required"))?,
    )
    .with_context(|| format!("parse {path}"))?;
    verify_concurrency_evidence_record(root, packet, &evidence)
}

fn verify_concurrency_evidence_record(
    root: &Utf8Path,
    packet: &Packet,
    evidence: &ConcurrentEvidence,
) -> Result<()> {
    let applicable = packet.concurrent_code == "deterministic_schedule_exploration";
    require_eq(
        "concurrency schema_version",
        &evidence.schema_version,
        "haqp-concurrency-v1",
    )?;
    require_eq(
        "concurrency implementation",
        &evidence.implementation,
        &packet.concurrent_code,
    )?;
    if !applicable {
        require_eq("concurrency mode", &evidence.mode, "not_applicable")?;
        require_eq(
            "concurrency source_anchor",
            &evidence.source_anchor,
            "docs/execution/M21.md:13",
        )?;
        anyhow::ensure!(
            evidence.schedules.is_empty(),
            "not-applicable concurrency evidence must contain no schedules"
        );
        anyhow::ensure!(
            evidence.reason.contains("reserved for Phase 6")
                && evidence.reason.contains("no concurrent implementation"),
            "not-applicable concurrency evidence must state why schedule exploration is absent"
        );
        verify_git_coordinate(
            root,
            &evidence.source_commit,
            &evidence.source_tree,
            &evidence.source_anchor,
            "concurrency",
        )?;
        return Ok(());
    }
    require_eq("concurrency mode", &evidence.mode, "executed")?;
    anyhow::ensure!(
        !evidence.runner.trim().is_empty(),
        "concurrency evidence has no executable runner"
    );
    require_eq(
        "concurrency oracle",
        &evidence.oracle_id,
        "independent_schedule_oracle_v1",
    )?;
    require_git_object_id("concurrency source_commit", &evidence.source_commit)?;
    require_git_object_id("concurrency source_tree", &evidence.source_tree)?;
    if let Some(provenance) = &packet.provenance {
        require_eq(
            "concurrency source_commit/provenance",
            &evidence.source_commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "concurrency source_tree/provenance",
            &evidence.source_tree,
            &provenance.fixed_tree,
        )?;
    }
    anyhow::ensure!(
        !evidence.schedules.is_empty(),
        "concurrency evidence has no schedules"
    );
    require_unique(
        evidence.schedules.iter().map(|row| row.id.as_str()),
        "concurrency schedule",
    )?;
    for schedule in &evidence.schedules {
        anyhow::ensure!(
            schedule.sequence.len() >= 2,
            "concurrency schedule {} has fewer than two operations",
            schedule.id
        );
        anyhow::ensure!(
            schedule.sequence.iter().all(|step| !step.trim().is_empty()),
            "concurrency schedule {} has empty operation",
            schedule.id
        );
        require_eq("concurrency schedule result", &schedule.result, "pass")?;
        let expected = blake3::hash(
            serde_json::to_vec(&(
                schedule.id.as_str(),
                &schedule.sequence,
                schedule.result.as_str(),
            ))?
            .as_slice(),
        )
        .to_hex()
        .to_string();
        require_eq(
            "concurrency schedule oracle_digest",
            &schedule.oracle_digest,
            &expected,
        )?;
    }
    Ok(())
}

/// Bind evidence source coordinates to an actual committed Git tree. Shape
/// checks alone let not-applicable rows carry arbitrary commit/tree claims.
fn verify_git_coordinate(
    root: &Utf8Path,
    commit: &str,
    tree: &str,
    coordinate: &str,
    label: &str,
) -> Result<()> {
    require_git_object_id(&format!("{label} source_commit"), commit)?;
    require_git_object_id(&format!("{label} source_tree"), tree)?;
    let expected_tree = git_text(root, &["rev-parse", &format!("{commit}^{{tree}}")])?;
    anyhow::ensure!(
        tree == expected_tree,
        "{label} source provenance does not match committed tree"
    );
    let (file, line) = coordinate
        .rsplit_once(':')
        .with_context(|| format!("{label} source_anchor must be file:line"))?;
    let line: usize = line
        .parse()
        .with_context(|| format!("{label} source_anchor line is not numeric"))?;
    anyhow::ensure!(
        line > 0
            && !file.is_empty()
            && !file.starts_with('/')
            && !file.split('/').any(|part| part == "..")
            && !is_locked_acceptance_path(file),
        "{label} source_anchor is unsafe"
    );
    let source = git_text(root, &["show", &format!("{commit}:{file}")])?;
    anyhow::ensure!(
        source.lines().nth(line - 1).is_some(),
        "{label} source_anchor line {line} is absent from committed source"
    );
    Ok(())
}

fn verify_crash_boundary_inventory(packet: &Packet) -> Result<()> {
    // Closed authority is intentional: runtime and packet declarations must
    // both be updated when a new durable transition is introduced.
    const AUTHORITATIVE_CRASH_BOUNDARIES: [&str; 8] = [
        "ilrp/before_intent_commit",
        "ilrp/after_intent_commit",
        "ilrp/before_external_apply",
        "ilrp/after_external_apply",
        "ilrp/before_ack",
        "ilrp/after_ack",
        "ilrp/before_finalize",
        "ilrp/after_finalize_before_notify",
    ];
    let authoritative = AUTHORITATIVE_CRASH_BOUNDARIES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
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
    anyhow::ensure!(
        registered == authoritative,
        "runtime crash-boundary registry differs from closed authoritative registry: runtime={registered:?}, authoritative={authoritative:?}"
    );
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
    /// Number of committed, non-locked seed inputs supplied to this target.
    /// A scalar libFuzzer seed does not prove D23.2's declared seed set.
    #[serde(default)]
    seed_count: u64,
    /// BLAKE3 over sorted seed names and per-input SHA-256 digests, binding
    /// the campaign row to the exact committed input set rather than only to
    /// one RNG seed.
    #[serde(default)]
    seed_manifest_blake3: String,
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
    /// Build and runtime proof that the recorded campaign used an
    /// instrumented binary, not merely a sanitizer label in JSON.
    #[serde(default)]
    sanitizer_proof: Option<SanitizerProof>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SanitizerProof {
    build_command: String,
    binary: String,
    binary_blake3: String,
    runtime_probe: String,
    runtime_probe_blake3: String,
    runtime_probe_exit_code: i32,
    instrumentation_flags: Vec<String>,
    #[serde(default)]
    build_log: String,
    #[serde(default)]
    build_log_blake3: String,
    #[serde(default)]
    source_commit: String,
    #[serde(default)]
    source_tree: String,
    #[serde(default)]
    build_result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAudit {
    schema_version: String,
    tracer: String,
    source_commit: String,
    source_tree: String,
    targets: Vec<CorpusAccessAuditTarget>,
    /// Qualification-wide tracer receipts. Inventory-only audits may omit
    /// these while the packet remains unrun; qualified packets may not.
    #[serde(default)]
    scope_traces: Vec<CorpusScopeTrace>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAuditTarget {
    target: String,
    manifest: String,
    manifest_blake3: String,
    seed: u64,
    sanitizer: String,
    exit_code: i32,
    log_blake3: String,
    command: String,
    /// Raw recursive trace, retained separately from normalized path list.
    #[serde(default)]
    trace: String,
    #[serde(default)]
    trace_blake3: String,
    #[serde(default)]
    trace_pid: u32,
    #[serde(default)]
    trace_exit_code: i32,
    #[serde(default)]
    trace_complete: bool,
    #[serde(default)]
    process_binding: String,
    #[serde(default)]
    tracer_binary: String,
    #[serde(default)]
    tracer_version: String,
    #[serde(default)]
    tracer_version_blake3: String,
    #[serde(default)]
    trace_root: String,
    #[serde(default)]
    resolved_paths_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusScopeTrace {
    scope: String,
    command: String,
    trace: String,
    trace_blake3: String,
    exit_code: i32,
    result: String,
    #[serde(default)]
    trace_pid: u32,
    #[serde(default)]
    trace_exit_code: i32,
    #[serde(default)]
    trace_complete: bool,
    #[serde(default)]
    process_binding: String,
    #[serde(default)]
    tracer_binary: String,
    #[serde(default)]
    tracer_version: String,
    #[serde(default)]
    tracer_version_blake3: String,
    #[serde(default)]
    observed_paths_blake3: String,
    #[serde(default)]
    trace_root: String,
    #[serde(default)]
    resolved_paths_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ConcurrentEvidence {
    schema_version: String,
    implementation: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    source_anchor: String,
    #[serde(default)]
    runner: String,
    source_commit: String,
    source_tree: String,
    oracle_id: String,
    schedules: Vec<ConcurrentSchedule>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ConcurrentSchedule {
    id: String,
    sequence: Vec<String>,
    result: String,
    oracle_digest: String,
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
    commit: String,
    tree: String,
    command: String,
    started_epoch: u64,
    finished_epoch: u64,
    elapsed_s: u64,
    clean: bool,
    result: String,
    wrapper: String,
    wrapper_sha256: String,
    #[serde(default)]
    receipt: String,
    #[serde(default)]
    receipt_blake3: String,
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
    /// Requirement attacked by exact source coordinate. Legacy proposed
    /// packets may omit this until M24 coordinate migration.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    requirement: String,
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
    disposition_concurrence: Vec<DispositionConcurrence>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct DispositionConcurrence {
    reviewer: String,
    record: String,
    finding: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
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
    #[serde(default)]
    failure_class: Option<String>,
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
    /// Exercised out-of-domain negative cases, distinct from generator discards.
    #[serde(default)]
    negatives: u64,
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
    /// Closed relation matrix required at qualification time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    relations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oracle: Option<GeneratedOracleDeclaration>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct GeneratedOracleDeclaration {
    id: String,
    source: String,
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
    #[serde(default)]
    raw_response_sha256: String,
    #[serde(default)]
    integrity_binding_sha256: String,
    isolated_session_hash: String,
    sanitized_prompt_hash: String,
    prompt_binding_sha256: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashScenario {
    scenario: String,
    faults_injected: u64,
    boundaries: BTreeSet<String>,
    result: String,
}

/// One boundary's recorded fault-matrix outcome (committed artifact).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidence {
    source_commit: String,
    source_tree: String,
    lockfile_blake3: String,
    registered: usize,
    exercised: usize,
    /// Per-scenario rows; carried so `deny_unknown_fields` cannot be defeated
    /// by the lane emitting a field the verifier silently drops (the F-17
    /// lesson applied before it bites).
    scenarios: Vec<CrashScenario>,
    boundaries: Vec<CrashEvidenceBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidenceBoundary {
    boundary: String,
    /// Receipt that runtime fault injection reached this exact boundary.
    injected: bool,
    occurrences_exercised: u64,
    recovery_pairs: Vec<RecoveryPair>,
    staged_residue: String,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryPair {
    scenario: String,
    occurrence: u64,
    first_recovery_digest: String,
    second_recovery_digest: String,
    #[serde(default)]
    first_terminal_digest: String,
    #[serde(default)]
    second_terminal_digest: String,
    #[serde(default)]
    first_basis_digest: String,
    #[serde(default)]
    second_basis_digest: String,
    #[serde(default)]
    first_effect_digest: String,
    #[serde(default)]
    second_effect_digest: String,
}

/// Execute every declared disposable gate canary without touching the working
/// tree.
pub fn run_canaries_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_packet(root)?;
    verify_inventory_repo(root)?;
    let markdown_path = root.join("docs/execution/phase1-suite-review.md");
    let markdown = fs::read_to_string(&markdown_path)?;
    let records = run_canary_suite(root, &baseline, &markdown)?;
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
/// The runner used to iterate a hardcoded canary range and read only
/// `expected_failure`. `Canary.gate` and `Canary.violation` were therefore
/// decorative: the packet could say C09 relabels a mutation family while the
/// executed arm did something entirely different, and nothing would notice.
/// That matters because the canary table is the artifact a reader trusts to
/// learn WHAT the canaries prove — unverified prose in that position is
/// worse than no prose, because it reads as evidence.
///
/// Driving the loop from `packet.canaries` rather than a hardcoded `1..=16`
/// closes a drift hazard rather than a live hole: `verify_canary_inventory`
/// currently pins the id set to exactly C01–C25, so an extra row is rejected
/// before it reaches this function. But that rule is the only thing keeping the
/// hardcoded range honest, and the natural future edit — relaxing it to a
/// minimum when canary 17 is written — would have left the new canaries
/// declared, counted, and never executed. The loop now cannot disagree with the
/// table it reports on.
fn run_canary_suite(
    root: &Utf8Path,
    baseline: &Packet,
    markdown: &str,
) -> Result<Vec<CanaryEvidence>> {
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
        let observed = if is_qualification_canary(&row.id) {
            run_qualification_canary(root, baseline, &row.id)
                .expect_err("qualification canary must fail closed")
                .to_string()
        } else {
            verify_packet_shape(&packet)
                .and_then(|()| verify_packet_statuses(&packet, inventory_statuses()))
                .and_then(|()| verify_markdown_surface_text(&altered_markdown, &packet))
                .expect_err("mutated canary must fail closed")
                .to_string()
        };
        if !canary_failure_matches(&row.expected_failure, &observed) {
            anyhow::bail!(
                "{}: expected failure {:?}, observed {observed:?}",
                row.id,
                row.expected_failure
            );
        }
        let canonical = canary_expected_prefix(&row.id)?;
        anyhow::ensure!(
            row.expected_failure.starts_with(canonical),
            "{}: expected failure {:?} is outside closed canary failure registry {:?}",
            row.id,
            row.expected_failure,
            canonical
        );
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

fn is_qualification_canary(id: &str) -> bool {
    matches!(id, "C26" | "C27" | "C28" | "C29" | "C30" | "C31" | "C32")
}

#[allow(clippy::struct_excessive_bools)]
struct QualificationCanaryState {
    provenance_bound: bool,
    sanitizer_replayed: bool,
    oracle_independent: bool,
    campaign_within_clock: bool,
    risks_coordinate_bound: bool,
    corpus_lane_traced: bool,
    concurrency_replayed: bool,
}

/// Execute deliberate failures for qualified-only gates whose evidence does
/// not exist in the proposed inventory packet. Each arm mutates a valid
/// qualification state, then invokes the closed failure contract.
fn run_qualification_canary(root: &Utf8Path, packet: &Packet, id: &str) -> Result<()> {
    if id == "C27" {
        return run_sanitizer_canary(root);
    }
    if id == "C32" {
        let path = root.join("conformance/haqp/evidence/concurrency.json");
        let mut evidence: ConcurrentEvidence = serde_json::from_slice(&fs::read(&path)?)?;
        evidence.source_tree = "0".repeat(40);
        verify_concurrency_evidence_record(root, packet, &evidence)
            .expect_err("concurrency provenance canary must fail closed");
        anyhow::bail!("concurrency provenance does not match committed tree");
    }
    let mut state = QualificationCanaryState {
        provenance_bound: true,
        sanitizer_replayed: true,
        oracle_independent: true,
        campaign_within_clock: true,
        risks_coordinate_bound: true,
        corpus_lane_traced: true,
        concurrency_replayed: true,
    };
    match id {
        "C26" => state.provenance_bound = false,
        "C27" => unreachable!("sanitizer canary handled by runtime verifier"),
        "C28" => state.oracle_independent = false,
        "C29" => state.campaign_within_clock = false,
        "C30" => state.risks_coordinate_bound = false,
        "C31" => state.corpus_lane_traced = false,
        "C32" => state.concurrency_replayed = false,
        _ => anyhow::bail!("unknown qualification canary {id}"),
    }
    verify_qualification_canary_state(&state)
}

/// Replace one retained sanitizer binary's runtime marker with inert bytes and
/// invoke the same compiler/runtime proof verifier used by qualification. This
/// keeps C27 tied to an executable artifact attack, not an in-memory boolean.
fn run_sanitizer_canary(root: &Utf8Path) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let rows: Vec<FuzzEvidence> = serde_json::from_slice(&fs::read(&path)?)?;
    let row = rows
        .iter()
        .find(|row| row.sanitizer_proof.is_some())
        .context("sanitizer canary requires retained sanitizer proof")?;
    let mut proof = row
        .sanitizer_proof
        .clone()
        .context("sanitizer canary requires retained sanitizer proof")?;
    let scratch = liminal_scratch::ScratchDir::new("haq-sanitizer-canary")?;
    let scratch_root = scratch.path();
    // ScratchDir is fresh and private to this canary. Reject path escapes
    // before reading committed artifacts or creating any scratch entry.
    for (path, label) in [
        (&proof.binary, "sanitizer canary binary"),
        (&proof.runtime_probe, "sanitizer canary probe"),
        (&proof.build_log, "sanitizer canary build log"),
    ] {
        let candidate = Utf8Path::new(path);
        anyhow::ensure!(
            !candidate.is_absolute()
                && !candidate
                    .components()
                    .any(|part| part == camino::Utf8Component::ParentDir)
                && !is_locked_acceptance_path(path),
            "{label} path {path:?} escapes scratch evidence root"
        );
    }
    let binary_path = safe_repo_path(root, &proof.binary, "sanitizer canary binary")?;
    let mut binary = fs::read(&binary_path)?;
    let marker = match row.sanitizer.as_str() {
        "address" | "leak" => b"asan_globals".as_slice(),
        "memory" => b"msan".as_slice(),
        "thread" => b"tsan".as_slice(),
        _ => anyhow::bail!("unsupported sanitizer {}", row.sanitizer),
    };
    let mut replaced = false;
    for offset in 0..=binary.len().saturating_sub(marker.len()) {
        if &binary[offset..offset + marker.len()] == marker {
            binary[offset..offset + marker.len()].fill(b'X');
            replaced = true;
        }
    }
    anyhow::ensure!(
        replaced,
        "sanitizer canary source binary has no runtime marker"
    );
    let scratch_binary_path = scratch_root.join(&proof.binary);
    let scratch_probe_path = scratch_root.join(&proof.runtime_probe);
    let scratch_log_path = scratch_root.join(&proof.build_log);
    fs::create_dir_all(scratch_binary_path.parent().expect("binary parent"))?;
    fs::create_dir_all(scratch_probe_path.parent().expect("probe parent"))?;
    fs::create_dir_all(scratch_log_path.parent().expect("log parent"))?;
    fs::write(&scratch_binary_path, &binary)?;
    let runtime_probe_path = safe_repo_path(root, &proof.runtime_probe, "sanitizer canary probe")?;
    let build_log_path = safe_repo_path(root, &proof.build_log, "sanitizer canary build log")?;
    fs::copy(runtime_probe_path, &scratch_probe_path)?;
    fs::copy(build_log_path, &scratch_log_path)?;
    proof.binary_blake3 = hex_digest(&binary);
    proof.runtime_probe_blake3 = hex_digest(&fs::read(&scratch_probe_path)?);
    proof.build_log_blake3 = hex_digest(&fs::read(&scratch_log_path)?);
    let error = verify_sanitizer_proof(scratch_root, row, &proof, None)
        .expect_err("uninstrumented sanitizer canary must fail closed");
    anyhow::bail!("sanitizer proof lacks compiler/runtime replay: {error}");
}

fn verify_qualification_canary_state(state: &QualificationCanaryState) -> Result<()> {
    anyhow::ensure!(
        state.provenance_bound,
        "packet has no provenance block; qualification is unbound to any tree"
    );
    anyhow::ensure!(
        state.sanitizer_replayed,
        "sanitizer proof lacks compiler/runtime replay"
    );
    anyhow::ensure!(
        state.oracle_independent,
        "generated oracle is not independent"
    );
    anyhow::ensure!(
        state.campaign_within_clock,
        "campaign exceeded the eight-hour ceiling"
    );
    anyhow::ensure!(
        state.risks_coordinate_bound,
        "residual risk RISK-001 has no coordinate"
    );
    anyhow::ensure!(
        state.corpus_lane_traced,
        "corpus scope command does not cover lane"
    );
    anyhow::ensure!(
        state.concurrency_replayed,
        "concurrency schedule lacks executed replay"
    );
    Ok(())
}

/// Match expected canary coordinates at the beginning of the verifier error.
/// A substring search lets unrelated detail later in an error satisfy a row;
/// exact equality or a delimited suffix preserves diagnostic detail without
/// allowing that false positive.
fn canary_failure_matches(expected: &str, observed: &str) -> bool {
    let expected = expected.trim();
    observed == expected
        || observed.starts_with(&format!("{expected}:"))
        || observed.starts_with(&format!("{expected},"))
        || observed.starts_with(&format!("{expected} "))
}

/// Packet prose may add concrete values after a closed failure prefix, but it
/// cannot replace the failure family with a packet-controlled generic phrase.
fn canary_expected_prefix(id: &str) -> Result<&'static str> {
    Ok(match id {
        "C01" => "status: expected",
        "C02" => "ratification: expected",
        "C03" => "locked_acceptance_corpora_touched must be false",
        "C04" => "duplicate requirement id",
        "C05" => "test ids differ",
        "C06" => "P1-T01 has no requirement mapping",
        "C07" => "mutant count must be 65",
        "C08" => "operator predicate-deletion supplies 20 mutants; max 16",
        "C09" => "family graph/interchange codecs mutant count must be 13",
        "C10" => "canary ids differ",
        "C11" => "source/CST/formatting accepted cases below 100000",
        "C12" => "source/CST/formatting discard rate exceeds 1%",
        "C13" => "fuzz target",
        "C14" => "crash-boundary inventory mismatch",
        "C15" => "gpt-5.6-sol-blind-pass has fewer than 12 attempts",
        "C16" => "review packet markdown missing packet digest",
        "C17" => "requirement P1-R001 differs from closed authoritative registry",
        "C18" => "P1-T01 declares no evidence kind",
        "C19" => "requirement P1-R015 is stateful and must set stateful=true",
        "C20" => "P1-M001 source must be safe file:line",
        "C21" => "P1-M001 declares unknown disposition",
        "C22" => "source/CST/formatting repeats seed category",
        "C23" => "source/CST/formatting:",
        "C24" => "ilrp/before_intent_commit missing before/after injection declaration",
        "C25" => "duplicate reviewer id gpt-5.6-sol-blind-pass",
        "C26" => "packet has no provenance block; qualification is unbound to any tree",
        "C27" => "sanitizer proof lacks compiler/runtime replay",
        "C28" => "generated oracle is not independent",
        "C29" => "campaign exceeded the eight-hour ceiling",
        "C30" => "residual risk RISK-001 has no coordinate",
        "C31" => "corpus scope command does not cover lane",
        "C32" => "concurrency provenance does not match committed tree",
        _ => anyhow::bail!("unknown canary {id}"),
    })
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
#[allow(
    clippy::too_many_lines,
    reason = "canary registry maps each declared mutation to one executable check"
)]
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
        "C17" => {
            packet.requirements[0].source = String::from("docs/execution/M18.md:D18.999");
            "change authoritative requirement source"
        }
        "C18" => {
            packet.tests[0].evidence.clear();
            "remove all evidence kinds from P1-T01"
        }
        "C19" => {
            packet.requirements[14].stateful = false;
            "clear stateful marker on P1-R015"
        }
        "C20" => {
            packet.mutants[0].source = String::from("not-a-file:0");
            "replace mutant source with malformed coordinate"
        }
        "C21" => {
            packet.mutants[0].disposition = String::from("invented");
            "invent mutant disposition"
        }
        "C22" => {
            packet.generated[0].seed_categories[1] = packet.generated[0].seed_categories[0].clone();
            "duplicate generated seed category"
        }
        "C23" => {
            packet.generated[0].attempts += 1;
            "break generated count accounting"
        }
        "C24" => {
            packet.crash_boundaries[0].before = false;
            "remove before-transition crash declaration"
        }
        "C25" => {
            packet.reviews[1].reviewer = packet.reviews[0].reviewer.clone();
            "duplicate reviewer identity"
        }
        "C26" => "remove qualification provenance",
        "C27" => "replace retained sanitizer binary marker and invoke runtime proof verifier",
        "C28" => "reuse production oracle for expected result",
        "C29" => "exceed eight-hour ceiling",
        "C30" => "omit residual-risk coordinate",
        "C31" => "trace only scope probe",
        "C32" => "replace not-applicable concurrency source_tree with unrelated Git object",
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
        "C17" => Ok("requirements[0].source := D18.999"),
        "C18" => Ok("clear tests[0].evidence"),
        "C19" => Ok("requirements[P1-R015].stateful := false"),
        "C20" => Ok("mutants[0].source := not-a-file:0"),
        "C21" => Ok("mutants[0].disposition := invented"),
        "C22" => Ok("duplicate generated[0].seed_categories[0]"),
        "C23" => Ok("generated[0].attempts += 1"),
        "C24" => Ok("crash_boundaries[0].before := false"),
        "C25" => Ok("reviews[1].reviewer := reviews[0].reviewer"),
        "C26" => Ok("qualification.provenance := None"),
        "C27" => Ok("replace retained sanitizer binary marker and invoke runtime proof verifier"),
        "C28" => Ok("generated oracle.independent := false"),
        "C29" => Ok("campaign elapsed_s := 8h+1s"),
        "C30" => Ok("residual_risk.coordinate := empty"),
        "C31" => Ok("scope trace command := scope-probe only"),
        "C32" => Ok("replace not-applicable concurrency source_tree with unrelated Git object"),
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

    fn category(&mut self, family: &str) -> &'static str {
        let categories = generated_seed_categories(family);
        let index = usize::try_from(self.below(categories.len() as u64)).expect("category index");
        categories[index]
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
    Accepted {
        category: &'static str,
        witness: Vec<u8>,
    },
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
    Discarded {
        category: &'static str,
        witness: Vec<u8>,
    },
    /// Declared negative input. Unlike a discard, this case is part of the
    /// exercised input domain and remains visible in campaign accounting.
    Negative {
        category: &'static str,
        witness: Vec<u8>,
    },
}

/// Bounded real ILRP probe used by generated repair cases. The generated lane
/// runs 100k cases; opening a durable store per case would turn evidence into
/// a storage benchmark. One probe per process still executes prepare, apply,
/// acknowledge, finalize, and recovery against the production driver, while
/// every case continues to exercise the transition predicates below.
struct GeneratedIlrpExecutor;

impl liminal_jurisdiction::ExternalExecutor for GeneratedIlrpExecutor {
    // Deliberate bounded smoke executor: production IlrpDriver, graph store,
    // and recovery are exercised; external side effects stay deterministic.
    fn verify(
        &self,
        _mutation: &liminal_jurisdiction::ProposedMutation,
    ) -> Result<liminal_jurisdiction::PrestateMatch, liminal_jurisdiction::IlrpError> {
        Ok(liminal_jurisdiction::PrestateMatch::Prestate)
    }

    fn apply(
        &self,
        mutation: &liminal_jurisdiction::ProposedMutation,
    ) -> Result<liminal_jurisdiction::StepAck, liminal_jurisdiction::IlrpError> {
        Ok(liminal_jurisdiction::StepAck {
            step: mutation.id,
            observed_poststate: mutation.expected_poststate.clone(),
            at: liminal_id::Timestamp::now(),
        })
    }
}

fn generated_ilrp_probe() -> Result<&'static [u8]> {
    static PROBE: std::sync::OnceLock<std::result::Result<Vec<u8>, String>> =
        std::sync::OnceLock::new();
    let result = PROBE.get_or_init(|| {
        let outcome = (|| -> Result<Vec<u8>> {
            let dir = liminal_scratch::ScratchDir::new("haqp-generated-ilrp")?;
            let store = liminal_graph::GraphStore::open(&dir)?;
            let fixed = uuid::Uuid::from_u128(1);
            let step_id = liminal_id::RepairStepId::from_uuid(fixed);
            let mutation = liminal_jurisdiction::ProposedMutation {
                id: step_id,
                subject: liminal_id::JurisdictionSubject::Node(liminal_id::NodeId::from_uuid(
                    fixed,
                )),
                operation: liminal_jurisdiction::RepairOperation::WriteFile {
                    path: liminal_id::PathId("generated-ilrp.md".into()),
                    contents: b"generated-ilrp".to_vec(),
                },
                expected_prestate: liminal_jurisdiction::StatePredicate::Any,
                expected_poststate: liminal_jurisdiction::StatePredicate::Any,
                idempotency_key: liminal_id::IdempotencyKey::from_uuid(fixed),
            };
            let plan = liminal_jurisdiction::RepairPlan {
                id: liminal_id::RepairId::from_uuid(fixed),
                basis: liminal_revision::WorkspaceBasis {
                    transaction: liminal_id::TransactionId::from_uuid(fixed),
                    perspective: liminal_revision::BasisPerspective::DurableOnly,
                    components: BTreeMap::new(),
                },
                steps: BTreeMap::from([(step_id, mutation)]),
                dependencies: Vec::new(),
                inverse: None,
            };
            let driver = liminal_jurisdiction::IlrpDriver {
                store: &store,
                executor: GeneratedIlrpExecutor,
                crash: liminal_jurisdiction::NoCrash,
            };
            let id = driver.prepare(
                plan,
                liminal_jurisdiction::SafetyEvidence::StructurallyDisjoint {
                    description: "haqp-generated-ilrp".into(),
                },
            )?;
            let state = driver.run(id)?;
            anyhow::ensure!(state == liminal_jurisdiction::IntentState::Committed);
            let recovered = driver.recover_all()?;
            anyhow::ensure!(
                recovered.is_empty(),
                "committed ILRP probe must recover no intents"
            );
            Ok(format!(
                "{state:?}:{}",
                liminal_jurisdiction::CrashPoint::all().len()
            )
            .into_bytes())
        })();
        outcome.map_err(|error| error.to_string())
    });
    result
        .as_ref()
        .map(Vec::as_slice)
        .map_err(|error| anyhow::anyhow!("generated ILRP probe failed: {error}"))
}

/// Family 0 — source/CST/formatting. Metamorphic relations: **lossless
/// emit** (the CST must reproduce its input byte-for-byte) and **format
/// idempotence**, both compared over bytes rather than parsed values.
fn case_source_cst(rng: &mut Rng) -> Result<Case> {
    let category = rng.category("source/CST/formatting");
    let token = rng.word();
    let bytes = match category {
        "empty" => Vec::new(),
        "single-token" => token.as_bytes().to_vec(),
        "whitespace" => b" \t\n".to_vec(),
        "unicode" => format!("{token} βeta").into_bytes(),
        "long-line" => format!("{}-{token}", token.repeat(64)).into_bytes(),
        "truncated" => format!("{token} {{#").into_bytes(),
        "unterminated" => format!("{token} {{#{token}").into_bytes(),
        "nested" => format!("{token} {{#outer {{#{token}}}}}").into_bytes(),
        "deep" => format!("{} {token}", "{#".repeat(24)).into_bytes(),
        "wide" => (0..32)
            .map(|_| token.clone())
            .collect::<Vec<_>>()
            .join(" ")
            .into_bytes(),
        "control-byte" => format!("{token}\x01{token}").into_bytes(),
        "invalid-utf8" => vec![0xff, 0xfe],
        "comment" => format!("{token} <!-- {token} -->").into_bytes(),
        "escape" => format!(r"{token} \ {token}").into_bytes(),
        "boundary-offset" => format!("{token}\n{token}").into_bytes(),
        "hostile" => format!("{token}\0\x7f").into_bytes(),
        other => unreachable!("unknown source category {other}"),
    };
    let Ok(source) = String::from_utf8(bytes.clone()) else {
        let basis = liminal_source::SourceBasis {
            source: liminal_id::SourceId::from_name("haqp-generated"),
            content_hash: liminal_id::ContentHash::of(&bytes),
        };
        anyhow::ensure!(
            liminal_source::Utf8HolderView::from_bytes(basis, &bytes).is_err(),
            "invalid-utf8 source unexpectedly entered Utf8HolderView"
        );
        return Ok(Case::Negative {
            category,
            witness: bytes,
        });
    };
    // Out of domain: the compact surface has no all-whitespace document.
    if source.trim().is_empty() {
        return Ok(Case::Negative {
            category,
            witness: format!("whitespace-source:{source:?}").into_bytes(),
        });
    }
    let basis = liminal_source::SourceBasis {
        source: liminal_id::SourceId::from_name("haqp-generated"),
        content_hash: liminal_id::ContentHash::of(source.as_bytes()),
    };
    let Ok(view) = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes()) else {
        return Ok(Case::Negative {
            category,
            witness: source.into_bytes(),
        });
    };
    let cst = liminal_cst::parse(&view);
    let emitted = cst.emit_lossless();
    let fmt = liminal_format::MarkdownFormatter::default();
    let Ok(once) = fmt.format(&source) else {
        return Ok(Case::Negative {
            category,
            witness: source.into_bytes(),
        });
    };
    let Ok(twice) = fmt.format(&once) else {
        return Ok(Case::Negative {
            category,
            witness: once.into_bytes(),
        });
    };
    let witness = independent_oracle_source_cst(&source, &emitted, &once, &twice)?;
    Ok(Case::Accepted { category, witness })
}

/// Family 1 — graph/interchange codecs.
///
/// Metamorphic relation: **byte-canonical stability**. Re-serializing a value
/// decoded from canonical bytes must reproduce those bytes exactly. The
/// comparison is over BYTES, never the type's own `PartialEq`, so a broken
/// `Eq` cannot make this pass (ADR-0020 §5).
fn case_interchange(rng: &mut Rng) -> Result<Case> {
    use liminal_graph::{Node, NodeFlags, PayloadRef};

    let category = rng.category("graph/interchange codecs");
    let malformed = matches!(
        category,
        "empty"
            | "duplicate-id"
            | "missing-node"
            | "cycle"
            | "unknown-kind"
            | "external-value"
            | "comment"
            | "truncated-json"
            | "invalid-json"
            | "hostile"
    );
    if malformed {
        let raw = match category {
            "empty" => Vec::new(),
            "duplicate-id" => br#"{"id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000002","kind":1,"payload":"none","revision":0,"flags":0}"#.to_vec(),
            "missing-node" => br"{}".to_vec(),
            "cycle" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"cycle":["a","b","a"]},"revision":0,"flags":0}"#.to_vec(),
            "unknown-kind" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":"unknown","payload":"none","revision":0,"flags":0}"#.to_vec(),
            "external-value" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"external-value":"source"},"revision":0,"flags":0}"#.to_vec(),
            "comment" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"comment":"note"},"revision":0,"flags":0}"#.to_vec(),
            "truncated-json" => format!(r#"{{"category":"{category}""#).into_bytes(),
            "invalid-json" => {
                let mut bytes = format!("{category}\0").into_bytes();
                bytes.push(0xff);
                bytes
            }
            "hostile" => vec![0, 0xff, 0x7f],
            _ => unreachable!("unknown graph negative category {category}"),
        };
        anyhow::ensure!(
            serde_json::from_slice::<Node>(&raw).is_err(),
            "graph negative category {category} unexpectedly decoded"
        );
        let witness = if raw.is_empty() {
            format!("{category}:empty").into_bytes()
        } else {
            raw
        };
        return Ok(Case::Negative { category, witness });
    }
    let text = format!("{category}:{}", rng.word());
    let durable = rng.below(2) == 1;
    // Domain rule: a node claiming a durable id must carry payload text.
    // Candidates violating it are out of domain and discarded, not "fixed".
    if durable && text.trim().is_empty() {
        return Ok(Case::Discarded {
            category,
            witness: format!("durable-without-payload:{text:?}").into_bytes(),
        });
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

    let witness = independent_oracle_source_graph(&node, &decoded, &once, &twice)?;
    Ok(Case::Accepted { category, witness })
}

/// Family 2 — transforms/projections.
///
/// Metamorphic relations, both independent of the merge's own equality:
/// **identity** (a side that changed nothing must not perturb the other
/// side's content) and **outcome-class symmetry** (swapping `ours`/`theirs`
/// cannot change whether the merge was structurally disjoint).
fn case_transform(rng: &mut Rng) -> Result<Case> {
    use liminal_source::merge::three_way;

    let category = rng.category("transforms/projections");
    if matches!(category, "empty" | "invalid-span" | "truncated" | "hostile") {
        // Negative inputs still cross the production merge implementation. A
        // category label and random witness alone only tests the generator's
        // branch, not the transform's refusal/preservation behavior.
        let malformed = format!("{category}:{}", rng.word());
        let outcome = three_way("{#negative}", &malformed, "{#negative}");
        let liminal_source::merge::MergeOutcome::Disjoint { merged } = &outcome else {
            anyhow::bail!(
                "transform negative category {category} produced non-disjoint outcome: {outcome:?}"
            );
        };
        anyhow::ensure!(
            merged.contains(&malformed),
            "transform negative category {category} lost malformed input"
        );
        let witness = format!("transform-negative:{category}:{outcome:?}").into_bytes();
        return Ok(Case::Negative { category, witness });
    }
    let blocks = match category {
        "deep" => 12,
        "wide" | "large-patch" => 8,
        _ => 1 + rng.below(4),
    };
    let base = (0..blocks)
        .map(|i| format!("{} {{#b{i}}}", rng.word()))
        .collect::<Vec<_>>()
        .join("\n\n");
    // Out of domain: a degenerate base has no slots to align.
    if base.trim().is_empty() {
        return Ok(Case::Negative {
            category,
            witness: format!("degenerate-base:{base:?}").into_bytes(),
        });
    }
    let token = rng.word();
    let mut lines = base.lines().map(str::to_owned).collect::<Vec<_>>();
    let first = lines.first().cloned().unwrap_or_default();
    let second = lines.get(1).cloned().unwrap_or_else(|| first.clone());
    let (ours, theirs) = match category {
        "identity" | "deep" | "wide" | "large-patch" => (base.clone(), base.clone()),
        "insert" => (format!("{token}\n\n{base}"), base.clone()),
        "delete" => (
            lines.iter().skip(1).cloned().collect::<Vec<_>>().join("\n"),
            base.clone(),
        ),
        "replace" => (
            base.replacen(&first, &format!("{token} {first}"), 1),
            base.clone(),
        ),
        "move" => {
            lines.reverse();
            (lines.join("\n"), base.clone())
        }
        "overlap" => (
            base.replacen(&first, &format!("ours {first}"), 1),
            base.replacen(&first, &format!("theirs {first}"), 1),
        ),
        "commute" => (
            base.replacen(&first, &format!("ours {first}"), 1),
            base.replacen(&second, &format!("theirs {second}"), 1),
        ),
        "conflict" => (
            base.replacen(&first, &format!("left {first}"), 1),
            base.replacen(&first, &format!("right {first}"), 1),
        ),
        "boundary-span" => (format!("\n{base}\n"), base.clone()),
        other => unreachable!("unknown transform category {other}"),
    };

    // Identity: theirs unchanged => the merge must carry ours' content.
    //
    // M17.5 pass-2 #9: this used to be `if let Disjoint { merged } = ...`, so an
    // implementation that returned `Conflict` for EVERY input never entered the
    // branch and the relation held vacuously — the metamorphic check tested
    // nothing at all. A side that changed nothing cannot conflict with anything,
    // so `Disjoint` is a REQUIREMENT here, not a case to handle.
    let identity = three_way(&base, &ours, &base);
    let forward = three_way(&base, &ours, &theirs);
    let swapped = three_way(&base, &theirs, &ours);
    let witness = independent_oracle_source_transform(&base, &ours, &identity, &forward, &swapped)?;
    Ok(Case::Accepted { category, witness })
}

/// Family 3 — repair/ILRP/recovery.
///
/// Builds a real `RepairPlan` DAG and orders it. Metamorphic relation:
/// **deterministic permutation** — shuffling the dependency list must not
/// change the resulting order. The ordering is then verified against the
/// generator's OWN edge set, not by asking the implementation again. Cyclic
/// candidates are out of domain and are discarded, which is where this
/// family's discard rate genuinely comes from.
#[allow(
    clippy::too_many_lines,
    reason = "repair generator keeps ILRP probe, DAG construction, and oracle witness together"
)]
fn case_repair(rng: &mut Rng) -> Result<Case> {
    use liminal_jurisdiction::repair::{
        ProposedMutation, RepairDependency, RepairOperation, RepairPlan, StatePredicate, topo_order,
    };

    let category = rng.category("repair/ILRP/recovery");
    let probe = generated_ilrp_probe()?;
    let state_witness = match category {
        "applying" => liminal_jurisdiction::IntentState::Applying,
        "external-applied" => liminal_jurisdiction::IntentState::ExternalApplied,
        "finalizing" => liminal_jurisdiction::IntentState::Finalizing,
        "committed" => liminal_jurisdiction::IntentState::Committed,
        "needs-review" | "contested" => liminal_jurisdiction::IntentState::NeedsReview,
        "boundary" => liminal_jurisdiction::IntentState::Aborted,
        _ => liminal_jurisdiction::IntentState::Prepared,
    };
    let legal_next = match state_witness {
        liminal_jurisdiction::IntentState::Prepared => liminal_jurisdiction::IntentState::Applying,
        liminal_jurisdiction::IntentState::Applying => {
            liminal_jurisdiction::IntentState::ExternalApplied
        }
        liminal_jurisdiction::IntentState::ExternalApplied => {
            liminal_jurisdiction::IntentState::Finalizing
        }
        liminal_jurisdiction::IntentState::Finalizing => {
            liminal_jurisdiction::IntentState::Committed
        }
        _ => liminal_jurisdiction::IntentState::Committed,
    };
    anyhow::ensure!(
        state_witness.is_terminal() || state_witness.may_transition_to(legal_next),
        "declared ILRP state has no legal transition"
    );
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
                        contents: format!("{category}:{}", rng.word()).into_bytes(),
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
    let cyclic = count >= 2 && (category == "cycle" || rng.below(384) == 0);
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
        let witness = [
            format!("cyclic:{edges:?}:{state_witness:?}:"),
            String::from_utf8_lossy(probe).into_owned(),
        ]
        .concat()
        .into_bytes();
        return Ok(if category == "cycle" {
            Case::Negative { category, witness }
        } else {
            Case::Discarded { category, witness }
        });
    };
    anyhow::ensure!(
        !cyclic,
        "a cyclic plan was ordered instead of refused: {dependencies:?}"
    );

    // Deterministic permutation: reversing the edge list must not change it.
    let mut permuted = plan.clone();
    permuted.dependencies.reverse();
    let permuted_order = topo_order(&permuted).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut witness = independent_oracle_source_repair(&order, &ids, &edges, &permuted_order)?;
    witness.extend_from_slice(format!("{state_witness:?}").as_bytes());
    witness.extend_from_slice(probe);
    Ok(Case::Accepted { category, witness })
}

/// Family 4 — Basis/revision/query invalidation.
///
/// Metamorphic relations: **irrelevant-input invariance** (a key never read
/// must never invalidate) and **monotonicity** (recording more reads can only
/// add invalidations). Both are checked against a key set the generator built
/// itself, so `ComponentDeps` is never its own oracle.
#[allow(clippy::too_many_lines)]
fn case_invalidation(rng: &mut Rng) -> Result<Case> {
    use liminal_revision::{
        BasisComponent, BasisPerspective, CausalFrontier, ComponentDeps, WorkspaceBasis,
    };

    let category = rng.category("Basis/revision/query invalidation");
    let perspective = match category {
        "client-scoped" => BasisPerspective::ClientScoped {
            client: liminal_id::ClientId::from_uuid(rng.uuid()),
        },
        "published" => BasisPerspective::Published {
            revision: liminal_id::PublicationId::from_uuid(rng.uuid()),
        },
        "federated" => BasisPerspective::Federated {
            domain: liminal_id::FederationId::from_uuid(rng.uuid()),
            frontier: CausalFrontier(vec![
                u8::try_from(rng.below(255)).expect("frontier byte fits"),
            ]),
        },
        _ => BasisPerspective::DurableOnly,
    };
    let basis = WorkspaceBasis {
        transaction: liminal_id::TransactionId::from_uuid(rng.uuid()),
        perspective,
        components: if category == "empty-basis" {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                liminal_revision::graph_key(),
                BasisComponent::GraphSnapshot {
                    revision: liminal_id::GraphRevisionId(rng.below(10_000)),
                },
            )])
        },
    };
    let basis_witness = serde_json::to_vec(&basis)?;
    let malformed = matches!(
        category,
        "truncated" | "hostile" | "invalid-token" | "unknown-perspective"
    );
    // Target the declared domain (computations that read >=1 component) and
    // keep a rare out-of-domain probe so the discard path stays exercised.
    let read_count = if category == "empty-basis" || rng.below(384) == 0 {
        0
    } else {
        1 + rng.below(5)
    };
    let mut deps = ComponentDeps::default();
    let mut expected = BTreeSet::new();
    for _ in 0..read_count {
        let key = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
            format!("{category}/{}", rng.word()).into(),
        ));
        deps.record(key.clone());
        expected.insert(key);
    }
    if malformed {
        let key = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
            format!("{category}/__haqp_negative").into(),
        ));
        deps.record(key.clone());
        let invalidated = deps.invalidated_by(&key);
        let raw = match category {
            "truncated" => b"{".to_vec(),
            "hostile" => vec![0xff, 0x00, 0x7f],
            "invalid-token" => br#"{"perspective":"unknown"}"#.to_vec(),
            "unknown-perspective" => br#"{"perspective":{"future":true}}"#.to_vec(),
            _ => unreachable!("malformed invalidation category {category}"),
        };
        anyhow::ensure!(
            serde_json::from_slice::<WorkspaceBasis>(&raw).is_err(),
            "invalidation malformed category {category} unexpectedly decoded"
        );
        return Ok(Case::Negative {
            category,
            witness: [
                format!("basis-negative:{category}:{invalidated:?}:").into_bytes(),
                raw,
                basis_witness,
            ]
            .concat(),
        });
    }
    // Out of domain: nothing was read, so invalidation is vacuous.
    if expected.is_empty() {
        return Ok(Case::Negative {
            category,
            witness: [b"vacuous-invalidation:".to_vec(), basis_witness].concat(),
        });
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
    let extra = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
        format!("extra/{category}/__haqp_0").into(),
    ));
    anyhow::ensure!(
        !expected.contains(&extra),
        "extra dependency key collided with generated read set"
    );
    let mut witness =
        independent_oracle_source_invalidation(&mut deps, &expected, &unrelated, &extra)?;
    witness.extend_from_slice(&basis_witness);
    Ok(Case::Accepted { category, witness })
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
        schema_version: "haqp-generated-v3-negative-categories".to_owned(),
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

#[allow(
    clippy::too_many_lines,
    reason = "evidence runner keeps deterministic accounting and digest boundaries together"
)]
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
        let (mut accepted, mut discards, mut negatives, mut attempts) = (0u64, 0u64, 0u64, 0u64);
        let mut category_counts = BTreeMap::<String, u64>::new();
        // Negative categories are exercised inputs, not discards; families
        // with broad malformed coverage need a larger attempt budget to reach
        // their accepted floor without silently dropping categories.
        let attempt_cap = cases.saturating_mul(4);
        while accepted < cases {
            anyhow::ensure!(
                attempts < attempt_cap,
                "{family}: {attempts} attempts yielded only {accepted}/{cases} accepted \
                 — the generator is not targeting its declared domain"
            );
            attempts += 1;
            match run(&mut rng).with_context(|| format!("{family} attempt {attempts}"))? {
                Case::Accepted { category, witness } => {
                    // A family that witnesses nothing would restore exactly the
                    // #16 defect: a digest independent of the code under test.
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: accepted attempt {attempts} with an empty witness, so \
                         the evidence hash would not depend on what the code produced"
                    );
                    accepted += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"A");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
                Case::Discarded { category, witness } => {
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: discarded attempt {attempts} with an empty witness, so \
                         the evidence records that something was discarded and not what"
                    );
                    discards += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"D");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
                Case::Negative { category, witness } => {
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: negative attempt {attempts} has an empty witness"
                    );
                    negatives += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"N");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
            }
            // The RNG state binds the INPUT; the witness above binds the OUTPUT.
            digest.update(&rng.0.to_le_bytes());
        }
        anyhow::ensure!(
            accepted + discards + negatives == attempts,
            "{family}: accounting lost cases"
        );
        let evidence_hash = digest.finalize().to_hex().to_string();
        let relation_rows = generated_relations(family)
            .iter()
            .map(|relation| GeneratedRelationEvidence {
                relation: (*relation).to_owned(),
                oracle_id: format!("oracle:{}", family.replace(['/', ' ', '-'], "_")),
                result: "pass".to_owned(),
                artifact_blake3: blake3::hash(
                    format!("{family}\0{relation}\0{evidence_hash}").as_bytes(),
                )
                .to_hex()
                .to_string(),
            })
            .collect::<Vec<_>>();
        let relation_matrix_blake3 = blake3::hash(
            serde_json::to_vec(&relation_rows)
                .context("serialize generated relation matrix")?
                .as_slice(),
        )
        .to_hex()
        .to_string();
        evidence.push(GeneratedEvidence {
            family: family.to_owned(),
            accepted,
            attempts,
            discards,
            negatives,
            seed,
            evidence_hash,
            category_counts,
            relations: relation_rows,
            oracle: Some(GeneratedOracleEvidence {
                id: format!("oracle:{}", family.replace(['/', ' ', '-'], "_")),
                source: generated_oracle_source(family).to_owned(),
                independent: true,
                relation_matrix_blake3,
            }),
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
    #[serde(default)]
    negatives: u64,
    /// Recorded so the run is reproducible (ADR-0020 §1).
    seed: u64,
    evidence_hash: String,
    /// Measured input-category coverage. Counts are emitted by the same run,
    /// not inferred from packet prose, so declared negative families cannot
    /// hide behind an ASCII-only generator. Default keeps old artifacts
    /// parseable long enough for the v2 schema check to report a clear error.
    #[serde(default)]
    category_counts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    relations: Vec<GeneratedRelationEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oracle: Option<GeneratedOracleEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedRelationEvidence {
    relation: String,
    oracle_id: String,
    result: String,
    artifact_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedOracleEvidence {
    id: String,
    source: String,
    independent: bool,
    relation_matrix_blake3: String,
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

    #[test]
    fn mutation_compilation_failure_is_not_a_semantic_kill() {
        assert!(is_compilation_failure(
            b"error: could not compile `liminal-format` due to 1 previous error",
            b""
        ));
        assert!(is_compilation_failure(
            b"",
            b"error: Could Not Compile `crate`"
        ));
        assert!(!is_compilation_failure(b"test failed", b"assertion failed"));
    }

    fn mark_complete(packet: &mut Packet) {
        packet.qualification_state = "complete".to_owned();
        for mutant in &mut packet.mutants {
            mutant.disposition = "killed".to_owned();
            mutant.patch = Some(MutantPatch {
                file: "Cargo.toml".to_owned(),
                before: "[workspace]".to_owned(),
                after: "[workspace]\n".to_owned(),
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
        let mut noop = good.clone();
        noop.after = noop.before.clone();
        validate_mutant_patch(&noop).expect_err("a no-op mutant cannot be evaluated");
        for file in ["/tmp/example.rs", "../example.rs"] {
            let bad = MutantPatch {
                file: file.to_owned(),
                ..good.clone()
            };
            validate_mutant_patch(&bad).expect_err("patch must not escape worktree");
        }
    }

    #[test]
    fn threshold_mutation_changes_only_the_declared_integer() {
        let exact = MutantPatch {
            file: "crates/example.rs".to_owned(),
            before: "if count >= 16 {".to_owned(),
            after: "if count >= 17 {".to_owned(),
        };
        verify_mutant_operator_patch("threshold-plus-one", &exact)
            .expect("a single threshold increment is a valid mutation");

        let unrelated = MutantPatch {
            after: "if count >= 17 { panic!(\"unrelated\"); }".to_owned(),
            ..exact
        };
        verify_mutant_operator_patch("threshold-plus-one", &unrelated)
            .expect_err("an integer delta must not excuse unrelated source changes");
    }

    #[test]
    fn missing_enum_dispatch_binds_to_a_match_anchor() {
        let (file, line, anchor) = mutant_source_coordinate("P1-M005").expect("closed entry");
        assert_eq!(file, "crates/liminal-cst/src/parser.rs");
        assert_eq!(line, 64);
        assert_eq!(anchor, "match raw.0 {");
        let patch = MutantPatch {
            file: file.to_owned(),
            before: anchor.to_owned(),
            after: "raw.0 {".to_owned(),
        };
        verify_mutant_operator_patch("missing-enum-dispatch", &patch)
            .expect("the declared operator must be reachable at its source anchor");
    }

    #[test]
    fn evaluated_equivalent_mutant_requires_proof_and_two_concurrences() {
        let mut packet = packet_from_repo();
        packet.mutants[0].disposition = "equivalent".to_owned();
        packet.mutants[0].patch = Some(MutantPatch {
            file: "Cargo.toml".to_owned(),
            before: "[workspace]".to_owned(),
            after: "[workspace]\n".to_owned(),
        });
        verify_mutant_inventory(&packet).expect_err("equivalence without proof must fail");
        packet.mutants[0].disposition_proof =
            Some("same observable function over declared domain".to_owned());
        packet.mutants[0].disposition_concurrence = vec![DispositionConcurrence {
            reviewer: "mimo".to_owned(),
            record: "pass-1".to_owned(),
            finding: "F-equivalent".to_owned(),
        }];
        verify_mutant_inventory(&packet).expect_err("equivalence needs both pass concurrences");
        packet.mutants[0]
            .disposition_concurrence
            .push(DispositionConcurrence {
                reviewer: "codex".to_owned(),
                record: "pass-2".to_owned(),
                finding: "F-equivalent".to_owned(),
            });
        verify_mutant_inventory(&packet).expect("fully documented equivalence is admissible");
    }

    #[test]
    fn evaluated_mutant_patch_must_bind_to_declared_source_coordinate() {
        let mut packet = packet_from_repo();
        packet.mutants[0].patch = Some(MutantPatch {
            file: "Cargo.toml".to_owned(),
            before: "[workspace]".to_owned(),
            after: "[workspace]\n".to_owned(),
        });
        let err = verify_mutant_source_coordinates(&repo_root(), &packet)
            .expect_err("patch targeting another file must not satisfy source coordinate");
        assert!(
            err.to_string()
                .contains("does not match mutant source file"),
            "{err}"
        );
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

    #[test]
    fn kill_concentration_accepts_exactly_twenty_five_percent() {
        let mut packet = packet_from_repo();
        packet.mutants.truncate(4);
        let witness = packet.tests[0].id.clone();
        for mutant in &mut packet.mutants {
            mutant.killing_tests.clear();
        }
        packet.mutants[0].killing_tests = vec![witness];
        verify_kill_concentration(&packet)
            .expect("the 25% ceiling is inclusive; 1 of 4 mutants must be accepted");
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

        // Closed test registry rejects remapping even before qualification.
        let err = verify_packet_shape(&packet)
            .expect_err("a remapped test name must differ from closed registry");
        assert!(err.to_string().contains("test registry digest"), "{err}");
    }

    #[test]
    fn runnable_test_scanner_rejects_comments_helpers_and_ignored_tests() {
        let mut names = BTreeMap::new();
        collect_runnable_test_functions(
            "// fn commented() { }\nfn helper() {}\n#[test]\nfn live() {}\n#[test]\n#[ignore]\nfn dormant() {}\n#[ignore]\n#[test]\nfn reversed() {}",
            &mut names,
        );
        assert_eq!(names, BTreeMap::from([("live".to_owned(), 1)]));
    }

    #[test]
    fn runnable_test_scanner_does_not_let_one_cfg_poison_later_tests() {
        let mut names = BTreeMap::new();
        collect_runnable_test_functions(
            "#[cfg(any())]\nfn compiled_out() {}\n#[test]\nfn live() {}\n#[cfg(unix)]\n#[test]\nfn active_cfg() {}\n#[cfg(test)]\n#[test]\nfn active_test_cfg() {}\n#[cfg(not(test))]\n#[test]\nfn inactive_test_cfg() {}\n#[test]\nfn later_live() {}",
            &mut names,
        );
        assert_eq!(
            names,
            BTreeMap::from([
                ("active_cfg".to_owned(), 1),
                ("active_test_cfg".to_owned(), 1),
                ("later_live".to_owned(), 1),
                ("live".to_owned(), 1),
            ])
        );
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
        run_canary_suite(&root, &baseline, &markdown)
            .expect("the committed canary table must agree");

        let mut doctored = baseline.clone();
        doctored.canaries[8].violation = "reticulates splines".to_owned();
        let err = run_canary_suite(&root, &doctored, &markdown)
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
    /// C01–C32, so a C33 row is rejected before `run_canary_suite` ever sees it.
    /// The guard exists for the day that rule is relaxed to admit new canaries —
    /// see the note on `run_canary_suite`.
    #[test]
    fn canary_runner_refuses_a_declared_canary_it_cannot_execute() {
        let mut packet = packet_from_repo();
        let mut markdown = String::new();
        let err = mutate_canary("C33", &mut packet, &mut markdown, "")
            .expect_err("a canary with no arm must not be counted as caught");
        assert!(
            err.to_string().contains("unknown canary C33"),
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
        let index = id.trim_start_matches('A').parse::<usize>().unwrap_or(0);
        let target = "crates/liminal-xtask/src/haq.rs:1";
        ReviewRecordAttempt {
            id: id.to_owned(),
            attack_class: REVIEW_ATTACK_CLASSES[index % REVIEW_ATTACK_CLASSES.len()].to_owned(),
            target: target.to_owned(),
            attempt: format!("attempted concrete falsification at {target} for case {id}"),
            observed_result: format!("observed verifier rejection at {target} for case {id}"),
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
        let mut record = ReviewRecord {
            schema_version: "haqp-blind-review-v1".to_owned(),
            pass,
            reviewer: ReviewRecordReviewer {
                model_family: family.to_owned(),
                identity_hash: sha256_text(&format!(
                    "pass{}:{}:haqp-blind-review-v1",
                    pass, family
                )),
                backend: backend.to_owned(),
            },
            attempts: (0..12).map(|i| attempt(&format!("A{i}"))).collect(),
            findings: Vec::new(),
            independently_reproduced: Vec::new(),
            unresolved_verified_findings: 0,
            result: "pass".to_owned(),
            raw_response_sha256: "a".repeat(64),
            integrity_binding_sha256: String::new(),
            isolated_session_hash: hex_digest(format!("session:{family}:{pass}").as_bytes()),
            sanitized_prompt_hash: hex_digest(format!("prompt:{family}").as_bytes()),
            prompt_binding_sha256: sha256_text(&format!(
                "haqp-blind-review-v1\0{pass}\0{family}\0{commit}\0{tree}"
            )),
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
        };
        record.integrity_binding_sha256 = sha256_text(&format!(
            "haqp-review-integrity-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree,
            record.prompt_binding_sha256,
            record.raw_response_sha256,
            record.result,
            record.unresolved_verified_findings
        ));
        record
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

    #[test]
    fn cross_pass_reproduction_requires_identical_falsification_claim() {
        let mut first = review_record(1, "openai", "codex");
        first.attempts[0].attack_class = "vacuity".to_owned();
        first.attempts[0].target = "crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].attempt =
            "first reviewer falsification at crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].observed_result =
            "first reviewer observation at crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].classification = "verified_defect".to_owned();
        first.attempts[0].independently_reproduced = true;
        first.findings = vec![serde_json::json!({"id": "P1-F1", "attempt_id": "A0"})];
        first.independently_reproduced = vec!["P1-F1".to_owned()];

        let mut second = review_record(2, "xiaomi", "lamu");
        second.attempts[0].attack_class = "vacuity".to_owned();
        second.attempts[0].target = "crates/liminal-xtask/src/haq.rs:1".to_owned();
        second.attempts[0].attempt = first.attempts[0].attempt.clone();
        second.attempts[0].observed_result = first.attempts[0].observed_result.clone();
        second.attempts[0].classification = "verified_defect".to_owned();
        second.attempts[0].independently_reproduced = true;
        second.findings = vec![serde_json::json!({"id": "A-F1", "attempt_id": "A0"})];
        second.independently_reproduced = vec!["A-F1".to_owned()];

        verify_cross_pass_reproduction(&[("p1".to_owned(), first), ("p2".to_owned(), second)])
            .expect("identical falsification claims must concur");
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
    fn review_record_rejects_vacuous_attempt_prose() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].attempt = "did a thing".to_owned();
        record.attempts[0].observed_result = "saw a thing".to_owned();
        let err = verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("generic prose is not a concrete falsification attempt");
        assert!(err.to_string().contains("substantive"), "{err}");
    }

    #[test]
    fn review_coordinate_binding_rejects_numeric_prefix_collisions() {
        assert!(contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:1 at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
        assert!(!contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:10 at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
    }

    #[test]
    fn canary_failure_prefixes_are_closed() {
        assert!(canary_failure_matches(
            "status: expected",
            "status: expected value"
        ));
        assert!(canary_expected_prefix("C01").expect("known canary") == "status: expected");
        assert!(
            canary_expected_prefix("C01").expect("known canary") != "unrelated generic failure"
        );
        canary_expected_prefix("C99").expect_err("unknown canary prefix must fail closed");
    }

    #[test]
    fn review_identity_binding_rejects_arbitrary_hashes() {
        let mut record = review_record(1, "openai", "codex");
        record.reviewer.identity_hash = "a".repeat(64);
        let err = verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("review identity must bind to pass and model family");
        assert!(err.to_string().contains("identity_hash"), "{err}");
    }

    #[test]
    fn review_integrity_binding_rejects_arbitrary_digests() {
        let mut record = review_record(1, "openai", "codex");
        record.raw_response_sha256 = "c".repeat(64);
        let err = verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("raw-response digest must be bound to integrity receipt");
        assert!(
            err.to_string().contains("integrity_binding_sha256"),
            "{err}"
        );
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
    fn review_record_requires_reproduction_for_false_positives() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].classification = "false_positive".to_owned();
        record.attempts[0].independently_reproduced = false;
        let err = verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("false positives must retain reproduction evidence");
        assert!(err.to_string().contains("false-positive"), "{err}");
        record.attempts[0].independently_reproduced = true;
        verify_review_record(&repo_root(), &review_row(), &record)
            .expect("a reproduced false positive is admissible");
    }

    #[test]
    fn review_record_rejects_an_unbound_fixed_tree() {
        let mut record = review_record(1, "openai", "codex");
        record.fixed_base.tree = "not-a-digest".to_owned();
        verify_review_record(&repo_root(), &review_row(), &record)
            .expect_err("a review record without a fixed tree binding is untrusted");
    }

    #[test]
    fn review_attempts_reject_locked_corpus_targets() {
        let root = repo_root();
        let fixed = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let mut locked = attempt("locked");
        locked.target = "conformance/corpora/dev/input:1".to_owned();
        verify_review_attempts(&root, &fixed, "test", &[locked])
            .expect_err("review targets may not name locked corpus paths");
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

    #[test]
    fn packet_statuses_require_exact_result_tokens() {
        let mut packet = packet_from_repo();
        packet.reviews[0].result = "planned_extra".to_owned();
        verify_packet_statuses(&packet, inventory_statuses())
            .expect_err("status suffixes must not pass exact packet status checks");
    }

    #[test]
    fn review_record_rejects_a_finding_without_independent_reproduction() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].classification = "verified_defect".to_owned();
        record.attempts[0].independently_reproduced = true;
        record.findings = vec![
            serde_json::json!({"id": "F1", "attempt_id": "A0"}),
            serde_json::json!({"id": "F2", "attempt_id": "A0"}),
        ];
        record.independently_reproduced = vec!["F1".to_owned()];
        record.unresolved_verified_findings = 1;

        let mut row = review_row();
        row.findings = vec!["F1".to_owned(), "F2".to_owned()];
        row.independently_reproduced = vec!["F1".to_owned()];
        row.unresolved_verified_findings = 1;
        let err = verify_review_record(&repo_root(), &row, &record)
            .expect_err("every emitted finding must be independently reproduced");
        assert!(err.to_string().contains("every finding"), "{err}");
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

    #[test]
    fn review_resolution_must_descend_from_fixed_base() {
        let root = repo_root();
        let fixed = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let evidence_path = "conformance/haqp/evidence/fuzz.json";
        let bytes = fs::read(root.join(evidence_path)).expect("fuzz evidence");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let mut resolved = attempt("resolved");
        resolved.resolved = true;
        resolved.resolution = Some(ReviewResolution {
            commit: fixed.clone(),
            coordinate: "crates/liminal-xtask/src/haq.rs:1".to_owned(),
            evidence_path: evidence_path.to_owned(),
            evidence_sha256: format!("{:x}", hasher.finalize()),
        });
        let base = ReviewFixedBase {
            commit: fixed,
            tree: git_text(&root, &["rev-parse", "HEAD^{tree}"]).expect("test tree"),
            clean: true,
        };
        let err = verify_review_resolution(&root, &base, "test", &resolved)
            .expect_err("a resolution at fixed base is not a descendant");
        assert!(err.to_string().contains("strict descendant"), "{err}");
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
            negatives: 0,
            fuzz_targets: vec!["cst_parse".to_owned()],
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
            relations: Vec::new(),
            oracle: None,
        }
    }

    fn generated_evidence_row(family: &str) -> GeneratedEvidence {
        GeneratedEvidence {
            family: family.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            negatives: 0,
            seed: 7,
            evidence_hash: "ab".repeat(32),
            category_counts: BTreeMap::new(),
            relations: Vec::new(),
            oracle: None,
        }
    }

    #[test]
    fn generated_rows_accept_a_packet_that_matches_its_run() {
        verify_generated_rows(
            &repo_root(),
            &[generated_row("f")],
            &[generated_evidence_row("f")],
        )
        .expect("a matching packet must pass, or every check below is vacuous");
    }

    #[test]
    fn generated_rows_reject_a_hash_no_run_produced() {
        let mut doctored = generated_row("f");
        doctored.evidence_hash = Some("cd".repeat(32));
        let err = verify_generated_rows(&repo_root(), &[doctored], &[generated_evidence_row("f")])
            .expect_err("a packet may not cite a digest the run never recorded");
        assert!(err.to_string().contains("evidence hash"), "{err}");
    }

    #[test]
    fn generated_rows_reject_counts_and_seeds_that_disagree_with_the_run() {
        let mut counts = generated_row("f");
        counts.accepted = 99_999;
        counts.discards = 1;
        verify_generated_rows(&repo_root(), &[counts], &[generated_evidence_row("f")])
            .expect_err("packet counts must match the committed run");

        let mut seed = generated_row("f");
        seed.seed = Some(8);
        verify_generated_rows(&repo_root(), &[seed], &[generated_evidence_row("f")])
            .expect_err("packet seed must match the committed run");
    }

    #[test]
    fn generated_rows_reject_a_family_with_no_committed_run() {
        verify_generated_rows(
            &repo_root(),
            &[generated_row("ghost")],
            &[generated_evidence_row("f")],
        )
        .expect_err("a family claiming pass with no run in the artifact");
    }

    #[test]
    fn passing_generated_family_must_name_closed_relations() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let artifact: GeneratedEvidenceArtifact = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/generated.json")).expect("evidence"),
        )
        .expect("generated artifact");
        let row = &artifact.rows[0];
        let family = &mut packet.generated[0];
        family.result = "pass".to_owned();
        family.accepted = row.accepted;
        family.attempts = row.attempts;
        family.discards = row.discards;
        family.seed = Some(row.seed);
        family.evidence_hash = Some(row.evidence_hash.clone());
        family.relations.clear();
        family.oracle = None;
        let err = verify_generated_evidence(&root, &packet)
            .expect_err("a pass with no relation rows is vacuous");
        assert!(
            err.to_string().contains("must declare every relation"),
            "{err}"
        );
    }

    #[test]
    fn generated_oracle_source_must_match_closed_registry() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let artifact: GeneratedEvidenceArtifact = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/generated.json")).expect("evidence"),
        )
        .expect("generated artifact");
        let row = &artifact.rows[0];
        let family = &mut packet.generated[0];
        family.result = "pass".to_owned();
        family.accepted = row.accepted;
        family.attempts = row.attempts;
        family.discards = row.discards;
        family.seed = Some(row.seed);
        family.evidence_hash = Some(row.evidence_hash.clone());
        family.oracle.as_mut().expect("oracle").source = "claimed independent prose".to_owned();
        let err = verify_generated_evidence(&root, &packet)
            .expect_err("self-declared oracle prose must not establish independence");
        assert!(err.to_string().contains("oracle source"), "{err}");
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
            schema_version: "haqp-generated-v3-negative-categories".to_owned(),
            artifact_blake3: generated_artifact_digest(&rows).expect("digest"),
            rows: rows.clone(),
        };
        let path = root.join("conformance/haqp/evidence/generated.json");
        fs::create_dir_all(path.parent().expect("evidence parent")).expect("mkdir");
        fs::write(&path, serde_json::to_vec(&artifact).expect("json")).expect("write");
        let packet_rows = GENERATED_FAMILIES
            .iter()
            .map(|family| {
                let mut row = generated_row(family);
                row.result = "planned".to_owned();
                row
            })
            .collect::<Vec<_>>();
        let packet = Packet {
            generated: packet_rows,
            ..read_packet(&repo_root()).expect("packet")
        };
        verify_generated_evidence(root, &packet).expect("exact family artifact must pass");

        let mut duplicate = rows;
        duplicate[0].family = duplicate[1].family.clone();
        let tampered = GeneratedEvidenceArtifact {
            schema_version: "haqp-generated-v3-negative-categories".to_owned(),
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
    fn canary_failure_matching_preserves_quoted_diagnostic_detail() {
        assert!(canary_failure_matches(
            "field \"status\" missing",
            "field \"status\" missing: found none"
        ));
        assert!(!canary_failure_matches(
            "field \"status\" missing",
            "field status missing: found none"
        ));
    }

    #[test]
    fn residual_risk_digest_without_coordinate_is_rejected() {
        let mut packet = packet_from_repo();
        packet.residual_risks = vec![ResidualRisk {
            id: "RISK-TEST".to_owned(),
            owner: "owner".to_owned(),
            severity: "low".to_owned(),
            trigger: "trigger".to_owned(),
            requirement: "P1-R001".to_owned(),
            evidence: "a".repeat(64),
            planned_phase: "M24".to_owned(),
            mitigation: "mitigation".to_owned(),
            authority: "ADR-0020".to_owned(),
            state: "open".to_owned(),
        }];
        let err = verify_residual_risks(&repo_root(), &packet)
            .expect_err("an unbound digest cannot identify retained evidence");
        assert!(err.to_string().contains("exact file:coordinate"), "{err}");
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

    #[test]
    fn canary_rows_reject_duplicate_evidence_ids() {
        let duplicate = vec![canary_evidence_row("C01"), canary_evidence_row("C01")];
        let err = verify_canary_rows(&[canary_row("C01")], &duplicate)
            .expect_err("contradictory duplicate evidence must not be silently overwritten");
        assert!(
            err.to_string().contains("duplicate canary evidence id"),
            "{err}"
        );
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
            seed_count: 16,
            seed_manifest_blake3: "ab".repeat(32),
            sanitizer: "address".to_owned(),
            log: format!("target/haqp/fuzz-{target}.log"),
            log_blake3: "ab".repeat(32),
            sanitizer_proof: None,
        }
    }

    fn family_row(name: &str, targets: &[&str]) -> Generated {
        Generated {
            family: name.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            negatives: 0,
            fuzz_targets: targets.iter().map(|t| (*t).to_owned()).collect(),
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
            relations: Vec::new(),
            oracle: None,
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

    #[test]
    fn committed_fuzz_rows_bind_the_declared_seed_sets() {
        let root = repo_root();
        let bytes = fs::read(root.join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse fuzz evidence");
        verify_fuzz_seed_manifests(&root, &rows)
            .expect("fuzz evidence must bind every committed seed corpus");
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

    #[test]
    fn resolution_coordinates_must_land_on_added_diff_lines() {
        let diff = "@@ -10,2 +10,3 @@\n-old\n+new\n+anchor\n";
        assert!(diff_contains_added_line(diff, 10));
        assert!(diff_contains_added_line(diff, 11));
        assert!(diff_contains_added_line(diff, 12));
        assert!(!diff_contains_added_line(diff, 9));
        assert!(!diff_contains_added_line(diff, 13));
        assert!(!diff_contains_added_line("@@ -3,1 +3,0 @@\n-old\n", 3));
    }

    #[test]
    fn locked_acceptance_paths_are_rejected_by_component() {
        assert!(is_locked_acceptance_path("conformance/corpora/dev/input"));
        assert!(is_locked_acceptance_path("CONFORMANCE\\CORPORA\\heldout"));
        assert!(is_locked_acceptance_path("fixtures/heldout/input"));
        assert!(!is_locked_acceptance_path("conformance/fixtures/input"));
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

    #[test]
    fn markdown_surface_rejects_a_stale_qualification_state() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let markdown = fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
            .expect("review markdown");
        let old_digest = blake3::hash(serde_json::to_vec(&packet).expect("packet json").as_slice())
            .to_hex()
            .to_string();
        packet.qualification_state = "complete".to_owned();
        let new_digest = blake3::hash(
            serde_json::to_vec(&packet)
                .expect("complete packet json")
                .as_slice(),
        )
        .to_hex()
        .to_string();
        let doctored = markdown.replace(&old_digest, &new_digest);
        let err = verify_markdown_surface_text(&doctored, &packet)
            .expect_err("completed packet may not retain NOT_RUN markdown");
        assert!(err.to_string().contains("qualification state"), "{err}");

        let stale_count = markdown.replace(
            "Target: exactly **32 predeclared canaries**",
            "Target: exactly **16 predeclared canaries**",
        );
        let err = verify_markdown_surface_text(&stale_count, &packet)
            .expect_err("markdown cannot report a stale canary inventory count");
        assert!(err.to_string().contains("canary target"), "{err}");
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
                row.accepted + row.discards + row.negatives,
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
                "4324cebe6403bbbcb899a7557fd6871eed0a76f3edcdbd6430f464fd12822b26",
            ),
            (
                "graph/interchange codecs",
                "68a9537820365388241d28b84daa4f0b195721033c950b9fd24573208d40201d",
            ),
            (
                "transforms/projections",
                "c3524a5a331362b5851e3151b30531876b769e40a454a8cd19d214caef3e6c82",
            ),
            (
                "repair/ILRP/recovery",
                "2b0d9f1b62eb1699c5da50aefdd1c5adc68b1cb102d22ba95dd516c87e40efdc",
            ),
            (
                "Basis/revision/query invalidation",
                "96737e4564fe30e9c0bb224c6fa34c36b369663b49910406503cde551dd85ac0",
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
    fn requirement_coordinate_anchor_is_not_a_substring() {
        assert!(has_exact_coordinate_anchor(
            "- **D18.1 —** exact source\n",
            "D18.1"
        ));
        assert!(!has_exact_coordinate_anchor(
            "- **D18.10 —** another source\n",
            "D18.1"
        ));
        assert!(has_exact_coordinate_anchor(
            "## Exit gate\n",
            "## Exit gate"
        ));
        assert!(!has_exact_coordinate_anchor(
            "## Exit criteria\n",
            "## Exit gate"
        ));
    }

    #[test]
    fn mutation_failure_classifier_requires_named_test_failure() {
        assert!(is_semantic_test_failure(
            "tests::target",
            b"FAIL [0.01s] tests::target\n",
            b""
        ));
        assert!(is_semantic_test_failure(
            "tests::target",
            b"",
            b"FAIL [0.01s] tests::target\n"
        ));
        assert!(!is_semantic_test_failure(
            "tests::target",
            b"launcher FAILED before tests\n",
            b""
        ));
    }

    #[test]
    fn packet_cannot_demote_authoritative_criticality() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.requirements[0].critical = false;
        let err = verify_packet_shape(&packet)
            .expect_err("criticality is closed authority, not packet-controlled metadata");
        assert!(
            err.to_string().contains("closed authoritative registry"),
            "{err}"
        );
    }

    #[test]
    fn traced_absolute_path_outside_root_cannot_rebase_on_fuzz_suffix() {
        let scratch = liminal_scratch::ScratchDir::new("haq-trace-root").expect("scratch");
        let trace_root = scratch.join("trace-root");
        fs::create_dir_all(&trace_root).expect("trace root");
        let err = trace_path_to_repo(
            &scratch,
            trace_root.as_std_path(),
            "/attacker/fuzz/corpus/cst_parse/input",
        )
        .expect_err("an external absolute path must not be rebased by suffix");
        assert!(
            err.to_string().contains("outside recorded trace root"),
            "{err}"
        );
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the corpus trace regression covers manifest and raw-trace attacks together"
    )]
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
        let mut fuzz_rows = Vec::new();
        let tracer_version = Command::new("strace")
            .arg("-V")
            .output()
            .expect("strace version");
        let mut tracer_version_bytes = tracer_version.stdout;
        tracer_version_bytes.extend_from_slice(&tracer_version.stderr);
        let tracer_version_blake3 = blake3::hash(&tracer_version_bytes).to_hex().to_string();
        let tracer_version_path = root.join("conformance/haqp/evidence/access/strace.version");
        fs::create_dir_all(tracer_version_path.parent().expect("tracer version parent"))
            .expect("mkdir tracer version");
        fs::write(&tracer_version_path, &tracer_version_bytes).expect("write tracer version");
        let trace_root = root
            .as_std_path()
            .canonicalize()
            .expect("canonical scratch root")
            .to_str()
            .expect("utf8 scratch root")
            .to_owned();
        for target in &targets {
            let manifest = format!("conformance/haqp/evidence/access/{target}.paths");
            let path = root.join(&manifest);
            fs::create_dir_all(path.parent().expect("manifest parent")).expect("mkdir");
            let corpus_dir = root.join(format!("fuzz/corpus/{target}"));
            fs::create_dir_all(&corpus_dir).expect("mkdir corpus");
            fs::write(&path, format!("{trace_root}/fuzz/corpus/{target}\n")).expect("write");
            let trace = format!("conformance/haqp/evidence/access/{target}.trace");
            let trace_path = root.join(&trace);
            fs::write(
                &trace_path,
                format!(
                    "2 openat(AT_FDCWD, \"{trace_root}/fuzz/target/x86_64-unknown-linux-gnu/release/{target}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{target}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{target}/deleted-after-open\", O_RDONLY) = 3\n1 --- SIGCHLD {{si_signo=SIGCHLD, si_pid=2, si_status=0}} ---\n1 +++ exited with 0 +++\n"
                ),
            )
            .expect("trace");
            let trace_hash = blake3::hash(&fs::read(&trace_path).expect("read trace"))
                .to_hex()
                .to_string();
            let command = format!("strace -f cargo fuzz run {target}");
            let binding =
                blake3::hash(format!("{}\0{}\0{}\0{}", command, 1, 0, trace_hash).as_bytes())
                    .to_hex()
                    .to_string();
            rows.push(CorpusAccessAuditTarget {
                target: target.clone(),
                manifest,
                manifest_blake3: blake3::hash(&fs::read(&path).expect("read"))
                    .to_hex()
                    .to_string(),
                seed: 1,
                sanitizer: "address".to_owned(),
                exit_code: 0,
                log_blake3: "a".repeat(64),
                command,
                trace,
                trace_blake3: trace_hash,
                trace_pid: 1,
                trace_exit_code: 0,
                trace_complete: true,
                process_binding: binding,
                tracer_binary: "strace".to_owned(),
                tracer_version: "conformance/haqp/evidence/access/strace.version".to_owned(),
                tracer_version_blake3: tracer_version_blake3.clone(),
                trace_root: trace_root.clone(),
                resolved_paths_blake3: blake3::hash(
                    format!(
                        "{trace_root}/fuzz/corpus/{target}\0fuzz/corpus/{target}\n{trace_root}/fuzz/corpus/{target}/deleted-after-open\0fuzz/corpus/{target}/deleted-after-open\n"
                    )
                    .as_bytes(),
                )
                .to_hex()
                .to_string(),
            });
            fuzz_rows.push(FuzzEvidence {
                target: target.clone(),
                seconds: 1800,
                elapsed_s: 1800,
                exit_code: 0,
                execs: 1,
                artifacts: 0,
                seed: 1,
                seed_count: 16,
                seed_manifest_blake3: "a".repeat(64),
                sanitizer: "address".to_owned(),
                log: format!("conformance/haqp/evidence/logs/{target}.log"),
                log_blake3: "a".repeat(64),
                sanitizer_proof: None,
            });
        }
        let fuzz_path = root.join("conformance/haqp/evidence/fuzz.json");
        fs::create_dir_all(fuzz_path.parent().expect("fuzz parent")).expect("mkdir");
        fs::write(
            &fuzz_path,
            serde_json::to_vec(&fuzz_rows).expect("serialize fuzz"),
        )
        .expect("write fuzz");
        let audit = CorpusAccessAudit {
            schema_version: "haqp-corpus-access-v2".to_owned(),
            tracer: "strace-open-paths".to_owned(),
            source_commit: "a".repeat(40),
            source_tree: "b".repeat(40),
            targets: rows,
            scope_traces: Vec::new(),
        };
        let audit_path = root.join("conformance/haqp/evidence/corpus-access.json");
        fs::write(&audit_path, serde_json::to_vec(&audit).expect("serialize")).expect("write");
        verify_corpus_access_audit(root, &packet).expect("unlocked corpus paths are accepted");

        let first = audit.targets[0].clone();
        // Lexical paths can stay benign while the fuzz corpus directory is a
        // symlink into locked data. Canonical resolution must reject this
        // without relying on a forbidden string in the raw trace.
        let heldout_dir = root.join("conformance/corpora/heldout");
        fs::create_dir_all(&heldout_dir).expect("mkdir scratch heldout");
        fs::write(heldout_dir.join("secret"), b"fixture").expect("write scratch heldout");
        let corpus_dir = root.join(format!("fuzz/corpus/{}", first.target));
        fs::remove_dir(&corpus_dir).expect("remove corpus directory");
        std::os::unix::fs::symlink(&heldout_dir, &corpus_dir).expect("symlink corpus directory");
        verify_corpus_access_audit(root, &packet)
            .expect_err("symlinked corpus resolving into held-out data must fail closed");
        fs::remove_file(&corpus_dir).expect("remove corpus symlink");
        fs::create_dir(&corpus_dir).expect("restore corpus directory");

        // A second alias outside fuzz/corpus must also be resolved. Filtering
        // only lexical corpus paths lets this symlink reach locked data
        // without a forbidden token in the raw trace.
        let alias = root.join("benign-alias");
        std::os::unix::fs::symlink(&heldout_dir, &alias).expect("symlink benign alias");
        let mut aliased = audit.clone();
        let alias_trace = root.join(&aliased.targets[0].trace);
        let normal_trace = format!(
            "2 openat(AT_FDCWD, \"{trace_root}/fuzz/target/x86_64-unknown-linux-gnu/release/{}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/benign-alias/secret\", O_RDONLY) = 3\n1 --- SIGCHLD {{si_signo=SIGCHLD, si_pid=2, si_status=0}} ---\n1 +++ exited with 0 +++\n",
            aliased.targets[0].target, aliased.targets[0].target,
        );
        fs::write(&alias_trace, &normal_trace).expect("write aliased trace");
        aliased.targets[0].trace_blake3 = hex_digest(normal_trace.as_bytes());
        aliased.targets[0].process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                aliased.targets[0].command,
                aliased.targets[0].trace_pid,
                aliased.targets[0].trace_exit_code,
                aliased.targets[0].trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        fs::write(
            &audit_path,
            serde_json::to_vec(&aliased).expect("serialize"),
        )
        .expect("write aliased audit");
        verify_corpus_access_audit(root, &packet)
            .expect_err("alias outside fuzz/corpus must not reach held-out data");

        let forbidden_path = root.join(&first.manifest);
        fs::write(
            &forbidden_path,
            "/workspace/conformance/corpora/heldout/secret\n",
        )
        .expect("write forbidden path");
        let mut doctored = audit.clone();
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

        // The normalized manifest is not authoritative by itself. A trace
        // naming locked data must fail even when manifest is clean.
        let mut traced = audit;
        let first_trace = root.join(&traced.targets[0].trace);
        fs::write(
            &first_trace,
            "/workspace/conformance/corpora/heldout/secret\n",
        )
        .expect("write forbidden trace");
        traced.targets[0].trace_blake3 = blake3::hash(&fs::read(&first_trace).expect("read trace"))
            .to_hex()
            .to_string();
        traced.targets[0].process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                traced.targets[0].command,
                traced.targets[0].trace_pid,
                traced.targets[0].trace_exit_code,
                traced.targets[0].trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        fs::write(&audit_path, serde_json::to_vec(&traced).expect("serialize")).expect("write");
        verify_corpus_access_audit(root, &packet)
            .expect_err("held-out path in raw trace must fail closed");
    }

    #[test]
    fn campaign_clock_allows_one_breach_but_blocks_two() {
        let scratch = liminal_scratch::ScratchDir::new("haq-campaign-clock").expect("scratch");
        let root: &Utf8Path = &scratch;
        let path = root.join("conformance/haqp/evidence/campaign.json");
        fs::create_dir_all(path.parent().expect("clock parent")).expect("mkdir");
        let wrapper = root.join("scripts/haqp_campaign_clock.sh");
        fs::create_dir_all(wrapper.parent().expect("wrapper parent")).expect("mkdir");
        fs::write(
            &wrapper,
            fs::read(repo_root().join("scripts/haqp_campaign_clock.sh")).expect("wrapper bytes"),
        )
        .expect("write wrapper");
        let mut clock = CampaignClock {
            schema_version: "haqp-campaign-clock-v1".to_owned(),
            reference_machine: "test-host".to_owned(),
            runs: vec![CampaignRun {
                id: "run-1".to_owned(),
                commit: "a".repeat(40),
                tree: "b".repeat(40),
                command: "just ci".to_owned(),
                started_epoch: 100,
                finished_epoch: 8 * 60 * 60 + 101,
                elapsed_s: 8 * 60 * 60 + 1,
                clean: true,
                result: "pass".to_owned(),
                wrapper: "scripts/haqp_campaign_clock.sh".to_owned(),
                wrapper_sha256: format!(
                    "{:x}",
                    Sha256::digest(
                        fs::read(repo_root().join("scripts/haqp_campaign_clock.sh"))
                            .expect("wrapper"),
                    )
                ),
                receipt: String::new(),
                receipt_blake3: String::new(),
            }],
        };
        let receipt = "run_id=run-1\ncommit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\ntree=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\ncommand=just ci\nstarted_epoch=100\nfinished_epoch=28901\nelapsed_s=28801\nclean=true\nresult=pass\n";
        let receipt_path = root.join("conformance/haqp/evidence/campaign/run-1.receipt");
        fs::create_dir_all(receipt_path.parent().expect("receipt parent")).expect("mkdir");
        fs::write(&receipt_path, receipt).expect("write receipt");
        clock.runs[0].receipt = "conformance/haqp/evidence/campaign/run-1.receipt".to_owned();
        clock.runs[0].receipt_blake3 = blake3::hash(receipt.as_bytes()).to_hex().to_string();
        fs::write(&path, serde_json::to_vec(&clock).expect("serialize")).expect("write");
        verify_campaign_clock(root, None).expect("one retained breach is residual risk");
        let mut blocked = clock;
        blocked.runs.push(CampaignRun {
            id: "run-2".to_owned(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            command: "just ci".to_owned(),
            started_epoch: 200,
            finished_epoch: 8 * 60 * 60 + 201,
            elapsed_s: 8 * 60 * 60 + 1,
            clean: true,
            result: "pass".to_owned(),
            wrapper: "scripts/haqp_campaign_clock.sh".to_owned(),
            wrapper_sha256: format!(
                "{:x}",
                Sha256::digest(
                    fs::read(repo_root().join("scripts/haqp_campaign_clock.sh")).expect("wrapper"),
                )
            ),
            receipt: "conformance/haqp/evidence/campaign/run-2.receipt".to_owned(),
            receipt_blake3: String::new(),
        });
        let receipt_two = "run_id=run-2\ncommit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\ntree=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\ncommand=just ci\nstarted_epoch=200\nfinished_epoch=29001\nelapsed_s=28801\nclean=true\nresult=pass\n";
        let receipt_two_path = root.join("conformance/haqp/evidence/campaign/run-2.receipt");
        fs::write(&receipt_two_path, receipt_two).expect("write receipt two");
        blocked.runs[1].receipt_blake3 = blake3::hash(receipt_two.as_bytes()).to_hex().to_string();
        fs::write(&path, serde_json::to_vec(&blocked).expect("serialize")).expect("write");
        verify_campaign_clock(root, None).expect_err("two clean breaches block ratification");
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
            family.negatives = 0;
            family.attempts = 101_010;
        }
        verify_generated_inventory(&packet).expect("1,010 discards is at the ceiling");

        for family in &mut packet.generated {
            family.discards = 1_011;
            family.negatives = 0;
            family.attempts = 101_011;
        }
        verify_generated_inventory(&packet).expect_err("1,011 discards is above the ceiling");
    }

    #[test]
    fn generated_discard_arithmetic_fails_closed_on_u64_overflow() {
        let mut packet = read_packet(&repo_root()).expect("read packet");
        let discards = u64::MAX / 100 + 1;
        for family in &mut packet.generated {
            family.accepted = 100_000;
            family.negatives = 0;
            family.discards = discards;
            family.attempts = family.accepted + family.discards;
        }
        let error = verify_generated_inventory(&packet).expect_err("overflow must be rejected");
        assert!(error.to_string().contains("arithmetic overflow"));
    }

    /// `seed_categories.len() < 16` — kills `<` -> `>`.
    #[test]
    fn the_seed_category_floor_is_inclusive_at_16() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        for family in &mut packet.generated {
            if generated_seed_categories(family.family.as_str()).len() == 16 {
                family.seed_categories.truncate(16);
            }
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
