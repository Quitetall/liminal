//! v4 §116 benchmark harness. Policy lives in the `liminal-benches` crate docs:
//! Law 14 — baselines recorded, no optimization and no CI regression gating
//! until the Phase 1 vertical slice activates CodSpeed via
//! `codspeed-divan-compat`.
//!
//! Real baselines run against the Phase -1 toy store (`liminal-graph`, v4 §92,
//! ADR-0007). The Phase-gated remainder of the §116 charter lives in [`future`]
//! behind the default-off `unimplemented` feature and panics by design.

use std::sync::atomic::{AtomicU64, Ordering};

use liminal_graph::{
    Node, NodeFlags, Operation, Origin, PayloadRef, Relation, RelationFlags, StoreOwner, Target,
    TxnMeta,
};
use liminal_id::{KindId, NodeId, RelationId, RevisionId, Timestamp};

/// Allocation profiler, required for the v4 §116 "memory per logical Node and
/// Relation" charter entry; today it also annotates every baseline with
/// allocation counts for free.
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// Distinguishes scratch store directories within one process (benches run
/// sequentially, but several benches open stores under the same temp root).
static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// Fresh scratch directory for a throwaway store, removed when the returned
/// guard drops (M17.5 F-12).
///
/// This used to say "deliberately never cleaned up — the OS owns its temp dir",
/// which is the assumption F-12 disproved: nothing reclaims `/tmp` until reboot,
/// it is tmpfs here so the leak is resident in RAM, and a benchmark sweep leaks
/// one store per case.
fn scratch_dir(label: &str) -> liminal_scratch::ScratchDir {
    let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
    liminal_scratch::ScratchDir::new(&format!("bench-{label}-{n}")).expect("create scratch store")
}

/// A minimal text Node (v4 §4) with a fresh id at physical revision 0.
fn text_node(payload: &str) -> Node {
    Node {
        id: NodeId::new(),
        kind: KindId(0),
        payload: PayloadRef::Text(payload.to_owned()),
        revision: RevisionId(0),
        flags: NodeFlags::default(),
    }
}

/// A minimal Relation (v4 §5) from `source` to `target` with an empty payload.
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

/// Human-origin transaction metadata (v4 §7.1) stamped now.
fn meta() -> TxnMeta {
    TxnMeta {
        actor: None,
        origin: Origin::Human,
        at: Timestamp::now(),
        provenance: None,
        inverse: None,
    }
}

/// Commit `ops` through the trusted fixture owner as one transaction.
fn commit_ops(owner: &StoreOwner, ops: Vec<Operation>) {
    let mut txn = owner.begin().expect("begin txn");
    for op in ops {
        txn.apply(op).expect("apply op");
    }
    txn.commit(meta()).expect("commit txn");
}

/// Day-one baselines against the Phase -1 toy store (v4 §92, ADR-0007).
/// Numbers are recorded, not gated (Law 14): they exist so that Phase 1
/// optimization proposals start from evidence, and so the harness itself is
/// proven real before any subsystem it will one day measure exists.
mod store_baselines {
    use liminal_graph::{Operation, StoreOwner};

    use super::{commit_ops, empty_relation, meta, scratch_dir, text_node};

    // NOTE: the planned `store_snapshot_replace` baseline is intentionally
    // absent — the toy `GraphStore` API does not expose snapshot writing
    // publicly (snapshot replacement is an internal v4 §92 concern). Add it
    // here the day the API surfaces one.

    /// Metric: latency of committing a single-op transaction — the append
    /// path of the v4 §92 checksummed log with a small realistic payload.
    #[divan::bench]
    fn store_append_txn(bencher: divan::Bencher<'_, '_>) {
        let dir = scratch_dir("append");
        let owner = StoreOwner::open(&dir).expect("open store");
        bencher.bench_local(|| {
            let mut txn = owner.begin().expect("begin txn");
            txn.apply(Operation::CreateNode {
                node: text_node("a steady-state paragraph-sized payload of ordinary prose text"),
            })
            .expect("apply op");
            txn.commit(meta()).expect("commit txn")
        });
    }

    /// Metric: the fsync-dominated floor of the commit path — the smallest
    /// possible payload so timing isolates the v4 §92 durability discipline
    /// (fsync-before-acknowledge) rather than serialization.
    #[divan::bench]
    fn store_commit_fsync(bencher: divan::Bencher<'_, '_>) {
        let dir = scratch_dir("fsync");
        let owner = StoreOwner::open(&dir).expect("open store");
        bencher.bench_local(|| {
            let mut txn = owner.begin().expect("begin txn");
            txn.apply(Operation::CreateNode {
                node: text_node(""),
            })
            .expect("apply op");
            txn.commit(meta()).expect("commit txn")
        });
    }

    /// Metric: `StoreOwner::open` recovery/replay over a log holding 1000
    /// committed operations — the cost model for the Phase -1 ILRP crash
    /// matrix (v4 §7.7, §92), where every experiment reopens the store.
    #[divan::bench(sample_count = 10, sample_size = 1)]
    fn store_recover_1k_records(bencher: divan::Bencher<'_, '_>) {
        bencher
            .with_inputs(|| {
                let dir = scratch_dir("recover");
                let owner = StoreOwner::open(&dir).expect("open store");
                for _ in 0..10 {
                    let ops = (0..100)
                        .map(|_| Operation::CreateNode {
                            node: text_node("recovery record"),
                        })
                        .collect();
                    commit_ops(&owner, ops);
                }
                dir
            })
            .bench_local_values(|dir| StoreOwner::open(&dir).expect("recover store"));
    }

    /// Metric: v4 §116 "Relation traversal" — `relations_from` on a hub Node
    /// with 999 outgoing Relations, read at the head revision.
    #[divan::bench]
    fn relation_traversal(bencher: divan::Bencher<'_, '_>) {
        let dir = scratch_dir("traversal");
        let owner = StoreOwner::open(&dir).expect("open store");
        let hub = text_node("hub");
        let hub_id = hub.id;
        let mut ops = vec![Operation::CreateNode { node: hub }];
        for _ in 0..999 {
            let spoke = text_node("spoke");
            let spoke_id = spoke.id;
            ops.push(Operation::CreateNode { node: spoke });
            ops.push(Operation::AddRelation {
                relation: empty_relation(hub_id, spoke_id),
            });
        }
        commit_ops(&owner, ops);
        let store = owner.store();
        let head = store.head().expect("head revision");
        bencher.bench_local(|| store.relations_from(head, hub_id).expect("traverse"));
    }

    /// Metric: v4 §116 "Graph query latency" — a `node_at` point lookup at the
    /// head revision of a 1000-Node store.
    #[divan::bench]
    fn graph_query_latency(bencher: divan::Bencher<'_, '_>) {
        let dir = scratch_dir("query");
        let owner = StoreOwner::open(&dir).expect("open store");
        let mut ids = Vec::with_capacity(1000);
        let mut ops = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let node = text_node("query target");
            ids.push(node.id);
            ops.push(Operation::CreateNode { node });
        }
        commit_ops(&owner, ops);
        let store = owner.store();
        let head = store.head().expect("head revision");
        let target = ids[ids.len() / 2];
        bencher.bench_local(|| store.node_at(head, target).expect("query"));
    }
}

/// Phase 1 source-to-HTML benchmarks. These measure the bounded vertical
/// slice that now exists; acceptance baselines remain a separate T1 decision.
mod phase1 {
    use std::mem::size_of;

    /// Cold one-shot source-to-HTML startup proxy.
    #[divan::bench]
    fn cold_startup(bencher: divan::Bencher<'_, '_>) {
        bencher.bench_local(|| {
            std::hint::black_box(
                liminal_format::MarkdownRenderer
                    .render("one {#a}\n\ntwo {#b}")
                    .expect("render fixture"),
            );
        });
    }

    /// Shallow logical Node/Relation footprint. Heap residency remains a
    /// separate allocator-profile measurement and is not inferred here.
    #[divan::bench]
    fn mem_per_node_and_relation(bencher: divan::Bencher<'_, '_>) {
        bencher.bench_local(|| {
            std::hint::black_box(
                size_of::<liminal_graph::Node>() + size_of::<liminal_graph::Relation>(),
            );
        });
    }
}

/// The Phase-gated remainder of the v4 §116 charter. Each bench names its
/// metric and the phase (v4 Part XXII) whose gate activates it; until then it
/// panics by design (Law 14 — the subsystem it measures is forbidden to exist,
/// so a "passing" bench here could only be measuring a lie).
#[cfg(feature = "unimplemented")]
mod future {
    /// Metric: warm `liminald` start to first served query. Activates:
    /// Phase 2 (v4 Part XXII, §116) — persistent daemons are forbidden in
    /// Phase -1.
    #[divan::bench]
    fn warm_daemon_startup() {
        unimplemented!("Phase 2 (v4 Part XXII): needs the persistent liminald daemon");
    }

    /// Metric: buffer keystroke to incremental CST update over the Liminal
    /// Document Protocol. Activates: Phase 2 (v4 Part XXII, §116) — real
    /// parsers/CSTs are forbidden in Phase -1 (Law 14).
    #[divan::bench]
    fn keystroke_to_cst_update() {
        unimplemented!("Phase 2 (v4 Part XXII): needs the incremental parser and LDP");
    }

    /// Metric: buffer keystroke to incremental HTML preview patch. Activates:
    /// Phase 2 (v4 Part XXII, §116).
    #[divan::bench]
    fn keystroke_to_html_patch() {
        unimplemented!("Phase 2 (v4 Part XXII): needs the incremental HTML preview pipeline");
    }

    /// Metric: scroll latency through a large file with lazy viewport
    /// materialization. Activates: Phase 2 (v4 Part XXII, §116) — real editor
    /// integration is forbidden in Phase -1.
    #[divan::bench]
    fn large_file_scroll_lazy_load() {
        unimplemented!("Phase 2 (v4 Part XXII): needs editor integration with lazy loading");
    }

    /// Metric: recalculation latency in the table/formula domain after a cell
    /// edit. Activates: Phase 5 (v4 Part XXII, §116).
    #[divan::bench]
    fn table_recalculation() {
        unimplemented!("Phase 5 (v4 Part XXII): needs the table/formula domain");
    }

    /// Metric: merge latency for divergent local-first histories at sync.
    /// Activates: Phase 6 (v4 Part XXII, §116) — sync is forbidden in
    /// Phase -1; no CRDTs (R4 §11.7).
    #[divan::bench]
    fn sync_merge() {
        unimplemented!("Phase 6 (v4 Part XXII): needs history and local-first synchronization");
    }

    /// Metric: load latency for multimodal resources through the resource
    /// stack. Activates: Phase 8 (v4 Part XXII, §116).
    #[divan::bench]
    fn resource_loading() {
        unimplemented!("Phase 8 (v4 Part XXII): needs the multimodal resource stack");
    }

    /// Metric: latency to compile an AI context from graph state with
    /// provenance intact (Law 15). Activates: Phase 10 (v4 Part XXII, §116).
    #[divan::bench]
    fn ai_context_compilation() {
        unimplemented!("Phase 10 (v4 Part XXII): needs the AI compiler");
    }

    /// Metric: macro expansion latency through the transform infrastructure.
    /// Activates: Phase 3 (v4 Part XXII, §116).
    #[divan::bench]
    fn macro_expansion() {
        unimplemented!("Phase 3 (v4 Part XXII): needs transform and macro infrastructure");
    }
}
