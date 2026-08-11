//! Phase 1 benchmark sampling and fail-closed comparison helpers.

use std::mem::size_of;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};
use camino::Utf8Path;
use liminal_graph::{
    GraphStore, Node, NodeFlags, Operation, Origin, PayloadRef, Relation, RelationFlags, Target,
    TxnMeta,
};
use liminal_id::{KindId, NodeId, RelationId, RevisionId, Timestamp};
use liminal_scratch::ScratchDir;
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
    let baseline_path = root.join("benches/baselines/phase1.json");
    let compared_baseline_commit = if baseline_path.exists() {
        let baseline = read_artifact(&baseline_path)?;
        validate_artifact(&baseline, "baseline")?;
        Some(baseline.subject_commit)
    } else {
        None
    };
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
        compared_baseline_commit,
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
    if candidate.subject_commit != git_head(root)? {
        anyhow::bail!("candidate subject commit is not current HEAD");
    }
    if candidate.compared_baseline_commit.as_deref() != Some(baseline.subject_commit.as_str()) {
        anyhow::bail!("candidate is not bound to accepted baseline subject commit");
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
            std::hint::black_box(size_of::<Node>() + size_of::<Relation>());
        }
        "store_append_txn" => {
            let dir = ScratchDir::new("bench-sample-append").expect("scratch directory");
            let store = GraphStore::open(&dir).expect("open store");
            commit_ops(
                &store,
                vec![Operation::CreateNode {
                    node: text_node("sample append"),
                }],
            );
        }
        "store_commit_fsync" => {
            let dir = ScratchDir::new("bench-sample-fsync").expect("scratch directory");
            let store = GraphStore::open(&dir).expect("open store");
            commit_ops(
                &store,
                vec![Operation::CreateNode {
                    node: text_node(""),
                }],
            );
        }
        "store_recover_1k_records" => {
            let dir = ScratchDir::new("bench-sample-recover").expect("scratch directory");
            let store = GraphStore::open(&dir).expect("open store");
            for _ in 0..10 {
                let ops = (0..100)
                    .map(|_| Operation::CreateNode {
                        node: text_node("recovery record"),
                    })
                    .collect();
                commit_ops(&store, ops);
            }
            drop(store);
            std::hint::black_box(GraphStore::open(&dir).expect("recover store"));
        }
        "relation_traversal" => {
            let dir = ScratchDir::new("bench-sample-traversal").expect("scratch directory");
            let store = GraphStore::open(&dir).expect("open store");
            let hub = text_node("hub");
            let hub_id = hub.id;
            let mut ops = vec![Operation::CreateNode { node: hub }];
            for _ in 0..999 {
                let spoke = text_node("spoke");
                ops.push(Operation::CreateNode {
                    node: spoke.clone(),
                });
                ops.push(Operation::AddRelation {
                    relation: empty_relation(hub_id, spoke.id),
                });
            }
            commit_ops(&store, ops);
            let head = store.head().expect("head revision");
            std::hint::black_box(store.relations_from(head, hub_id).expect("traverse"));
        }
        "graph_query_latency" => {
            let dir = ScratchDir::new("bench-sample-query").expect("scratch directory");
            let store = GraphStore::open(&dir).expect("open store");
            let mut ids = Vec::with_capacity(1000);
            let mut ops = Vec::with_capacity(1000);
            for _ in 0..1000 {
                let node = text_node("query target");
                ids.push(node.id);
                ops.push(Operation::CreateNode { node });
            }
            commit_ops(&store, ops);
            let head = store.head().expect("head revision");
            std::hint::black_box(store.node_at(head, ids[ids.len() / 2]).expect("query"));
        }
        _ => unreachable!("frozen benchmark name"),
    }
}

fn text_node(payload: &str) -> Node {
    Node {
        id: NodeId::new(),
        kind: KindId(0),
        payload: PayloadRef::Text(payload.to_owned()),
        revision: RevisionId(0),
        flags: NodeFlags::default(),
    }
}

fn empty_relation(source: NodeId, target: NodeId) -> Relation {
    Relation {
        id: RelationId::new(),
        source,
        target: Target::Node(target),
        kind: KindId(0),
        payload: PayloadRef::Text(String::new()),
        revision: RevisionId(0),
        flags: RelationFlags::default(),
        requires: None,
    }
}

fn commit_ops(store: &GraphStore, ops: Vec<Operation>) {
    let mut txn = store.begin().expect("begin transaction");
    for op in ops {
        txn.apply(op).expect("apply operation");
    }
    txn.commit(TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: None,
        inverse: None,
    })
    .expect("commit transaction");
}

fn read_artifact(path: &Utf8Path) -> Result<Artifact> {
    let bytes = std::fs::read(path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

fn validate_artifact(artifact: &Artifact, expected_kind: &str) -> Result<()> {
    if artifact.schema_version != 1 || artifact.kind != expected_kind {
        anyhow::bail!("invalid benchmark artifact kind/schema");
    }
    if artifact.subject_commit.len() != 40
        || !artifact
            .subject_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || artifact
            .subject_commit
            .bytes()
            .any(|byte| byte.is_ascii_uppercase())
    {
        anyhow::bail!("benchmark subject commit must be lowercase 40-character hex");
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
