//! Phase -1 M09 spike 2: rich-editor ops → graph transactions + undo fidelity.
//!
//! This is a throwaway spike — `publish = false`, no workspace lints,
//! trivially deletable at box end. `#[allow(...)]` blocks are acceptable
//! (non-production).
//!
//! # Spec
//! - Op set: `insert_text`, `delete_range`, `add_mark`, `split_block`,
//!   `join_block`, `undo`
//! - Graph-op mapping: each editor op → a `RichTxn` of graph `Operation`s
//! - Undo: replay recorded inverse ops (never byte restoration, R4 §11.3)
//! - Concurrency: one interleaved concurrent op between op and undo
//! - Measurement: undo-fidelity tally across 8 seeds

use liminal_conformance::identity::Config;
use liminal_graph::Operation as GraphOp;
use liminal_graph::{Node, NodeFlags, PayloadRef};
use liminal_id::{KindId, NodeId, RelationId, RevisionId};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use uuid::Uuid;

pub const BOX_START: &str = "2026-07-18";

// ── ID helpers ──────────────────────────────────────────────────────────

fn node_id(n: u64) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(n as u128))
}

fn relation_id(n: u64) -> RelationId {
    RelationId::from_uuid(Uuid::from_u128(n as u128 + 1_000_000))
}

// ── Data model ──────────────────────────────────────────────────────────

/// A paragraph block in the rich document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: NodeId,
    pub text: String,
}

/// A mark/annotation on a block (a Relation in the graph model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub id: RelationId,
    pub target: NodeId,
    /// Character range within the block text (start, end). None = whole block.
    pub range: Option<(usize, usize)>,
    pub kind: &'static str,
    pub payload: String,
}

/// The rich document — blocks + marks + graph state.
#[derive(Debug, Clone)]
pub struct RichDoc {
    pub blocks: Vec<Block>,
    pub marks: Vec<Mark>,
    /// Ordered root-level block IDs.
    pub root_order: Vec<NodeId>,
    /// Undo stack: each entry is the inverse `RichTxn` for that operation.
    pub undo_stack: Vec<RichTxn>,
    /// Forward transaction log (for measurement).
    pub txns: Vec<RichTxn>,
}

/// A graph transaction: forward ops + inverse ops (R4 §6, §11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichTxn {
    pub forward: Vec<GraphOp>,
    pub inverse: Vec<GraphOp>,
    /// Mark ranges to restore on undo: (mark_id, original_range).
    pub mark_restores: Vec<(RelationId, Option<(usize, usize)>)>,
}

/// The outcome of an undo attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoOutcome {
    Faithful,
    Diverged,
    Empty,
}

/// A snapshot of document state for fidelity comparison.
pub type DocSnapshot = Vec<(NodeId, String, Vec<(RelationId, Option<(usize, usize)>)>)>;

// ── RichDoc construction ────────────────────────────────────────────────

impl RichDoc {
    /// Build a rich document from the M07 template text for a given seed.
    /// Each paragraph block becomes a graph Node; 2 marks per block.
    pub fn from_template(template: &str, seed: u64) -> Self {
        let blocks = liminal_source::paragraph::parse(template);
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut doc = Self {
            blocks: Vec::new(),
            marks: Vec::new(),
            root_order: Vec::new(),
            undo_stack: Vec::new(),
            txns: Vec::new(),
        };

        for (i, block) in blocks.iter().enumerate() {
            let nid = node_id((i as u64) + 1);
            doc.blocks.push(Block {
                id: nid,
                text: block.text.clone(),
            });
            doc.root_order.push(nid);

            // 2 marks per block: one whole-block, one range.
            let block_len = block.text.len();
            for j in 0..2u32 {
                let rid = relation_id((i as u64) * 10 + j as u64 + 1);
                let (kind, range) = if j == 0 {
                    ("highlight", None)
                } else if block_len >= 4 {
                    let s = rng.random_range(0..block_len - 1);
                    let e = rng.random_range(s + 1..=block_len.min(s + block_len / 2 + 1));
                    ("link", Some((s, e)))
                } else {
                    ("link", None)
                };
                doc.marks.push(Mark {
                    id: rid,
                    target: nid,
                    range,
                    kind,
                    payload: format!("mark-{seed}-{i}-{j}"),
                });
            }
        }
        doc
    }

    /// Snapshot the document state for fidelity comparison.
    pub fn snapshot(&self) -> DocSnapshot {
        let mut s: Vec<_> = self
            .blocks
            .iter()
            .map(|b| {
                let marks: Vec<_> = self
                    .marks
                    .iter()
                    .filter(|m| m.target == b.id)
                    .map(|m| (m.id, m.range))
                    .collect();
                (b.id, b.text.clone(), marks)
            })
            .collect();
        s.sort_by_key(|(id, _, _)| *id);
        s
    }

    fn block_mut(&mut self, id: NodeId) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    fn block(&self, id: NodeId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    fn pick_block(&self, rng: &mut SmallRng) -> NodeId {
        let idx = rng.random_range(0..self.blocks.len());
        self.blocks[idx].id
    }

    fn next_node_id(&self) -> NodeId {
        node_id(self.blocks.iter().map(|b| b.id.as_uuid().as_u128() as u64).max().unwrap_or(0) + 1)
    }

    fn next_relation_id(&self) -> RelationId {
        relation_id(self.marks.iter().map(|m| (m.id.as_uuid().as_u128() as u64).saturating_sub(1_000_000)).max().unwrap_or(0) + 1)
    }
}

// ── Editor operations ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichOp {
    InsertText,
    DeleteRange,
    AddMark,
    SplitBlock,
    JoinBlock,
}

impl RichOp {
    pub const ALL: [RichOp; 5] = [
        RichOp::InsertText,
        RichOp::DeleteRange,
        RichOp::AddMark,
        RichOp::SplitBlock,
        RichOp::JoinBlock,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RichOp::InsertText => "insert_text",
            RichOp::DeleteRange => "delete_range",
            RichOp::AddMark => "add_mark",
            RichOp::SplitBlock => "split_block",
            RichOp::JoinBlock => "join_block",
        }
    }
}

/// Apply a rich-edit operation to the document. Returns the graph transaction.
pub fn apply_rich_op(doc: &mut RichDoc, op: RichOp, seed: u64) -> RichTxn {
    let mut rng = SmallRng::seed_from_u64(seed.wrapping_add(999));
    match op {
        RichOp::InsertText => op_insert_text(doc, &mut rng),
        RichOp::DeleteRange => op_delete_range(doc, &mut rng),
        RichOp::AddMark => op_add_mark(doc, &mut rng),
        RichOp::SplitBlock => op_split_block(doc, &mut rng),
        RichOp::JoinBlock => op_join_block(doc, &mut rng),
    }
}

fn op_insert_text(doc: &mut RichDoc, rng: &mut SmallRng) -> RichTxn {
    let bid = doc.pick_block(rng);
    let block = doc.block(bid).unwrap().clone();
    let ins = rng.random_range(0..=block.text.len());
    let snippet = format!("<ins-{}>", rng.random_range(100..999));
    let new_text = format!("{}{}{}", &block.text[..ins], snippet, &block.text[ins..]);

    let forward = vec![GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(new_text.clone()) }];
    let inverse = vec![GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(block.text.clone()) }];

    // Capture mark ranges BEFORE mutation for undo.
    let mark_restores: Vec<_> = doc.marks.iter()
        .filter(|m| m.target == bid)
        .map(|m| (m.id, m.range))
        .collect();

    let bm = doc.block_mut(bid).unwrap();
    bm.text = new_text;

    // Retarget mark ranges affected by the insertion.
    for mark in &mut doc.marks {
        if mark.target == bid {
            if let Some((ref mut s, ref mut e)) = mark.range {
                if ins <= *s {
                    *s += snippet.len();
                    *e += snippet.len();
                } else if ins < *e {
                    *e += snippet.len();
                }
            }
        }
    }

    RichTxn { forward, inverse, mark_restores }
}

fn op_delete_range(doc: &mut RichDoc, rng: &mut SmallRng) -> RichTxn {
    let bid = doc.pick_block(rng);
    let block = doc.block(bid).unwrap().clone();
    let len = block.text.len();
    if len < 2 {
        return RichTxn { forward: vec![], inverse: vec![], mark_restores: vec![] };
    }
    let start = rng.random_range(0..len - 1);
    let end = rng.random_range(start + 1..=len);
    let del_len = end - start;
    let new_text = format!("{}{}", &block.text[..start], &block.text[end..]);

    let forward = vec![GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(new_text.clone()) }];
    let inverse = vec![GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(block.text.clone()) }];

    // Capture mark ranges BEFORE mutation for undo.
    let mark_restores: Vec<_> = doc.marks.iter()
        .filter(|m| m.target == bid)
        .map(|m| (m.id, m.range))
        .collect();

    let bm = doc.block_mut(bid).unwrap();
    bm.text = new_text;

    for mark in &mut doc.marks {
        if mark.target == bid {
            if let Some((ref mut s, ref mut e)) = mark.range {
                if start <= *s && end >= *e {
                    *s = start;
                    *e = start;
                } else if start <= *s {
                    *s = s.saturating_sub(del_len);
                    *e = e.saturating_sub(del_len);
                } else if start < *e {
                    *e = e.saturating_sub(del_len);
                }
            }
        }
    }

    RichTxn { forward, inverse, mark_restores }
}

fn op_add_mark(doc: &mut RichDoc, rng: &mut SmallRng) -> RichTxn {
    let bid = doc.pick_block(rng);
    let block = doc.block(bid).unwrap().clone();
    let rid = doc.next_relation_id();
    let len = block.text.len();
    let range = if len >= 4 {
        let s = rng.random_range(0..len - 1);
        let e = rng.random_range(s + 1..=len.min(s + len / 2 + 1));
        Some((s, e))
    } else {
        None
    };

    let mark = Mark { id: rid, target: bid, range, kind: "comment", payload: format!("new-mark-{}", rng.random_range(0..999)) };

    let graph_relation = liminal_graph::Relation {
        id: rid,
        source: bid,
        target: liminal_graph::Target::Anchored { node: bid, anchor: liminal_graph::AnchorRef { description: format!("{range:?}") } },
        kind: KindId(0),
        payload: PayloadRef::Text(mark.payload.clone()),
        revision: RevisionId(0),
        flags: liminal_graph::RelationFlags::default(),
        requires: None,
    };
    let forward = vec![GraphOp::AddRelation { relation: graph_relation }];
    let inverse = vec![GraphOp::RemoveRelation { id: rid }];

    doc.marks.push(mark);

    RichTxn { forward, inverse, mark_restores: vec![] }
}

fn op_split_block(doc: &mut RichDoc, rng: &mut SmallRng) -> RichTxn {
    let bid = doc.pick_block(rng);
    let block = doc.block(bid).unwrap().clone();
    let len = block.text.len();
    if len < 4 {
        return RichTxn { forward: vec![], inverse: vec![], mark_restores: vec![] };
    }
    let split_at = rng.random_range(1..len - 1);
    let first_text = block.text[..split_at].to_owned();
    let second_text = block.text[split_at..].to_owned();
    let new_id = doc.next_node_id();

    // Capture mark ranges BEFORE mutation for undo.
    let mark_restores: Vec<_> = doc.marks.iter()
        .map(|m| (m.id, m.range))
        .collect();

    let forward = vec![
        GraphOp::CreateNode { node: Node { id: new_id, kind: KindId(0), payload: PayloadRef::Text(second_text.clone()), revision: RevisionId(0), flags: NodeFlags::default() } },
        GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(first_text.clone()) },
        GraphOp::InsertChild { parent: bid, child: new_id, index: 0 },
    ];
    let inverse = vec![
        GraphOp::SetPayload { id: bid, payload: PayloadRef::Text(block.text.clone()) },
        GraphOp::DeleteNode { id: new_id },
    ];

    // Retarget marks whose range falls in the second half.
    let mut new_marks = Vec::new();
    for mark in &mut doc.marks {
        if mark.target == bid {
            if let Some((s, e)) = mark.range {
                if s >= split_at {
                    new_marks.push(Mark { id: mark.id, target: new_id, range: Some((s - split_at, e - split_at)), kind: mark.kind, payload: mark.payload.clone() });
                    mark.range = Some((0, 0));
                } else if e > split_at {
                    mark.range = Some((s, split_at));
                }
            }
        }
    }

    let bm = doc.block_mut(bid).unwrap();
    bm.text = first_text;
    doc.blocks.push(Block { id: new_id, text: second_text });
    if let Some(pos) = doc.root_order.iter().position(|&id| id == bid) {
        doc.root_order.insert(pos + 1, new_id);
    }
    doc.marks.extend(new_marks);

    RichTxn { forward, inverse, mark_restores }
}

fn op_join_block(doc: &mut RichDoc, rng: &mut SmallRng) -> RichTxn {
    let candidates: Vec<usize> = doc
        .root_order
        .iter()
        .enumerate()
        .filter_map(|(i, &id)| {
            if doc.blocks.iter().any(|b| b.id == id) && i + 1 < doc.root_order.len() {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    if candidates.is_empty() {
        return RichTxn { forward: vec![], inverse: vec![], mark_restores: vec![] };
    }
    let pos = candidates[rng.random_range(0..candidates.len())];

    let first_id = doc.root_order[pos];
    let second_id = doc.root_order[pos + 1];
    // Capture mark ranges BEFORE mutation for undo.
    let mark_restores: Vec<_> = doc.marks.iter()
        .map(|m| (m.id, m.range))
        .collect();

    let first = doc.block(first_id).unwrap().clone();
    let second = doc.block(second_id).unwrap().clone();
    let joined_text = format!("{}{}", first.text, second.text);

    let forward = vec![
        GraphOp::SetPayload { id: first_id, payload: PayloadRef::Text(joined_text.clone()) },
        GraphOp::DeleteNode { id: second_id },
    ];
    let inverse = vec![
        GraphOp::SetPayload { id: first_id, payload: PayloadRef::Text(first.text.clone()) },
        GraphOp::CreateNode { node: Node { id: second_id, kind: KindId(0), payload: PayloadRef::Text(second.text.clone()), revision: RevisionId(0), flags: NodeFlags::default() } },
        GraphOp::InsertChild { parent: first_id, child: second_id, index: 0 },
    ];

    let offset = first.text.len();
    for mark in &mut doc.marks {
        if mark.target == second_id {
            mark.target = first_id;
            if let Some((ref mut s, ref mut e)) = mark.range {
                *s += offset;
                *e += offset;
            }
        }
    }

    let bm = doc.block_mut(first_id).unwrap();
    bm.text = joined_text;
    doc.blocks.retain(|b| b.id != second_id);
    doc.root_order.remove(pos + 1);

    RichTxn { forward, inverse, mark_restores }
}

/// Undo the last operation by replaying its inverse graph ops + mark restores.
pub fn undo(doc: &mut RichDoc, pre_snapshot: &DocSnapshot) -> UndoOutcome {
    let txn = match doc.undo_stack.pop() {
        Some(t) => t,
        None => return UndoOutcome::Empty,
    };

    for op in &txn.inverse {
        apply_graph_op_inverse(doc, op);
    }

    // Restore mark ranges from before the forward op.
    for (mid, range) in &txn.mark_restores {
        if let Some(mark) = doc.marks.iter_mut().find(|m| m.id == *mid) {
            mark.range = *range;
        }
    }

    let post = doc.snapshot();
    if post == *pre_snapshot { UndoOutcome::Faithful } else { UndoOutcome::Diverged }
}

fn apply_graph_op_inverse(doc: &mut RichDoc, op: &GraphOp) {
    match op {
        GraphOp::SetPayload { id, payload } => {
            if let PayloadRef::Text(text) = payload {
                if let Some(block) = doc.block_mut(*id) {
                    block.text = text.clone();
                }
            }
        }
        GraphOp::CreateNode { node } => {
            if !doc.blocks.iter().any(|b| b.id == node.id) {
                let text = match &node.payload { PayloadRef::Text(t) => t.clone(), _ => String::new() };
                doc.blocks.push(Block { id: node.id, text });
                if !doc.root_order.contains(&node.id) {
                    doc.root_order.push(node.id);
                }
            }
        }
        GraphOp::DeleteNode { id } => {
            doc.blocks.retain(|b| b.id != *id);
            doc.root_order.retain(|nid| *nid != *id);
            doc.marks.retain(|m| m.target != *id);
        }
        GraphOp::InsertChild { child, .. } => {
            if !doc.root_order.contains(child) {
                doc.root_order.push(*child);
            }
        }
        GraphOp::RemoveRelation { id } => {
            doc.marks.retain(|m| m.id != *id);
        }
        _ => {}
    }
}

// ── Measurement loop ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RichOpTally {
    pub op_name: String,
    pub undo_faithful: usize,
    pub undo_diverged: usize,
    pub undo_total: usize,
    pub concurrent_undo_faithful: usize,
    pub concurrent_undo_diverged: usize,
    pub concurrent_undo_total: usize,
}

impl RichOpTally {
    pub fn fidelity_pct(&self) -> f64 {
        let total = self.undo_total + self.concurrent_undo_total;
        if total == 0 { return 100.0; }
        (self.undo_faithful + self.concurrent_undo_faithful) as f64 / total as f64 * 100.0
    }
}

/// Run the rich-edit loop: 5 ops × 8 seeds, measuring undo fidelity
/// (simple + concurrent).
pub fn run_richedit_loop(cfg: &Config) -> Vec<RichOpTally> {
    let mut results = Vec::new();

    for &op in &RichOp::ALL {
        let mut tally = RichOpTally {
            op_name: op.name().to_owned(),
            undo_faithful: 0, undo_diverged: 0, undo_total: 0,
            concurrent_undo_faithful: 0, concurrent_undo_diverged: 0, concurrent_undo_total: 0,
        };

        for &seed in &cfg.seeds {
            // ── Simple undo test ──
            {
                let mut doc = RichDoc::from_template(&cfg.template, seed);
                let pre = doc.snapshot();
                let txn = apply_rich_op(&mut doc, op, seed);
                if !txn.forward.is_empty() {
                    doc.undo_stack.push(txn);
                    let outcome = undo(&mut doc, &pre);
                    tally.undo_total += 1;
                    match outcome {
                        UndoOutcome::Faithful => tally.undo_faithful += 1,
                        UndoOutcome::Diverged => tally.undo_diverged += 1,
                        UndoOutcome::Empty => {}
                    }
                }
            }

            // ── Concurrent undo test ──
            {
                let mut doc = RichDoc::from_template(&cfg.template, seed);
                let pre = doc.snapshot();
                let txn = apply_rich_op(&mut doc, op, seed);
                if !txn.forward.is_empty() {
                    doc.undo_stack.push(txn);
                    let _concurrent = apply_rich_op(&mut doc, RichOp::InsertText, seed.wrapping_add(777));
                    let outcome = undo(&mut doc, &pre);
                    tally.concurrent_undo_total += 1;
                    match outcome {
                        UndoOutcome::Faithful => tally.concurrent_undo_faithful += 1,
                        UndoOutcome::Diverged => tally.concurrent_undo_diverged += 1,
                        UndoOutcome::Empty => {}
                    }
                }
            }
        }

        results.push(tally);
    }

    results
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config { Config::load().expect("load config") }

    #[test]
    fn template_produces_blocks_and_marks() {
        let c = cfg();
        let doc = RichDoc::from_template(&c.template, 33);
        assert!(!doc.blocks.is_empty());
        assert!(!doc.marks.is_empty());
        assert_eq!(doc.marks.len(), doc.blocks.len() * 2);
    }

    #[test]
    fn insert_text_has_txn() {
        let c = cfg();
        let mut doc = RichDoc::from_template(&c.template, 33);
        let txn = apply_rich_op(&mut doc, RichOp::InsertText, 33);
        assert!(!txn.forward.is_empty());
        assert!(!txn.inverse.is_empty());
    }

    #[test]
    fn split_block_creates_new_block() {
        let c = cfg();
        let mut doc = RichDoc::from_template(&c.template, 33);
        let before = doc.blocks.len();
        let txn = apply_rich_op(&mut doc, RichOp::SplitBlock, 33);
        if !txn.forward.is_empty() {
            assert_eq!(doc.blocks.len(), before + 1);
        }
    }

    #[test]
    fn join_block_removes_block() {
        let c = cfg();
        let mut doc = RichDoc::from_template(&c.template, 33);
        let before = doc.blocks.len();
        let txn = apply_rich_op(&mut doc, RichOp::JoinBlock, 33);
        if !txn.forward.is_empty() {
            assert_eq!(doc.blocks.len(), before - 1);
        }
    }

    #[test]
    fn add_mark_creates_mark() {
        let c = cfg();
        let mut doc = RichDoc::from_template(&c.template, 33);
        let before = doc.marks.len();
        apply_rich_op(&mut doc, RichOp::AddMark, 33);
        assert_eq!(doc.marks.len(), before + 1);
    }

    #[test]
    fn undo_insert_text_faithful() {
        let c = cfg();
        let mut doc = RichDoc::from_template(&c.template, 33);
        let pre = doc.snapshot();
        let txn = apply_rich_op(&mut doc, RichOp::InsertText, 33);
        doc.undo_stack.push(txn);
        let outcome = undo(&mut doc, &pre);
        assert_eq!(outcome, UndoOutcome::Faithful);
    }

    #[test]
    fn richedit_loop_completes() {
        let c = cfg();
        let results = run_richedit_loop(&c);
        assert_eq!(results.len(), 5);
        for r in &results {
            assert!(r.undo_total > 0, "{}: no undos attempted", r.op_name);
        }
    }

    #[test]
    fn graph_txns_have_both_directions() {
        let c = cfg();
        for &seed in &c.seeds {
            for &op in &RichOp::ALL {
                let mut doc = RichDoc::from_template(&c.template, seed);
                let txn = apply_rich_op(&mut doc, op, seed);
                if !txn.forward.is_empty() {
                    assert!(!txn.inverse.is_empty(), "{} seed {}: forward but no inverse", op.name(), seed);
                }
            }
        }
    }
}
