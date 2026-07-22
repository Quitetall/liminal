//! Phase 1 benchmark sampling and fail-closed comparison helpers.

use std::mem::size_of;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};
use camino::Utf8Path;
use serde::{Deserialize, Serialize};

const BENCHMARKS: [&str; 7] = [
    "cold_startup",
    "mem_per_node_and_relation",
    "store_append_txn",
    "store_commit_fsync",
    "store_recover_1k_records",
    "relation_traversal",
    "graph_query_latency",
];

/// Capture exactly `sample_count` raw samples for frozen Phase 1 names.
pub fn sample_repo(root: &Utf8Path, sample_count: usize) -> Result<()> {
    if sample_count != 30 {
        anyhow::bail!("Phase 1 benchmark sample count must be exactly 30, got {sample_count}");
    }
    let mut rows = Vec::new();
    for name in BENCHMARKS {
        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let started = Instant::now();
            exercise(name);
            samples.push(
                u64::try_from(started.elapsed().as_nanos()).expect("sample duration fits u64"),
            );
        }
        samples.sort_unstable();
        let median = u64::midpoint(samples[14], samples[15]);
        let p95 = samples[28];
        rows.push(Benchmark {
            name: name.to_owned(),
            unit: "ns".to_owned(),
            samples,
            median,
            p95,
        });
    }
    let candidate = Artifact {
        schema_version: 1,
        kind: "candidate".to_owned(),
        subject_commit: git_head(root)?,
        compared_baseline_commit: None,
        reference_environment: reference_environment(),
        toolchain: rustc_version(),
        sample_count,
        benchmarks: rows,
    };
    let path = root.join("benches/baselines/phase1-candidate.json");
    std::fs::create_dir_all(path.parent().expect("candidate parent"))?;
    std::fs::write(&path, serde_json::to_vec_pretty(&candidate)?)?;
    println!("wrote {path}");
    Ok(())
}

/// Validate committed baseline internals without accepting it as policy.
pub fn baseline_check_repo(root: &Utf8Path) -> Result<()> {
    let path = root.join("benches/baselines/phase1.json");
    let artifact = read_artifact(&path)?;
    validate_artifact(&artifact, "baseline")?;
    println!("baseline valid: {path}");
    Ok(())
}

/// Compare candidate against an accepted baseline using checked integer bounds.
pub fn gate_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_artifact(&root.join("benches/baselines/phase1.json"))?;
    let candidate = read_artifact(&root.join("benches/baselines/phase1-candidate.json"))?;
    validate_artifact(&baseline, "baseline")?;
    validate_artifact(&candidate, "candidate")?;
    if baseline.reference_environment != candidate.reference_environment {
        anyhow::bail!("benchmark reference environment mismatch");
    }
    if baseline.toolchain != candidate.toolchain {
        anyhow::bail!("benchmark toolchain mismatch");
    }
    for (base, cand) in baseline.benchmarks.iter().zip(&candidate.benchmarks) {
        if base.name != cand.name {
            anyhow::bail!(
                "benchmark order/name mismatch: {} vs {}",
                base.name,
                cand.name
            );
        }
        if u128::from(cand.median) * 100 > u128::from(base.median) * 110 {
            anyhow::bail!("{} median exceeds 10% threshold", base.name);
        }
        if u128::from(cand.p95) * 100 > u128::from(base.p95) * 115 {
            anyhow::bail!("{} p95 exceeds 15% threshold", base.name);
        }
    }
    println!("benchmark gate passed");
    Ok(())
}

fn exercise(name: &str) {
    match name {
        "cold_startup" => {
            let _ = liminal_format::MarkdownRenderer.render("one {#a}\n\ntwo {#b}");
        }
        "mem_per_node_and_relation" => {
            std::hint::black_box(
                size_of::<liminal_graph::Node>() + size_of::<liminal_graph::Relation>(),
            );
        }
        "store_append_txn" => {
            // Phase 1 registry proxy: the real store benchmark remains
            // pending store implementation and must not be baseline-blessed.
            std::hint::black_box(blake3::hash(b"append"));
        }
        "store_commit_fsync" => {
            // Phase 1 registry proxy; no durable baseline is accepted here.
            std::hint::black_box(blake3::hash(b"fsync"));
        }
        "store_recover_1k_records" => {
            // Phase 1 registry proxy; recovery benchmark awaits real store.
            std::hint::black_box(blake3::hash(&[0u8; 1024]));
        }
        "relation_traversal" => {
            // Phase 1 registry proxy; real graph traversal remains pending.
            std::hint::black_box((0..999u64).sum::<u64>());
        }
        "graph_query_latency" => {
            // Phase 1 registry proxy; real query benchmark remains pending.
            std::hint::black_box(blake3::hash(b"query"));
        }
        _ => unreachable!("frozen benchmark name"),
    }
}

fn read_artifact(path: &Utf8Path) -> Result<Artifact> {
    let bytes = std::fs::read(path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

fn validate_artifact(artifact: &Artifact, expected_kind: &str) -> Result<()> {
    if artifact.schema_version != 1 || artifact.kind != expected_kind {
        anyhow::bail!("invalid benchmark artifact kind/schema");
    }
    if artifact.sample_count != 30 || artifact.benchmarks.len() != BENCHMARKS.len() {
        anyhow::bail!("benchmark sample inventory must be exactly seven x 30");
    }
    for (expected, row) in BENCHMARKS.iter().zip(&artifact.benchmarks) {
        if row.name != *expected || row.samples.len() != 30 {
            anyhow::bail!("benchmark samples not frozen/sorted for {expected}");
        }
        let mut sorted = row.samples.clone();
        sorted.sort_unstable();
        if sorted != row.samples || row.p95 != row.samples[28] {
            anyhow::bail!("benchmark samples must be sorted with nearest-rank p95");
        }
        let median = u64::midpoint(row.samples[14], row.samples[15]);
        if row.median != median {
            anyhow::bail!("benchmark median mismatch for {expected}");
        }
    }
    Ok(())
}

fn git_head(root: &Utf8Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", root.as_str(), "rev-parse", "HEAD"])
        .output()
        .context("git rev-parse HEAD")?;
    if !output.status.success() {
        anyhow::bail!("git rev-parse HEAD failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |value| value.trim().to_owned())
}

fn reference_environment() -> String {
    "linux-x86_64-local".to_owned()
}

#[derive(Debug, Serialize, Deserialize)]
struct Artifact {
    schema_version: u32,
    #[serde(rename = "artifact_kind")]
    kind: String,
    subject_commit: String,
    compared_baseline_commit: Option<String>,
    reference_environment: String,
    toolchain: String,
    sample_count: usize,
    benchmarks: Vec<Benchmark>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Benchmark {
    name: String,
    unit: String,
    samples: Vec<u64>,
    median: u64,
    p95: u64,
}
