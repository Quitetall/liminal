//! Identity strategies, outcomes, and the six deterministic observers
//! (Algorithm A). An observer maps (base state, post-operation world) → one
//! [`Outcome`] for one tracked block. A strategy never grades a heuristic match
//! above [`IdentityGrade::Inferred`] (v4 §19.2, Law 12).

use std::collections::BTreeSet;

use liminal_id::IdentityGrade;
use liminal_source::paragraph::{self, Block};

use crate::identity::ops::PostOp;
use crate::identity::{Config, content_hash_hex, normalize};

/// The demonstrated identity outcome for one tracked block after one operation
/// (v4 §-1.1). This is the unit the guarantee matrix aggregates.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Identity survived at a demonstrated grade (never above what the observer
    /// can prove — heuristics cap at [`IdentityGrade::Inferred`]).
    Preserved(IdentityGrade),
    /// Recovered heuristically with the given confidence (0.0–1.0).
    Recovered {
        /// Recovery confidence, rendered to two decimals in the matrix.
        confidence: f32,
    },
    /// Two or more candidates tie — identity is ambiguous.
    Ambiguous,
    /// No candidate — identity was lost.
    Lost,
}

impl Outcome {
    /// The declared-claim ceiling this outcome permits (Algorithm E `cap`):
    /// `Preserved(g) → g`; `Recovered(_) → Inferred`; `Ambiguous | Lost →
    /// Anchored`. This is the bound a profile's declared `required_identity`
    /// may never exceed.
    #[must_use]
    pub fn cap(&self) -> IdentityGrade {
        match self {
            Outcome::Preserved(g) => *g,
            Outcome::Recovered { .. } => IdentityGrade::Inferred,
            Outcome::Ambiguous | Outcome::Lost => IdentityGrade::Anchored,
        }
    }

    /// Render for a matrix cell: `Preserved(<kebab grade>)` | `Recovered(0.62)`
    /// (two decimals) | `Ambiguous` | `Lost` (D07, §D).
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Outcome::Preserved(g) => format!("Preserved({})", grade_kebab(*g)),
            Outcome::Recovered { confidence } => format!("Recovered({confidence:.2})"),
            Outcome::Ambiguous => "Ambiguous".to_owned(),
            Outcome::Lost => "Lost".to_owned(),
        }
    }

    /// The worst-outcome total-ordering key (D07.5): `Lost < Ambiguous <
    /// Recovered(c)` (lower `c` worse) `< Preserved(g)` (weaker grade worse).
    /// Smaller key = worse. The case/cell outcome is the MINIMUM over its parts.
    fn worst_key(&self) -> (u8, f32, u8) {
        match self {
            Outcome::Lost => (0, 0.0, 0),
            Outcome::Ambiguous => (1, 0.0, 0),
            Outcome::Recovered { confidence } => (2, *confidence, 0),
            Outcome::Preserved(g) => (3, 0.0, g.strength()),
        }
    }
}

impl Eq for Outcome {}

impl Ord for Outcome {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (a0, a1, a2) = self.worst_key();
        let (b0, b1, b2) = other.worst_key();
        a0.cmp(&b0)
            .then_with(|| a1.total_cmp(&b1))
            .then_with(|| a2.cmp(&b2))
    }
}

impl PartialOrd for Outcome {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Fold a set of per-block outcomes to the worst (D07.5). Empty → `Lost` (no
/// tracked block survived to report anything).
#[must_use]
pub fn worst(outcomes: impl IntoIterator<Item = Outcome>) -> Outcome {
    outcomes.into_iter().min().unwrap_or(Outcome::Lost)
}

/// The kebab spelling of an identity grade (matches the serde `rename_all`).
fn grade_kebab(g: IdentityGrade) -> &'static str {
    match g {
        IdentityGrade::Ephemeral => "ephemeral",
        IdentityGrade::Anchored => "anchored",
        IdentityGrade::Inferred => "inferred",
        IdentityGrade::Explicit => "explicit",
        IdentityGrade::Managed => "managed",
        IdentityGrade::External => "external",
        IdentityGrade::ContentAddressed => "content-addressed",
    }
}

/// The six identity strategies (v4 §-1.1), in the config's declared order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// `{#id}` markers in the Holder-controlled source (strongest portable).
    InlineId,
    /// A `<file>.ids.json` sidecar, written at base and never updated.
    Sidecar,
    /// blake3 of `normalize(text)` — version identity, not entity identity.
    ContentHash,
    /// Normalized equality then 3-gram Jaccard (D07.3), a heuristic.
    Structural,
    /// A byte anchor with bounded context (D07 revision-anchor), a heuristic.
    RevisionAnchor,
    /// The file mirrored into the M03 graph; identity is Holder-managed.
    ManagedGraph,
}

impl Strategy {
    /// All strategies in declared/render order.
    #[must_use]
    pub fn all() -> [Strategy; 6] {
        [
            Strategy::InlineId,
            Strategy::Sidecar,
            Strategy::ContentHash,
            Strategy::Structural,
            Strategy::RevisionAnchor,
            Strategy::ManagedGraph,
        ]
    }

    /// The kebab column header used in config + golden.
    #[must_use]
    pub fn kebab(self) -> &'static str {
        match self {
            Strategy::InlineId => "inline-id",
            Strategy::Sidecar => "sidecar",
            Strategy::ContentHash => "content-hash",
            Strategy::Structural => "structural",
            Strategy::RevisionAnchor => "revision-anchor",
            Strategy::ManagedGraph => "managed-graph",
        }
    }

    /// Parse a kebab column header.
    ///
    /// # Errors
    /// Unknown strategy name.
    pub fn from_kebab(s: &str) -> anyhow::Result<Strategy> {
        Strategy::all()
            .into_iter()
            .find(|st| st.kebab() == s)
            .ok_or_else(|| anyhow::anyhow!("unknown strategy: {s:?}"))
    }
}

/// A byte anchor with bounded context (D07 revision-anchor schema). Exported for
/// M09 reuse. Contexts are truncated at file bounds and capped at 16 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// Start offset of the anchored bytes in the base file.
    pub byte_offset: u64,
    /// Length of the anchored bytes.
    pub len: u64,
    /// Up to 16 bytes immediately before the anchored span.
    pub ctx_before: Vec<u8>,
    /// Up to 16 bytes immediately after the anchored span.
    pub ctx_after: Vec<u8>,
}

impl Anchor {
    /// Capture an anchor over `bytes[offset..offset+len]` with ≤16 bytes of
    /// context on each side, truncated at file bounds.
    #[must_use]
    pub fn capture(bytes: &[u8], offset: usize, len: usize) -> Anchor {
        let end = (offset + len).min(bytes.len());
        let before_start = offset.saturating_sub(16);
        let after_end = (end + 16).min(bytes.len());
        Anchor {
            byte_offset: offset as u64,
            len: (end - offset) as u64,
            ctx_before: bytes[before_start..offset].to_vec(),
            ctx_after: bytes[end..after_end].to_vec(),
        }
    }

    /// The anchored bytes as they were at base capture.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "byte offsets into an in-memory test document"
    )]
    pub fn anchored<'a>(&self, base: &'a [u8]) -> &'a [u8] {
        let start = self.byte_offset as usize;
        let end = (start + self.len as usize).min(base.len());
        &base[start.min(base.len())..end]
    }
}

/// One base block plus the per-strategy state captured at base (before the
/// operation runs). `index` is its position in the base document.
#[derive(Debug, Clone)]
pub struct BaseBlock {
    /// Position in the base document (0-based).
    pub index: usize,
    /// Block text (marker stripped, per the M03 grammar).
    pub text: String,
    /// The inline `{#id}`, if the block carries one.
    pub id: Option<String>,
    /// blake3 hex of `normalize(text)` (sidecar/content-hash key).
    pub content_hash: String,
    /// The revision anchor over this block's bytes in the base file.
    pub anchor: Anchor,
}

/// The base world: the template file plus every base block's captured state.
#[derive(Debug, Clone)]
pub struct BaseWorld {
    /// The base file bytes (the template verbatim).
    pub file: String,
    /// One [`BaseBlock`] per parsed block.
    pub blocks: Vec<BaseBlock>,
}

impl BaseWorld {
    /// Build the base world from the template text.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "byte offsets into an in-memory test document"
    )]
    pub fn build(template: &str) -> BaseWorld {
        let parsed = paragraph::parse(template);
        let bytes = template.as_bytes();
        let blocks = parsed
            .iter()
            .enumerate()
            .map(|(index, b)| BaseBlock {
                index,
                text: b.text.clone(),
                id: b.id.clone(),
                content_hash: content_hash_hex(&b.text),
                anchor: Anchor::capture(
                    bytes,
                    b.range.start as usize,
                    (b.range.end - b.range.start) as usize,
                ),
            })
            .collect();
        BaseWorld {
            file: template.to_owned(),
            blocks,
        }
    }

    /// The base sidecar (`<file>.ids.json` contents): one entry per block with
    /// its `index`, `id`, and `content_hash` — written at base, NEVER updated by
    /// an operation (that is the point of the sidecar test).
    #[must_use]
    pub fn sidecar(&self) -> Vec<SidecarEntry> {
        self.blocks
            .iter()
            .map(|b| SidecarEntry {
                index: b.index,
                id: b.id.clone().unwrap_or_default(),
                hash: b.content_hash.clone(),
            })
            .collect()
    }
}

/// One `<file>.ids.json` entry (schema: `{index, id, hash}`).
#[derive(Debug, Clone)]
pub struct SidecarEntry {
    /// The block index recorded at base.
    pub index: usize,
    /// The block id recorded at base (empty string if anonymous).
    pub id: String,
    /// blake3 hex of `normalize(text)` recorded at base.
    pub hash: String,
}

/// Which base blocks a strategy can track (Algorithm D "tracked blocks"):
/// `inline-id` tracks only id-bearing blocks (it cannot identify anonymous
/// text); every other strategy tracks all blocks.
#[must_use]
pub fn tracked_blocks(strategy: Strategy, world: &BaseWorld) -> Vec<usize> {
    world
        .blocks
        .iter()
        .filter(|b| match strategy {
            Strategy::InlineId => b.id.is_some(),
            _ => true,
        })
        .map(|b| b.index)
        .collect()
}

/// Observe one tracked block's outcome under one strategy after one operation
/// (Algorithm A). `tracked` is the base state of the block being followed;
/// `post` is the post-operation world.
///
/// The six arms implement, verbatim, Algorithm A rules 1–6:
///
/// 1. **inline-id** — re-parse `post.file`: the tracked block's id present
///    exactly once → `Preserved(Explicit)`; ≥2 → `Ambiguous`; absent → `Lost`.
/// 2. **sidecar** — using the base sidecar entry for this block: a unique
///    post-op block with an EQUAL `content_hash` at the SAME index →
///    `Preserved(Explicit)`; a unique equal hash at a DIFFERENT index →
///    `Recovered(0.9)`; an equal hash at ≥2 blocks → `Ambiguous`; no hash match
///    but the post-op block at the recorded index has Jaccard ≥ threshold →
///    `Recovered(0.5)`; else `Lost`. (`post.sidecar_moved` only affects the
///    file the sidecar is read alongside — the sidecar CONTENT is the base
///    snapshot regardless; both worlds are exercised by seeding the bool.)
/// 3. **content-hash** — blake3 of `normalize(text)`: the tracked block's base
///    `content_hash` equal in exactly one post-op block → `Preserved(
///    ContentAddressed)`; ≥2 → `Ambiguous`; zero → `Lost`.
/// 4. **structural** — D07.3: exact `normalize` equality against a post-op
///    block first; else 3-gram character-set Jaccard; unique argmax ≥ 0.5 →
///    `Recovered(score)`; tie (|Δ| ≤ 1e-9) → `Ambiguous`; best < 0.5 → `Lost`.
///    A heuristic never grades above `Inferred` (exact normalize equality →
///    `Preserved(Inferred)`, NOT Explicit — structural is a heuristic).
/// 5. **revision-anchor** — (1) bytes at `offset..offset+len` unchanged AND both
///    contexts match at that offset → `Preserved(Anchored)`; (2) exactly one
///    occurrence of `ctx_before ∥ anchored ∥ ctx_after` anywhere →
///    `Recovered(0.9)`; (3) exactly one occurrence of the anchored bytes alone →
///    `Recovered(0.6)`; ≥2 occurrences at step 2 or 3 → `Ambiguous`; none →
///    `Lost`.
/// 6. **managed-graph** — `post.managed` says whether the op was **observed**
///    (driven as a graph transaction that preserves the `EntityId`) or
///    **foreign** (bytes changed behind the Holder). Observed → the tracked
///    entity still resolves in the mirror → `Preserved(Managed)`. Foreign →
///    re-ingest and fall back to rule 4 (structural), honestly demonstrating
///    v4 §19.2 (Managed holds only while the Holder observes the operation).
#[must_use]
pub fn observe(strategy: Strategy, tracked: &BaseBlock, post: &PostOp, cfg: &Config) -> Outcome {
    match strategy {
        Strategy::InlineId => observe_inline_id(tracked, post),
        Strategy::Sidecar => observe_sidecar(tracked, post, cfg),
        Strategy::ContentHash => observe_content_hash(tracked, post),
        Strategy::Structural => observe_structural(tracked, post, cfg),
        Strategy::RevisionAnchor => observe_revision_anchor(tracked, post),
        Strategy::ManagedGraph => observe_managed_graph(tracked, post, cfg),
    }
}

/// 3-gram character-set Jaccard over two normalized strings (D07.3). Text with
/// fewer than `ngram` chars contributes a single gram (the whole string).
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "n-gram set cardinalities are tiny in the toy corpus"
)]
pub fn jaccard(a: &str, b: &str, ngram: usize) -> f64 {
    let ga = char_ngrams(a, ngram);
    let gb = char_ngrams(b, ngram);
    if ga.is_empty() && gb.is_empty() {
        return 1.0;
    }
    let inter = ga.intersection(&gb).count() as f64;
    let union = ga.union(&gb).count() as f64;
    if union == 0.0 { 0.0 } else { inter / union }
}

/// The set of character n-grams of `s` (over `char`s, not bytes). `< ngram`
/// chars → a single gram holding the whole string.
fn char_ngrams(s: &str, ngram: usize) -> BTreeSet<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < ngram {
        let mut set = BTreeSet::new();
        if !chars.is_empty() {
            set.insert(chars.iter().collect());
        }
        return set;
    }
    chars
        .windows(ngram)
        .map(|w| w.iter().collect::<String>())
        .collect()
}

// ── Algorithm A observers ──────────────────────────────────────────────────

/// Rule 1 (inline-id): the tracked id present exactly once → `Preserved(
/// Explicit)`; ≥2 → `Ambiguous`; absent → `Lost`.
fn observe_inline_id(tracked: &BaseBlock, post: &PostOp) -> Outcome {
    let Some(id) = &tracked.id else {
        // inline-id only tracks id-bearing blocks (tracked_blocks guarantees
        // this); a defensive Lost keeps the fn total.
        return Outcome::Lost;
    };
    match count_id_occurrences(&post.blocks, id) {
        0 => Outcome::Lost,
        1 => Outcome::Preserved(IdentityGrade::Explicit),
        _ => Outcome::Ambiguous,
    }
}

/// Rule 3 (content-hash): the tracked block's base `normalize`-hash equal in
/// exactly one post-op block → `Preserved(ContentAddressed)`; ≥2 → `Ambiguous`;
/// zero → `Lost`. Version identity, not entity identity.
fn observe_content_hash(tracked: &BaseBlock, post: &PostOp) -> Outcome {
    match count_hash_matches(&post.blocks, &tracked.content_hash) {
        0 => Outcome::Lost,
        1 => Outcome::Preserved(IdentityGrade::ContentAddressed),
        _ => Outcome::Ambiguous,
    }
}

/// Rule 2 (sidecar). The sidecar CONTENT is the base snapshot (never updated).
/// When the sidecar did not travel with a `rename_move` (`!sidecar_moved`), it
/// is orphaned from the file and resolves nothing → `Lost`. Otherwise: a unique
/// post-op block with an EQUAL base hash at the SAME index → `Preserved(
/// Explicit)`; a unique equal hash at a DIFFERENT index → `Recovered(0.9)`; an
/// equal hash at ≥2 blocks → `Ambiguous`; no hash match but the post-op block at
/// the recorded index has Jaccard ≥ threshold → `Recovered(0.5)`; else `Lost`.
fn observe_sidecar(tracked: &BaseBlock, post: &PostOp, cfg: &Config) -> Outcome {
    if !post.sidecar_moved {
        return Outcome::Lost;
    }
    // The base sidecar entry for this block is (tracked.index, tracked.content_hash).
    let matches: Vec<usize> = post
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| content_hash_hex(&b.text) == tracked.content_hash)
        .map(|(i, _)| i)
        .collect();
    match matches.as_slice() {
        [only] if *only == tracked.index => Outcome::Preserved(IdentityGrade::Explicit),
        [_only] => Outcome::Recovered { confidence: 0.9 },
        [] => {
            // No hash match: fall back to the block at the recorded index.
            match post.blocks.get(tracked.index) {
                Some(b)
                    if jaccard(&normalize(&tracked.text), &normalize(&b.text), cfg.ngram)
                        >= cfg.jaccard_threshold =>
                {
                    Outcome::Recovered { confidence: 0.5 }
                }
                _ => Outcome::Lost,
            }
        }
        _ => Outcome::Ambiguous,
    }
}

/// Rule 4 (structural, D07.3). Exact `normalize` equality first: a unique
/// normalize-equal block → `Recovered(1.0)`; ≥2 → `Ambiguous`. Else 3-gram
/// character-set Jaccard argmax over all post-op blocks: `best < 0.5` → `Lost`;
/// a tie at the max (|Δ| ≤ 1e-9) → `Ambiguous`; a unique argmax ≥ 0.5 →
/// `Recovered(best)`. Structural is a heuristic, so its outcomes are confined to
/// `{Recovered, Ambiguous, Lost}` — never `Preserved` (v4 §19.2; caps at
/// `Inferred`).
#[allow(
    clippy::cast_possible_truncation,
    reason = "Jaccard scores are 0.0..=1.0; f32 confidence is the D07 schema"
)]
fn observe_structural(tracked: &BaseBlock, post: &PostOp, cfg: &Config) -> Outcome {
    let base_norm = normalize(&tracked.text);
    let exact = post
        .blocks
        .iter()
        .filter(|b| normalize(&b.text) == base_norm)
        .count();
    match exact {
        1 => return Outcome::Recovered { confidence: 1.0 },
        n if n >= 2 => return Outcome::Ambiguous,
        _ => {}
    }
    let scores: Vec<f64> = post
        .blocks
        .iter()
        .map(|b| jaccard(&base_norm, &normalize(&b.text), cfg.ngram))
        .collect();
    let best = scores.iter().copied().fold(f64::MIN, f64::max);
    if best < 0.5 {
        return Outcome::Lost;
    }
    let winners = scores.iter().filter(|s| (**s - best).abs() <= 1e-9).count();
    if winners >= 2 {
        Outcome::Ambiguous
    } else {
        Outcome::Recovered {
            confidence: best as f32,
        }
    }
}

/// Rule 5 (revision-anchor). (1) The anchored bytes at the recorded offset are
/// unchanged AND both contexts match at that offset → `Preserved(Anchored)`;
/// (2) exactly one occurrence of `ctx_before ∥ anchored ∥ ctx_after` anywhere →
/// `Recovered(0.9)`; (3) exactly one occurrence of the anchored bytes alone →
/// `Recovered(0.6)`; ≥2 occurrences at step 2 or 3 → `Ambiguous`; none → `Lost`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "byte offsets into an in-memory test document"
)]
fn observe_revision_anchor(tracked: &BaseBlock, post: &PostOp) -> Outcome {
    let base = post.base.file.as_bytes();
    let anchored = tracked.anchor.anchored(base).to_vec();
    let post_bytes = post.file.as_bytes();
    let off = tracked.anchor.byte_offset as usize;
    let len = tracked.anchor.len as usize;
    let cb = &tracked.anchor.ctx_before;
    let ca = &tracked.anchor.ctx_after;

    // Step 1: exact span + both contexts at the recorded offset.
    if off + len <= post_bytes.len()
        && post_bytes[off..off + len] == anchored[..]
        && off >= cb.len()
        && post_bytes[off - cb.len()..off] == cb[..]
        && off + len + ca.len() <= post_bytes.len()
        && post_bytes[off + len..off + len + ca.len()] == ca[..]
    {
        return Outcome::Preserved(IdentityGrade::Anchored);
    }

    // Step 2: ctx_before ∥ anchored ∥ ctx_after occurrences anywhere.
    let mut with_ctx = Vec::with_capacity(cb.len() + anchored.len() + ca.len());
    with_ctx.extend_from_slice(cb);
    with_ctx.extend_from_slice(&anchored);
    with_ctx.extend_from_slice(ca);
    match count_occurrences(post_bytes, &with_ctx) {
        1 => return Outcome::Recovered { confidence: 0.9 },
        n if n >= 2 => return Outcome::Ambiguous,
        _ => {}
    }

    // Step 3: anchored bytes alone.
    match count_occurrences(post_bytes, &anchored) {
        1 => Outcome::Recovered { confidence: 0.6 },
        0 => Outcome::Lost,
        _ => Outcome::Ambiguous,
    }
}

/// Rule 6 (managed-graph). When the Holder **observed** the operation (it was a
/// graph transaction), the tracked entity retains its `EntityId` by construction
/// (v4 §7.6) → `Preserved(Managed)`. When the operation was **foreign** (bytes
/// changed behind the Holder), managed identity does not hold; the file is
/// re-ingested and identity falls back to the structural heuristic (rule 4),
/// honestly demonstrating v4 §19.2.
fn observe_managed_graph(tracked: &BaseBlock, post: &PostOp, cfg: &Config) -> Outcome {
    match post.managed {
        crate::identity::ops::ManagedObservation::Observed => {
            Outcome::Preserved(IdentityGrade::Managed)
        }
        crate::identity::ops::ManagedObservation::Foreign => observe_structural(tracked, post, cfg),
    }
}

/// Count non-overlapping occurrences of `needle` in `haystack` (byte search).
/// An empty needle counts as zero (no meaningful anchor).
#[must_use]
pub fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || needle.len() > haystack.len() {
        return 0;
    }
    let mut count = 0;
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            count += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    count
}

/// Count the post-op blocks that carry inline id `id`.
#[must_use]
pub fn count_id_occurrences(blocks: &[Block], id: &str) -> usize {
    blocks
        .iter()
        .filter(|b| b.id.as_deref() == Some(id))
        .count()
}

/// Count the post-op blocks whose `normalize(text)` hash equals `hash`.
#[must_use]
pub fn count_hash_matches(blocks: &[Block], hash: &str) -> usize {
    blocks
        .iter()
        .filter(|b| content_hash_hex(&b.text) == hash)
        .count()
}

/// The normalized text of a post-op block, for structural comparison.
#[must_use]
pub fn normalized_block(b: &Block) -> String {
    normalize(&b.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Config;
    use crate::identity::ops::{ManagedObservation, Operation, PostOp, apply};

    fn cfg() -> Config {
        Config::load().expect("load config")
    }

    /// M07.2: the worst-outcome ordering (D07.5) is total and correctly ranks
    /// `Lost < Ambiguous < Recovered(lower c) < Preserved(weaker grade)`.
    #[test]
    fn worst_ordering_is_total() {
        use IdentityGrade::{Anchored, Explicit, Managed};
        let ordered = [
            Outcome::Lost,
            Outcome::Ambiguous,
            Outcome::Recovered { confidence: 0.5 },
            Outcome::Recovered { confidence: 0.9 },
            Outcome::Preserved(Anchored),
            Outcome::Preserved(Explicit),
            Outcome::Preserved(Managed),
        ];
        for (i, a) in ordered.iter().enumerate() {
            for (j, b) in ordered.iter().enumerate() {
                assert_eq!(a.cmp(b), i.cmp(&j), "ordering mismatch at {i},{j}");
            }
        }
        // `worst` folds to the minimum (the worst).
        assert_eq!(
            worst([
                Outcome::Preserved(Explicit),
                Outcome::Lost,
                Outcome::Ambiguous
            ]),
            Outcome::Lost
        );
        assert_eq!(worst(std::iter::empty()), Outcome::Lost);
    }

    /// M07.3: two identical candidates tie → `Ambiguous` for content-hash and
    /// structural (the `duplicate_identical` op makes three identical anonymous
    /// blocks).
    #[test]
    fn duplicates_are_ambiguous() {
        let c = cfg();
        let world = BaseWorld::build(&c.template);
        let post = apply(Operation::DuplicateIdentical, &world, 11, &c).expect("apply");
        let anon = world
            .blocks
            .iter()
            .find(|b| b.id.is_none())
            .expect("anonymous block");
        assert_eq!(
            observe(Strategy::ContentHash, anon, &post, &c),
            Outcome::Ambiguous
        );
        assert_eq!(
            observe(Strategy::Structural, anon, &post, &c),
            Outcome::Ambiguous
        );
    }

    /// M07.3: a byte-exact copy of an id-bearing block duplicates its `{#id}` →
    /// inline-id `Ambiguous`; and the untouched original id resolves once
    /// elsewhere. Uses `copy_paste` seeded to copy block 0 (p-fourier).
    #[test]
    fn duplicate_inline_id_is_ambiguous() {
        let c = cfg();
        let world = BaseWorld::build(&c.template);
        // Directly duplicate the first id block's source and re-parse.
        let dup_file = format!(
            "{}\n\n{}",
            world.file.trim_end(),
            "The Fourier transform decomposes a signal\ninto its constituent frequencies. {#p-fourier}"
        );
        let post = PostOp {
            blocks: paragraph::parse(&dup_file),
            file: dup_file,
            sidecar: world.sidecar(),
            sidecar_moved: true,
            managed: ManagedObservation::Foreign,
            policy: None,
            base: world.clone(),
        };
        let b0 = &world.blocks[0];
        assert_eq!(
            observe(Strategy::InlineId, b0, &post, &c),
            Outcome::Ambiguous
        );
    }

    #[test]
    fn jaccard_bounds() {
        assert!((jaccard("abcdef", "abcdef", 3) - 1.0).abs() < 1e-9);
        assert!((jaccard("abc", "xyz", 3) - 0.0).abs() < 1e-9);
        // Short (< ngram) strings are a single whole-string gram.
        assert!((jaccard("ab", "ab", 3) - 1.0).abs() < 1e-9);
        assert!((jaccard("", "", 3) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn normalize_collapses_and_trims() {
        // Per-line trim + whitespace-run collapse; leading/trailing empty lines
        // dropped, interior blank lines preserved (D07.2).
        assert_eq!(normalize("  a   b  \n\n  c  "), "a b\n\nc");
        assert_eq!(normalize("\n\nx\n\n"), "x");
        assert_eq!(normalize("one\ttwo   three"), "one two three");
    }
}
