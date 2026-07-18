//! Three-way block merge over the toy paragraph grammar (AM-4.3; R4 §6).
//!
//! Determinism is not safety: a UNIQUE merge result that touches the same block
//! from both sides is a `UniqueOverlap` — a candidate the safety predicate must
//! refuse (R4 §6). Only edits to disjoint blocks are `Disjoint` (auto-safe).
//!
//! Blocks pair across versions by `{#id}` when present on both sides;
//! anonymous blocks pair by index among the anonymous blocks in order (D04.2).

use std::collections::{BTreeMap, BTreeSet};

use crate::paragraph::{self, Block};

/// The outcome of a three-way merge (Data schemas, M04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// No aligned slot changed by both sides (or identical changes): merged
    /// result is unique AND structurally disjoint.
    Disjoint {
        /// The merged source text.
        merged: String,
    },
    /// A unique candidate exists but >= 1 slot was changed by both sides
    /// (line-disjoint within the block): deterministic, NOT safe (R4 §6).
    UniqueOverlap {
        /// The merged source text.
        merged: String,
        /// Slot keys where both sides edited the same block line-disjointly.
        overlapping: Vec<String>,
    },
    /// No unique candidate (same lines touched incompatibly).
    Conflict,
}

/// Compute a three-way merge of `base` → (`ours`, `theirs`).
pub fn three_way(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    let base_slots = slot_map(base);
    let ours_slots = slot_map(ours);
    let theirs_slots = slot_map(theirs);

    // Union of all slot keys across the three versions, in a stable order:
    // base order first (defines primary layout), then ours-only insertions,
    // then theirs-only insertions.
    let mut order: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for key in slot_order(base) {
        if seen.insert(key.clone()) {
            order.push(key);
        }
    }
    for key in slot_order(ours) {
        if seen.insert(key.clone()) {
            order.push(key);
        }
    }
    for key in slot_order(theirs) {
        if seen.insert(key.clone()) {
            order.push(key);
        }
    }

    let mut merged_blocks: Vec<String> = Vec::new();
    let mut overlapping: Vec<String> = Vec::new();

    for key in &order {
        let base_t = base_slots.get(key);
        let ours_t = ours_slots.get(key);
        let theirs_t = theirs_slots.get(key);

        let ours_changed = ours_t != base_t;
        let theirs_changed = theirs_t != base_t;

        let resolved: Option<&String> = match (ours_changed, theirs_changed) {
            (false, false) => base_t,
            (true, false) => ours_t,
            (false, true) => theirs_t,
            (true, true) => {
                if ours_t == theirs_t {
                    // Identical change from both sides — still disjoint.
                    ours_t
                } else {
                    // Both edited the same block differently. Attempt a
                    // line-disjoint merge; otherwise it is a Conflict.
                    match line_merge(base_t, ours_t, theirs_t) {
                        Some(merged) => {
                            overlapping.push(key.clone());
                            merged_blocks.push(merged);
                            continue;
                        }
                        None => return MergeOutcome::Conflict,
                    }
                }
            }
        };

        if let Some(text) = resolved {
            merged_blocks.push(text.clone());
        }
        // resolved == None means the slot was deleted on the changing side;
        // deletions are honored by simply not emitting the block.
    }

    let merged = merged_blocks.join("\n\n");
    if overlapping.is_empty() {
        MergeOutcome::Disjoint { merged }
    } else {
        MergeOutcome::UniqueOverlap {
            merged,
            overlapping,
        }
    }
}

/// Slots that `merged` changed relative to `current` in a block `current` had
/// already changed vs `base` — i.e. slots BOTH sides touched (DG-4.1 resolution).
///
/// This lets a safety predicate detect structural non-disjointness from
/// `(base, current, merged)` alone, without the raw "ours" buffer: a slot
/// overlaps iff `current != base` there AND `merged != current` there. Empty
/// result ⇒ the merge is structurally disjoint (auto-safe, R4 §6).
#[must_use]
pub fn overlap_slots(base: &str, current: &str, merged: &str) -> Vec<String> {
    let base_slots = slot_map(base);
    let current_slots = slot_map(current);
    let merged_slots = slot_map(merged);
    let mut overlapping = Vec::new();
    for key in slot_order(current) {
        let b = base_slots.get(&key);
        let c = current_slots.get(&key);
        let m = merged_slots.get(&key);
        if c != b && m != c {
            overlapping.push(key);
        }
    }
    overlapping
}

/// The reconstructable source text of a block: its text plus the `{#id}` marker
/// appended to the last line when present (so a merged block round-trips).
fn block_source(block: &Block) -> String {
    if let Some(id) = &block.id {
        format!("{} {{#{id}}}", block.text)
    } else {
        block.text.clone()
    }
}

/// Slot key for a block (D04.2): the `{#id}` when present, else `anon:<index>`
/// where index counts anonymous blocks in order.
fn slot_map(input: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut anon = 0usize;
    for block in paragraph::parse(input) {
        let key = if let Some(id) = &block.id {
            id.clone()
        } else {
            let k = format!("anon:{anon}");
            anon += 1;
            k
        };
        out.insert(key, block_source(&block));
    }
    out
}

/// Slot keys in document order (for stable merged output layout).
fn slot_order(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut anon = 0usize;
    for block in paragraph::parse(input) {
        if let Some(id) = &block.id {
            out.push(id.clone());
        } else {
            out.push(format!("anon:{anon}"));
            anon += 1;
        }
    }
    out
}

/// Line-disjoint three-way merge of one block. Returns `Some(merged)` when the
/// changed-line index sets are disjoint AND all three sides have equal line
/// counts; `None` (→ Conflict) otherwise.
///
/// T2 decision: the toy fixtures use word substitutions that preserve line
/// count. Differing line counts within a both-changed block are treated as a
/// Conflict — the safe fallback — rather than attempting a full diff3.
fn line_merge(
    base: Option<&String>,
    ours: Option<&String>,
    theirs: Option<&String>,
) -> Option<String> {
    let (base, ours, theirs) = (base?, ours?, theirs?);
    let bl: Vec<&str> = base.lines().collect();
    let ol: Vec<&str> = ours.lines().collect();
    let tl: Vec<&str> = theirs.lines().collect();
    if bl.len() != ol.len() || bl.len() != tl.len() {
        return None;
    }

    let ours_changed: BTreeSet<usize> = (0..bl.len()).filter(|&i| ol[i] != bl[i]).collect();
    let theirs_changed: BTreeSet<usize> = (0..bl.len()).filter(|&i| tl[i] != bl[i]).collect();
    if !ours_changed.is_disjoint(&theirs_changed) {
        return None;
    }

    let merged: Vec<&str> = (0..bl.len())
        .map(|i| {
            if ours_changed.contains(&i) {
                ol[i]
            } else if theirs_changed.contains(&i) {
                tl[i]
            } else {
                bl[i]
            }
        })
        .collect();
    Some(merged.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes_is_disjoint_identity() {
        let base = "alpha\n\nbeta";
        let out = three_way(base, base, base);
        assert_eq!(
            out,
            MergeOutcome::Disjoint {
                merged: base.into()
            }
        );
    }

    #[test]
    fn edits_to_different_blocks_are_disjoint() {
        let base = "one {#a}\n\ntwo {#b}";
        let ours = "ONE {#a}\n\ntwo {#b}";
        let theirs = "one {#a}\n\nTWO {#b}";
        let out = three_way(base, ours, theirs);
        match out {
            MergeOutcome::Disjoint { merged } => {
                assert!(merged.contains("ONE {#a}"));
                assert!(merged.contains("TWO {#b}"));
            }
            other => panic!("expected Disjoint, got {other:?}"),
        }
    }

    #[test]
    fn identical_change_both_sides_is_disjoint() {
        let base = "one {#a}";
        let edit = "ONE {#a}";
        assert_eq!(
            three_way(base, edit, edit),
            MergeOutcome::Disjoint {
                merged: edit.into()
            }
        );
    }

    #[test]
    fn line_disjoint_same_block_is_unique_overlap() {
        // Both edit block #a, but different lines within it → deterministic
        // but NOT safe (R4 §6).
        let base = "line one\nline two {#a}";
        let ours = "LINE ONE\nline two {#a}";
        let theirs = "line one\nLINE TWO {#a}";
        let out = three_way(base, ours, theirs);
        match out {
            MergeOutcome::UniqueOverlap {
                merged,
                overlapping,
            } => {
                assert_eq!(overlapping, vec!["a".to_string()]);
                assert!(merged.contains("LINE ONE"));
                assert!(merged.contains("LINE TWO"));
            }
            other => panic!("expected UniqueOverlap, got {other:?}"),
        }
    }

    #[test]
    fn same_line_incompatible_edits_conflict() {
        let base = "hello world {#a}";
        let ours = "hello there {#a}";
        let theirs = "hello everyone {#a}";
        assert_eq!(three_way(base, ours, theirs), MergeOutcome::Conflict);
    }

    #[test]
    fn deletion_on_one_side_is_honored() {
        let base = "keep {#a}\n\ndrop {#b}";
        let ours = "keep {#a}"; // #b deleted
        let theirs = base; // unchanged
        match three_way(base, ours, theirs) {
            MergeOutcome::Disjoint { merged } => {
                assert!(merged.contains("keep {#a}"));
                assert!(!merged.contains("drop"));
            }
            other => panic!("expected Disjoint, got {other:?}"),
        }
    }

    #[test]
    fn overlap_slots_detects_both_touched_block() {
        // #a changed vs base and the merge kept a value differing from base's
        // → overlap. #b changed only on one side → not an overlap here.
        let base = "one {#a}\n\ntwo {#b}";
        let current = "ONE {#a}\n\ntwo {#b}"; // durable file: #a edited
        let merged = "MERGED-A {#a}\n\nTWO {#b}"; // merge touched #a and #b
        let over = overlap_slots(base, current, merged);
        assert_eq!(over, vec!["a".to_string()]);
    }

    #[test]
    fn overlap_slots_empty_when_disjoint() {
        // current edited #a; merge edited only #b → no slot both-touched.
        let base = "one {#a}\n\ntwo {#b}";
        let current = "ONE {#a}\n\ntwo {#b}";
        let merged = "ONE {#a}\n\nTWO {#b}";
        assert!(overlap_slots(base, current, merged).is_empty());
    }

    #[test]
    fn differing_line_counts_in_shared_block_conflict() {
        let base = "a\nb {#x}";
        let ours = "a\nb\nc {#x}"; // added a line
        let theirs = "A\nb {#x}"; // changed a line
        assert_eq!(three_way(base, ours, theirs), MergeOutcome::Conflict);
    }
}
