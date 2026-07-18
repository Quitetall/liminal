//! The eleven operations (Algorithm B) — exact seeded mutations of the base
//! document. Every random choice derives from `SmallRng::seed_from_u64(seed)`
//! (D07.4), so the same seed twice yields identical bytes.

use liminal_source::paragraph::{self, Block};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::identity::Config;
use crate::identity::gitenv::{self, ConflictPolicy};
use crate::identity::strategy::BaseWorld;

/// Whether an operation is **observed** by the graph Holder (driven as a graph
/// transaction) or **foreign** (bytes changed behind the Holder) — the
/// managed-graph distinction of v4 §19.2 (Algorithm A rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedObservation {
    /// The Holder observed the operation as a graph transaction.
    Observed,
    /// The bytes changed behind the Holder's back.
    Foreign,
}

/// The eleven torture operations (v4 §-1.1), in the config's declared order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// `git mv notes.md docs/renamed.md` (bytes identical).
    RenameMove,
    /// Split a seeded ≥2-sentence block at a sentence boundary; a `{#id}` stays
    /// with the seeded half.
    Split,
    /// Join two seeded adjacent blocks; keep the first's marker, drop the
    /// second's.
    Merge,
    /// Append a byte-exact copy of a seeded block (marker included → duplicate
    /// id).
    CopyPaste,
    /// Append two byte-identical copies of the anonymous block → three identical
    /// anonymous blocks.
    DuplicateIdentical,
    /// Delete a seeded block; append a block with identical text but NO marker
    /// at the original position.
    DeleteRecreate,
    /// Deterministic reformat: greedy re-wrap to width 72, collapse blank runs,
    /// strip trailing whitespace, single trailing newline.
    FormatterRewrite,
    /// `k = seeded 1..=3` blocks each get one seeded mutation.
    ExternalEdit,
    /// `git merge -s ort --no-edit b` over a seeded divergence.
    GitMerge,
    /// `git rebase main` on `b` over a seeded divergence.
    GitRebase,
    /// `git cherry-pick <sha>` onto `main`.
    GitCherryPick,
}

impl Operation {
    /// All operations in declared/render order.
    #[must_use]
    pub fn all() -> [Operation; 11] {
        [
            Operation::RenameMove,
            Operation::Split,
            Operation::Merge,
            Operation::CopyPaste,
            Operation::DuplicateIdentical,
            Operation::DeleteRecreate,
            Operation::FormatterRewrite,
            Operation::ExternalEdit,
            Operation::GitMerge,
            Operation::GitRebase,
            Operation::GitCherryPick,
        ]
    }

    /// The snake_case row name used in config + golden.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Operation::RenameMove => "rename_move",
            Operation::Split => "split",
            Operation::Merge => "merge",
            Operation::CopyPaste => "copy_paste",
            Operation::DuplicateIdentical => "duplicate_identical",
            Operation::DeleteRecreate => "delete_recreate",
            Operation::FormatterRewrite => "formatter_rewrite",
            Operation::ExternalEdit => "external_edit",
            Operation::GitMerge => "git_merge",
            Operation::GitRebase => "git_rebase",
            Operation::GitCherryPick => "git_cherry_pick",
        }
    }

    /// Parse a row name.
    ///
    /// # Errors
    /// Unknown operation name.
    pub fn from_name(s: &str) -> anyhow::Result<Operation> {
        Operation::all()
            .into_iter()
            .find(|op| op.name() == s)
            .ok_or_else(|| anyhow::anyhow!("unknown operation: {s:?}"))
    }

    /// Observed vs foreign classification (Algorithm A rule 6). Observed:
    /// rename_move, split, merge, copy_paste, duplicate_identical,
    /// delete_recreate. Foreign: formatter_rewrite, external_edit, all git ops.
    #[must_use]
    pub fn managed_observation(self) -> ManagedObservation {
        match self {
            Operation::RenameMove
            | Operation::Split
            | Operation::Merge
            | Operation::CopyPaste
            | Operation::DuplicateIdentical
            | Operation::DeleteRecreate => ManagedObservation::Observed,
            Operation::FormatterRewrite
            | Operation::ExternalEdit
            | Operation::GitMerge
            | Operation::GitRebase
            | Operation::GitCherryPick => ManagedObservation::Foreign,
        }
    }

    /// Whether this operation drives the real `git` CLI (D07.6).
    #[must_use]
    pub fn is_git(self) -> bool {
        matches!(
            self,
            Operation::GitMerge | Operation::GitRebase | Operation::GitCherryPick
        )
    }
}

/// The post-operation world observed by the strategies.
#[derive(Debug, Clone)]
pub struct PostOp {
    /// The post-operation file bytes.
    pub file: String,
    /// The post-operation file, parsed into blocks (M03 grammar).
    pub blocks: Vec<Block>,
    /// The base sidecar entries (snapshot at base — NEVER updated by the op).
    pub sidecar: Vec<crate::identity::strategy::SidecarEntry>,
    /// rename_move only: did the sidecar move with the file (seeded)? For every
    /// other op, `true` (the sidecar sits beside the unchanged path).
    pub sidecar_moved: bool,
    /// The observed/foreign classification of the operation that produced this.
    pub managed: ManagedObservation,
    /// Git conflict policy applied (git ops only).
    pub policy: Option<ConflictPolicy>,
    /// The base world (retained so managed-graph re-ingest / structural fallback
    /// can reach base block state).
    pub base: BaseWorld,
}

/// The raw byte output of one operation, before it is parsed into a [`PostOp`].
#[derive(Debug, Clone)]
pub struct OpOutput {
    /// The post-operation file bytes.
    pub file: String,
    /// rename_move: whether the sidecar moved with the file. Default `true`.
    pub sidecar_moved: bool,
    /// Git conflict policy applied, if any.
    pub policy: Option<ConflictPolicy>,
}

impl OpOutput {
    /// A plain text-only output (sidecar co-located, no git policy).
    #[must_use]
    pub fn text(file: String) -> OpOutput {
        OpOutput {
            file,
            sidecar_moved: true,
            policy: None,
        }
    }
}

/// Apply `op` to `world` under `seed`, producing the observed post-op world.
/// Deterministic in `seed` (D07.4). Git ops run through [`gitenv`] in a hermetic
/// temp repo keyed by `(op, seed)`.
///
/// # Errors
/// Propagates git-runbook failures (a below-floor `git`, a failed command).
pub fn apply(op: Operation, world: &BaseWorld, seed: u64, cfg: &Config) -> anyhow::Result<PostOp> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let out = if op.is_git() {
        gitenv::run_git_op(op, world, seed, &mut rng, cfg)?
    } else {
        match op {
            Operation::RenameMove => op_rename_move(world, &mut rng),
            Operation::Split => op_split(world, &mut rng),
            Operation::Merge => op_merge(world, &mut rng),
            Operation::CopyPaste => op_copy_paste(world, &mut rng),
            Operation::DuplicateIdentical => op_duplicate_identical(world, &mut rng),
            Operation::DeleteRecreate => op_delete_recreate(world, &mut rng),
            Operation::FormatterRewrite => op_formatter_rewrite(world),
            Operation::ExternalEdit => op_external_edit(world, &mut rng, seed),
            Operation::GitMerge | Operation::GitRebase | Operation::GitCherryPick => {
                unreachable!("git ops handled above")
            }
        }
    };
    Ok(PostOp {
        blocks: paragraph::parse(&out.file),
        file: out.file,
        sidecar: world.sidecar(),
        sidecar_moved: out.sidecar_moved,
        managed: op.managed_observation(),
        policy: out.policy,
        base: world.clone(),
    })
}

// ── Algorithm B mutations ───────────────────────────────────────────────────

/// A working document as an ordered list of `(text, id)` blocks — the mutable
/// form the non-git operations edit. Re-rendering is the inverse of the M03
/// grammar: block sources joined by a blank line, one trailing newline.
#[derive(Debug, Clone)]
struct Doc {
    blocks: Vec<(String, Option<String>)>,
}

impl Doc {
    fn from_world(world: &BaseWorld) -> Doc {
        Doc {
            blocks: world
                .blocks
                .iter()
                .map(|b| (b.text.clone(), b.id.clone()))
                .collect(),
        }
    }

    fn render(&self) -> String {
        let body = self
            .blocks
            .iter()
            .map(|(t, id)| block_source(t, id.as_deref()))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("{body}\n")
    }
}

/// The Holder-controlled source of one block: its text with the `{#id}` marker
/// re-appended to the last line (the inverse of the M03 parser).
fn block_source(text: &str, id: Option<&str>) -> String {
    match id {
        Some(id) => format!("{text} {{#{id}}}"),
        None => text.to_owned(),
    }
}

/// Interior split boundaries of a block's text (D07.3 resolution, DG-7.3): the
/// byte position after each interior `". "` sentence break, unioned with the
/// position after each interior `"\n"` line break. Strictly inside the text.
fn interior_boundaries(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut bounds = std::collections::BTreeSet::new();
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            let p = i + 1;
            if p > 0 && p < text.len() {
                bounds.insert(p);
            }
        }
        if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            let p = i + 2;
            if p > 0 && p < text.len() {
                bounds.insert(p);
            }
        }
    }
    bounds.into_iter().collect()
}

/// B1 `rename_move` — bytes identical; only the path (and maybe the sidecar)
/// moves. Seeds a bool: does the sidecar move with the file? Both worlds are
/// exercised. Returns the base file unchanged.
fn op_rename_move(world: &BaseWorld, rng: &mut SmallRng) -> OpOutput {
    let sidecar_moved = rng.random_bool(0.5);
    OpOutput {
        file: world.file.clone(),
        sidecar_moved,
        policy: None,
    }
}

/// B2 `split` (DG-7.3) — pick a seeded block with an interior boundary; insert a
/// blank line at a seeded boundary; the trailing `{#id}` marker rides with the
/// SECOND (later) half. If no block has an interior boundary the op is the
/// identity.
fn op_split(world: &BaseWorld, rng: &mut SmallRng) -> OpOutput {
    let mut doc = Doc::from_world(world);
    let candidates: Vec<usize> = doc
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| !interior_boundaries(t).is_empty())
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return OpOutput::text(doc.render());
    }
    let bi = candidates[rng.random_range(0..candidates.len())];
    let (text, id) = doc.blocks[bi].clone();
    let bounds = interior_boundaries(&text);
    let p = bounds[rng.random_range(0..bounds.len())];
    let head = text[..p].trim_end().to_owned();
    let tail = text[p..].trim_start().to_owned();
    doc.blocks.splice(bi..=bi, [(head, None), (tail, id)]);
    OpOutput::text(doc.render())
}

/// B3 `merge` — join two seeded adjacent blocks with one space; keep the first's
/// marker, drop the second's.
fn op_merge(world: &BaseWorld, rng: &mut SmallRng) -> OpOutput {
    let mut doc = Doc::from_world(world);
    if doc.blocks.len() < 2 {
        return OpOutput::text(doc.render());
    }
    let i = rng.random_range(0..doc.blocks.len() - 1);
    let (t0, id0) = doc.blocks[i].clone();
    let (t1, _id1) = doc.blocks[i + 1].clone();
    let merged = format!("{t0} {t1}");
    doc.blocks.splice(i..=i + 1, [(merged, id0)]);
    OpOutput::text(doc.render())
}

/// B4 `copy_paste` — append a byte-exact copy of a seeded block (marker included
/// → a duplicate id when the block carries one).
fn op_copy_paste(world: &BaseWorld, rng: &mut SmallRng) -> OpOutput {
    let mut doc = Doc::from_world(world);
    let i = rng.random_range(0..doc.blocks.len());
    let copy = doc.blocks[i].clone();
    doc.blocks.push(copy);
    OpOutput::text(doc.render())
}

/// B5 `duplicate_identical` — append TWO byte-identical copies of the anonymous
/// block → three identical anonymous blocks.
fn op_duplicate_identical(world: &BaseWorld, _rng: &mut SmallRng) -> OpOutput {
    let mut doc = Doc::from_world(world);
    let Some(ai) = doc.blocks.iter().position(|(_, id)| id.is_none()) else {
        return OpOutput::text(doc.render());
    };
    let copy = doc.blocks[ai].clone();
    doc.blocks.push(copy.clone());
    doc.blocks.push(copy);
    OpOutput::text(doc.render())
}

/// B6 `delete_recreate` — delete a seeded block and recreate it at the same
/// position with identical text but NO marker (DG-7.1: an id lost with no
/// duplicate, information-theoretically unsurfaceable — v4 §19.2).
fn op_delete_recreate(world: &BaseWorld, rng: &mut SmallRng) -> OpOutput {
    let mut doc = Doc::from_world(world);
    let i = rng.random_range(0..doc.blocks.len());
    let (text, _id) = doc.blocks[i].clone();
    doc.blocks[i] = (text, None);
    OpOutput::text(doc.render())
}

/// B7 `formatter_rewrite` — deterministic reformat: per block, collapse all
/// interior whitespace and greedily re-wrap the block source (marker included,
/// so it stays a trailing word) to width 72; strip trailing whitespace; join
/// blocks with one blank line; single trailing newline.
fn op_formatter_rewrite(world: &BaseWorld) -> OpOutput {
    let doc = Doc::from_world(world);
    let body = doc
        .blocks
        .iter()
        .map(|(t, id)| wrap72(&block_source(t, id.as_deref())))
        .collect::<Vec<_>>()
        .join("\n\n");
    OpOutput::text(format!("{body}\n"))
}

/// Greedy word-wrap to width 72 over whitespace-collapsed input.
fn wrap72(s: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if cur.is_empty() {
            w.clone_into(&mut cur);
        } else if cur.chars().count() + 1 + w.chars().count() <= 72 {
            cur.push(' ');
            cur.push_str(w);
        } else {
            lines.push(std::mem::take(&mut cur));
            w.clone_into(&mut cur);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.join("\n")
}

/// B8 `external_edit` — `k = seeded 1..=3` distinct blocks; each gets one seeded
/// mutation, applied to the block SOURCE (marker included, so a deletion can
/// genuinely drop the `{#id}`): {replace the first word with `EDITED<seed>`,
/// delete the last sentence, prepend `Inserted by seed <seed>. `}.
fn op_external_edit(world: &BaseWorld, rng: &mut SmallRng, seed: u64) -> OpOutput {
    let doc = Doc::from_world(world);
    let mut sources: Vec<String> = doc
        .blocks
        .iter()
        .map(|(t, id)| block_source(t, id.as_deref()))
        .collect();
    let n = sources.len();
    let k = rng.random_range(1..=3.min(n).max(1));
    let mut pool: Vec<usize> = (0..n).collect();
    for _ in 0..k {
        if pool.is_empty() {
            break;
        }
        let j = rng.random_range(0..pool.len());
        let bi = pool.remove(j);
        let mutation = rng.random_range(0..3u8);
        sources[bi] = apply_external_mutation(&sources[bi], mutation, seed);
    }
    OpOutput::text(format!("{}\n", sources.join("\n\n")))
}

/// Rebuild the file with block `block_idx`'s FIRST word replaced by `tag` — the
/// deterministic per-branch divergence edit for the git operations (DG-7.4).
/// Out-of-range `block_idx` returns the file unchanged.
#[must_use]
pub fn edit_block_first_word(world: &BaseWorld, block_idx: usize, tag: &str) -> String {
    let mut doc = Doc::from_world(world);
    if let Some((text, _id)) = doc.blocks.get_mut(block_idx) {
        let mut it = text.splitn(2, char::is_whitespace);
        let _first = it.next().unwrap_or("");
        let rest = it.next().unwrap_or("");
        *text = if rest.is_empty() {
            tag.to_owned()
        } else {
            format!("{tag} {rest}")
        };
    }
    doc.render()
}

/// Resolve a conflicted file the **union** way (D07 §C): drop the git conflict
/// scaffolding lines (`<<<<<<<`, `=======`, `>>>>>>>`) and keep both sides'
/// content in order.
#[must_use]
pub fn resolve_union(conflicted: &str) -> String {
    let kept: Vec<&str> = conflicted
        .lines()
        .filter(|l| {
            !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>")
        })
        .collect();
    let mut out = kept.join("\n");
    if conflicted.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// One seeded external mutation over a block source.
fn apply_external_mutation(src: &str, mutation: u8, seed: u64) -> String {
    match mutation {
        // Replace the first whitespace-delimited word with `EDITED<seed>`.
        0 => {
            let mut it = src.splitn(2, ' ');
            let _first = it.next().unwrap_or("");
            let rest = it.next().unwrap_or("");
            if rest.is_empty() {
                format!("EDITED{seed}")
            } else {
                format!("EDITED{seed} {rest}")
            }
        }
        // Delete the last sentence: drop everything after the last interior
        // `". "` (which, for a marked block, is the `{#id}` trailer); else drop
        // the last physical line.
        1 => {
            if let Some(pos) = src.rfind(". ") {
                src[..=pos].to_owned()
            } else if let Some(nl) = src.rfind('\n') {
                src[..nl].to_owned()
            } else {
                src.to_owned()
            }
        }
        // Prepend a marker sentence.
        _ => format!("Inserted by seed {seed}. {src}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Config;
    use crate::identity::strategy::BaseWorld;

    /// M07.4: every operation is deterministic — the same seed twice yields
    /// byte-identical output (D07.4). Covers the git ops too (hermetic env).
    #[test]
    fn same_seed_yields_identical_bytes() {
        let cfg = Config::load().expect("load config");
        let world = BaseWorld::build(&cfg.template);
        for op in Operation::all() {
            let a = apply(op, &world, 33, &cfg).expect("apply a");
            let b = apply(op, &world, 33, &cfg).expect("apply b");
            assert_eq!(
                a.file,
                b.file,
                "{} is not deterministic in the seed",
                op.name()
            );
        }
    }

    /// Distinct seeds may diverge, but the corpus still parses cleanly for every
    /// (op, seed) — the post-op file is always valid toy source (Law 3B).
    #[test]
    fn every_case_reparses() {
        let cfg = Config::load().expect("load config");
        let world = BaseWorld::build(&cfg.template);
        for op in Operation::all() {
            for &seed in &cfg.seeds {
                let post = apply(op, &world, seed, &cfg).expect("apply");
                // Parsing is total; just confirm it does not panic and the
                // re-parsed block list matches what the PostOp captured.
                assert_eq!(post.blocks, paragraph::parse(&post.file));
            }
        }
    }

    #[test]
    fn managed_observation_table() {
        assert_eq!(
            Operation::Merge.managed_observation(),
            ManagedObservation::Observed
        );
        assert_eq!(
            Operation::FormatterRewrite.managed_observation(),
            ManagedObservation::Foreign
        );
        assert_eq!(
            Operation::GitMerge.managed_observation(),
            ManagedObservation::Foreign
        );
    }

    #[test]
    fn resolve_union_strips_conflict_scaffolding() {
        let conflicted = "a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> other\nd\n";
        assert_eq!(resolve_union(conflicted), "a\nb\nc\nd\n");
    }
}
