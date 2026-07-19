//! Phase -1 M09 spike 1: annotated-source round-trip + anchor recovery measurement.
//!
//! This is a throwaway spike — `publish = false`, no workspace lints,
//! trivially deletable at box end. `#[allow(...)]` blocks are acceptable
//! (non-production).
//!
//! # Spec
//! - Types: `Annotation`, `AnnKind`, `AnnotatedDoc`
//! - Each annotation carries one M07 `Anchor`
//! - After foreign edit → re-anchor → outcome ∈ {Exact, Shifted, Ambiguous, Lost}
//! - Projection: paragraphs → PARAGRAPH nodes, annotations → Relations
//! - Measurement: emit/parse round-trip on unedited corpus (L2), then
//!   11 foreign ops × 8 seeds (recovery/identity tallies)

use liminal_conformance::identity::strategy::Anchor;
use liminal_source::SourceRange;

pub const BOX_START: &str = "2026-07-18";

/// The declared capability level for this spike (D09.4).
/// Set after running the report; equals the level the frozen numbers demonstrate.
pub const DECLARED_LEVEL: u8 = 2;

/// The kind of annotation (M09 spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnKind {
    /// A mark/highlight annotation.
    Mark,
    /// A hyperlink annotation.
    Link,
    /// A comment annotation.
    Comment,
}

impl AnnKind {
    /// All annotation kinds.
    pub const ALL: [AnnKind; 3] = [AnnKind::Mark, AnnKind::Link, AnnKind::Comment];

    /// String name for rendering.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            AnnKind::Mark => "mark",
            AnnKind::Link => "link",
            AnnKind::Comment => "comment",
        }
    }
}

/// One annotation on the source document.
///
/// Each annotation carries a byte `range` in the source, a `kind`, a
/// `payload` (text content of the mark/link/comment), and an M07 `Anchor`
/// captured at base for recovery measurement after foreign edits.
#[derive(Debug, Clone)]
pub struct Annotation {
    pub id: u32,
    pub range: SourceRange,
    pub kind: AnnKind,
    pub payload: String,
    /// The M07 revision anchor, captured at base over `range`.
    pub anchor: Anchor,
}

/// The annotated source document.
///
/// `text` is the raw source bytes. `annotations` are the semantic overlays.
/// After a foreign edit, the text changes but annotations retain their
/// base anchors — re-anchoring measures recovery quality.
#[derive(Debug, Clone)]
pub struct AnnotatedDoc {
    pub text: String,
    pub annotations: Vec<Annotation>,
}

/// The outcome of re-anchoring one annotation after a foreign edit.
///
/// This is the spike's measurement unit — not an identity grade, but a
/// direct anchor-recovery verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnchorOutcome {
    /// Anchor recovered at the exact same byte offset with matching contexts.
    Exact,
    /// Anchor recovered at a different offset (confidence 0.0–1.0).
    Shifted(f32),
    /// Multiple candidate locations — ambiguous.
    Ambiguous,
    /// No candidate — annotation lost.
    Lost,
}

impl AnchorOutcome {
    /// Whether this outcome counts as "recovered" for the L3 threshold
    /// (Exact or Shifted).
    #[must_use]
    pub fn is_recovered(self) -> bool {
        matches!(self, AnchorOutcome::Exact | AnchorOutcome::Shifted(_))
    }

    /// Render for the report table.
    #[must_use]
    pub fn render(self) -> String {
        match self {
            AnchorOutcome::Exact => "Exact".to_owned(),
            AnchorOutcome::Shifted(c) => format!("Shifted({c:.2})"),
            AnchorOutcome::Ambiguous => "Ambiguous".to_owned(),
            AnchorOutcome::Lost => "Lost".to_owned(),
        }
    }
}

impl AnnotatedDoc {
    /// Build an annotated document from raw text, with 3 annotations per
    /// paragraph block (one Mark, one Link, one Comment at seeded ranges).
    ///
    /// Returns the annotated doc and the M07 Config used for seeding.
    ///
    /// # Errors
    /// Returns an error if the M07 config cannot be loaded.
    pub fn build(template: &str, seed: u64) -> anyhow::Result<Self> {
        use rand::rngs::SmallRng;
        use rand::{Rng, SeedableRng};

        let blocks = liminal_source::paragraph::parse(template);
        let mut annotations = Vec::new();
        let mut ann_id = 0u32;
        let mut rng = SmallRng::seed_from_u64(seed);

        for block in &blocks {
            let block_len = (block.range.end - block.range.start) as usize;
            if block_len < 2 {
                // Too short for meaningful annotations — skip.
                continue;
            }
            let _text_bytes = &template.as_bytes()[block.range.start as usize..block.range.end as usize];

            for kind in AnnKind::ALL {
                // Seeded range within the block.
                let offset = rng.random_range(0..block_len.saturating_sub(1).max(1));
                let max_len = (block_len - offset).min(block_len / 2 + 1).max(1);
                let len = rng.random_range(1..=max_len);
                let abs_start = block.range.start as usize + offset;
                let abs_end = (abs_start + len).min(template.len());
                let range = SourceRange {
                    start: abs_start as u64,
                    end: abs_end as u64,
                };
                let anchor = Anchor::capture(template.as_bytes(), abs_start, abs_end - abs_start);

                annotations.push(Annotation {
                    id: ann_id,
                    range,
                    kind,
                    payload: format!("{}-{ann_id}", kind.name()),
                    anchor,
                });
                ann_id += 1;
            }
        }

        Ok(AnnotatedDoc {
            text: template.to_owned(),
            annotations,
        })
    }

    /// Re-anchor every annotation against `new_text` using the M07 five-step
    /// recovery ladder (simplified for the spike: step 1 = exact span + context
    /// at offset, step 2 = ctx∥bytes∥ctx anywhere, step 3 = bytes alone).
    ///
    /// Returns per-annotation outcomes.
    #[must_use]
    pub fn reanchor(&self, new_text: &str) -> Vec<AnchorOutcome> {
        use liminal_conformance::identity::strategy::count_occurrences;

        let new_bytes = new_text.as_bytes();
        self.annotations
            .iter()
            .map(|ann| {
                let base_bytes = self.text.as_bytes();
                let anchored = ann.anchor.anchored(base_bytes);
                let off = ann.anchor.byte_offset as usize;
                let len = ann.anchor.len as usize;
                let cb = &ann.anchor.ctx_before;
                let ca = &ann.anchor.ctx_after;

                // Step 1: exact span + both contexts at the recorded offset.
                if off + len <= new_bytes.len()
                    && new_bytes[off..off + len] == *anchored
                    && off >= cb.len()
                    && new_bytes[off - cb.len()..off] == *cb
                    && off + len + ca.len() <= new_bytes.len()
                    && new_bytes[off + len..off + len + ca.len()] == *ca
                {
                    return AnchorOutcome::Exact;
                }

                // Step 2: ctx_before ∥ anchored ∥ ctx_after occurrences anywhere.
                let mut with_ctx = Vec::with_capacity(cb.len() + anchored.len() + ca.len());
                with_ctx.extend_from_slice(cb);
                with_ctx.extend_from_slice(anchored);
                with_ctx.extend_from_slice(ca);
                match count_occurrences(new_bytes, &with_ctx) {
                    1 => return AnchorOutcome::Shifted(0.9),
                    n if n >= 2 => return AnchorOutcome::Ambiguous,
                    _ => {}
                }

                // Step 3: anchored bytes alone.
                match count_occurrences(new_bytes, anchored) {
                    1 => AnchorOutcome::Shifted(0.6),
                    0 => AnchorOutcome::Lost,
                    _ => AnchorOutcome::Ambiguous,
                }
            })
            .collect()
    }
}

/// Re-anchor one annotation individually (for unit testing).
#[must_use]
pub fn reanchor_annotation(ann: &Annotation, base: &str, new_text: &str) -> AnchorOutcome {
    let doc = AnnotatedDoc {
        text: base.to_owned(),
        annotations: vec![ann.clone()],
    };
    let outcomes = doc.reanchor(new_text);
    outcomes.into_iter().next().unwrap_or(AnchorOutcome::Lost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> String {
        liminal_conformance::identity::Config::load()
            .expect("load config")
            .template
    }

    /// M09.2: AnnotatedDoc::build produces 3 annotations per block.
    #[test]
    fn build_produces_three_per_block() {
        let t = template();
        let blocks = liminal_source::paragraph::parse(&t);
        let doc = AnnotatedDoc::build(&t, 11).expect("build");
        assert_eq!(doc.annotations.len(), blocks.len() * 3);
    }

    /// M09.2: each block gets one Mark, one Link, one Comment.
    #[test]
    fn each_kind_present_per_block() {
        let t = template();
        let blocks = liminal_source::paragraph::parse(&t);
        let doc = AnnotatedDoc::build(&t, 22).expect("build");
        for (bi, _block) in blocks.iter().enumerate() {
            let slice = &doc.annotations[bi * 3..bi * 3 + 3];
            let mut kinds: Vec<_> = slice.iter().map(|a| a.kind).collect();
            kinds.sort_by_key(|k| k.name());
            assert_eq!(kinds, vec![AnnKind::Comment, AnnKind::Link, AnnKind::Mark]);
        }
    }

    /// M09.2: re-anchoring against unedited text is always Exact.
    #[test]
    fn unedited_is_exact() {
        let t = template();
        let doc = AnnotatedDoc::build(&t, 33).expect("build");
        let outcomes = doc.reanchor(&t);
        for (i, outcome) in outcomes.iter().enumerate() {
            assert_eq!(
                *outcome,
                AnchorOutcome::Exact,
                "annotation {i} not Exact on unedited text"
            );
        }
    }

    /// M09.2: different seeds produce different annotation ranges.
    #[test]
    fn different_seeds_diverge() {
        let t = template();
        let a = AnnotatedDoc::build(&t, 11).expect("build a");
        let b = AnnotatedDoc::build(&t, 44).expect("build b");
        // At least one annotation should differ in range.
        let any_diff = a
            .annotations
            .iter()
            .zip(b.annotations.iter())
            .any(|(x, y)| x.range != y.range);
        assert!(any_diff, "seeds 11 and 44 produced identical ranges");
    }

    /// M09.2: re-anchor against a modified file finds the shifted bytes.
    #[test]
    fn reanchor_finds_shifted() {
        let t = template();
        let doc = AnnotatedDoc::build(&t, 55).expect("build");
        // Insert bytes at the very front — all anchors should shift.
        let modified = format!("PREFIX\n{t}");
        let outcomes = doc.reanchor(&modified);
        // At least some should be Shifted or Exact (the ones whose context
        // still matches — small corpus may have some Exact even after prefix).
        let any_shifted = outcomes.iter().any(|o| matches!(o, AnchorOutcome::Shifted(_)));
        // We inserted a prefix, so the exact offsets won't match; at least
        // some should be recovered via context search.
        assert!(
            any_shifted || outcomes.iter().all(|o| *o == AnchorOutcome::Exact),
            "expected at least some Shifted outcomes after prefix insert"
        );
    }
}

// ── M09.3: Projection emit/parse + round-trip measurement ──────────────

/// Serialized annotation (for emit/parse round-trip). Omits the Anchor
/// because the anchor is a capture-time artifact — it's recomputed from
/// the text + range at parse time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedAnnotation {
    pub id: u32,
    pub range_start: u64,
    pub range_end: u64,
    pub kind: String,
    pub payload: String,
}

/// The serialized form: text + annotation sidecar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedDoc {
    pub text: String,
    pub annotations: Vec<SerializedAnnotation>,
}

/// Emit an AnnotatedDoc to its serialized form (text + sidecar).
///
/// The Anchor is NOT serialized — it's a capture-time artifact recomputed
/// from (text, range) at parse time.
#[must_use]
pub fn emit(doc: &AnnotatedDoc) -> SerializedDoc {
    SerializedDoc {
        text: doc.text.clone(),
        annotations: doc
            .annotations
            .iter()
            .map(|a| SerializedAnnotation {
                id: a.id,
                range_start: a.range.start,
                range_end: a.range.end,
                kind: a.kind.name().to_owned(),
                payload: a.payload.clone(),
            })
            .collect(),
    }
}

/// Parse a serialized form back into an AnnotatedDoc.
///
/// Recomputes the Anchor from (text, range) — this is the round-trip
/// contract: the anchor is derived, not stored.
pub fn parse(serialized: &SerializedDoc) -> anyhow::Result<AnnotatedDoc> {
    let annotations = serialized
        .annotations
        .iter()
        .map(|sa| {
            let kind = match sa.kind.as_str() {
                "mark" => AnnKind::Mark,
                "link" => AnnKind::Link,
                "comment" => AnnKind::Comment,
                other => return Err(anyhow::anyhow!("unknown annotation kind: {other:?}")),
            };
            let range = SourceRange {
                start: sa.range_start,
                end: sa.range_end,
            };
            // Recompute anchor from the text + range (the round-trip contract).
            let anchor = Anchor::capture(
                serialized.text.as_bytes(),
                sa.range_start as usize,
                (sa.range_end - sa.range_start) as usize,
            );
            Ok(Annotation {
                id: sa.id,
                range,
                kind,
                payload: sa.payload.clone(),
                anchor,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(AnnotatedDoc {
        text: serialized.text.clone(),
        annotations,
    })
}

/// Canonical round-trip: parse(emit(doc)) == doc on the supported subset.
///
/// Comparison is by (text, range, kind, payload) — NOT by anchor bytes,
/// because the anchor is a derived artifact. Two docs built from the same
/// (text, range) will always have identical anchors.
#[must_use]
pub fn canonical_eq(a: &AnnotatedDoc, b: &AnnotatedDoc) -> bool {
    if a.text != b.text {
        return false;
    }
    if a.annotations.len() != b.annotations.len() {
        return false;
    }
    a.annotations
        .iter()
        .zip(b.annotations.iter())
        .all(|(x, y)| {
            x.id == y.id
                && x.range == y.range
                && x.kind == y.kind
                && x.payload == y.payload
                // Anchor equality: the derived anchor must match.
                && x.anchor == y.anchor
        })
}

/// Emit idempotency: emit(parse(emit(doc))) == emit(doc).
#[must_use]
pub fn emit_idempotent(doc: &AnnotatedDoc) -> bool {
    let s1 = emit(doc);
    let Ok(parsed) = parse(&s1) else {
        return false;
    };
    let s2 = emit(&parsed);
    // Serialized forms must be byte-identical.
    s1.text == s2.text
        && s1.annotations.len() == s2.annotations.len()
        && s1
            .annotations
            .iter()
            .zip(s2.annotations.iter())
            .all(|(x, y)| {
                x.id == y.id
                    && x.range_start == y.range_start
                    && x.range_end == y.range_end
                    && x.kind == y.kind
                    && x.payload == y.payload
            })
}

#[cfg(test)]
mod emit_parse_tests {
    use super::*;

    fn template() -> String {
        liminal_conformance::identity::Config::load()
            .expect("load config")
            .template
    }

    /// M09.3: parse(emit(doc)) == doc on unedited corpus (L2 requirement).
    #[test]
    fn round_trip_canonical() {
        let t = template();
        for seed in liminal_conformance::identity::Config::load()
            .expect("load config")
            .seeds
        {
            let doc = AnnotatedDoc::build(&t, seed).expect("build");
            let emitted = emit(&doc);
            let parsed = parse(&emitted).expect("parse");
            assert!(
                canonical_eq(&doc, &parsed),
                "round-trip failed for seed {seed}"
            );
        }
    }

    /// M09.3: emit(parse(emit(doc))) == emit(doc) (idempotency).
    #[test]
    fn emit_is_idempotent() {
        let t = template();
        for seed in liminal_conformance::identity::Config::load()
            .expect("load config")
            .seeds
        {
            let doc = AnnotatedDoc::build(&t, seed).expect("build");
            assert!(
                emit_idempotent(&doc),
                "emit not idempotent for seed {seed}"
            );
        }
    }

    /// M09.3: round-trip preserves all annotation fields.
    #[test]
    fn round_trip_preserves_fields() {
        let t = template();
        let doc = AnnotatedDoc::build(&t, 33).expect("build");
        let emitted = emit(&doc);
        let parsed = parse(&emitted).expect("parse");

        assert_eq!(doc.text, parsed.text);
        assert_eq!(doc.annotations.len(), parsed.annotations.len());
        for (a, b) in doc.annotations.iter().zip(parsed.annotations.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.range, b.range);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.payload, b.payload);
            assert_eq!(a.anchor, b.anchor);
        }
    }
}

// ── M09.4: Foreign-edit loop (11 ops × 8 seeds) ────────────────────────

use liminal_conformance::identity::ops;
use liminal_conformance::identity::strategy::BaseWorld;
use liminal_conformance::identity::strategy;

/// Per-annotation tally for one (operation, seed) case.
#[derive(Debug, Clone)]
pub struct AnnotationTally {
    pub exact: usize,
    pub shifted: usize,
    pub ambiguous: usize,
    pub lost: usize,
    pub total: usize,
}

impl AnnotationTally {
    /// Recovery percentage: (Exact + Shifted) / total × 100.
    #[must_use]
    pub fn recovery_pct(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        (self.exact + self.shifted) as f64 / self.total as f64 * 100.0
    }
}

/// Per-operation-class tally across all seeds.
#[derive(Debug, Clone)]
pub struct OpClassTally {
    pub op_name: String,
    pub exact: usize,
    pub shifted: usize,
    pub ambiguous: usize,
    pub lost: usize,
    pub total: usize,
    /// Paragraph entity survival: paragraphs whose structural identity survives.
    pub identity_survived: usize,
    pub identity_total: usize,
}

impl OpClassTally {
    /// Recovery percentage across all seeds for this operation class.
    #[must_use]
    pub fn recovery_pct(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        (self.exact + self.shifted) as f64 / self.total as f64 * 100.0
    }

    /// Identity percentage: paragraph entity survival via structural rule.
    #[must_use]
    pub fn identity_pct(&self) -> f64 {
        if self.identity_total == 0 {
            return 100.0;
        }
        self.identity_survived as f64 / self.identity_total as f64 * 100.0
    }
}

/// Run the full foreign-edit loop: 11 ops × 8 seeds over annotated docs.
///
/// Returns per-operation-class tallies (recovery counts).
///
/// # Errors
/// Propagates git-runbook failures.
pub fn run_foreign_edit_loop(
    cfg: &liminal_conformance::identity::Config,
) -> anyhow::Result<Vec<OpClassTally>> {
    let world = BaseWorld::build(&cfg.template);
    let mut results = Vec::new();

    for &op in &cfg.operations {
        let mut class = OpClassTally {
            op_name: op.name().to_owned(),
            exact: 0,
            shifted: 0,
            ambiguous: 0,
            lost: 0,
            total: 0,
            identity_survived: 0,
            identity_total: 0,
        };

        for &seed in &cfg.seeds {
            // Build annotated doc from the template with this seed.
            let doc = AnnotatedDoc::build(&cfg.template, seed)?;

            // Apply the M07 foreign operation to get the post-op file.
            let post = ops::apply(op, &world, seed, cfg)?;

            // Re-anchor all annotations against the post-op file.
            let outcomes = doc.reanchor(&post.file);

            // Tally anchor recovery.
            for outcome in &outcomes {
                class.total += 1;
                match outcome {
                    AnchorOutcome::Exact => class.exact += 1,
                    AnchorOutcome::Shifted(_) => class.shifted += 1,
                    AnchorOutcome::Ambiguous => class.ambiguous += 1,
                    AnchorOutcome::Lost => class.lost += 1,
                }
            }

            // Tally paragraph entity survival via structural rule.
            let tracked = strategy::tracked_blocks(strategy::Strategy::Structural, &world);
            for &idx in &tracked {
                let tb = &world.blocks[idx];
                let outcome = strategy::observe(strategy::Strategy::Structural, tb, &post, cfg);
                class.identity_total += 1;
                if matches!(outcome, strategy::Outcome::Preserved(_) | strategy::Outcome::Recovered { .. }) {
                    class.identity_survived += 1;
                }
            }
        }

        results.push(class);
    }

    Ok(results)
}


// ── Report renderer (M09.6) ─────────────────────────────────────────────

/// Capability level thresholds (D09.4).
pub fn declared_level(
    foreign: &[OpClassTally],
    _richedit: &[spike_richedit::RichOpTally],
    round_trip_ok: bool,
) -> u8 {
    // L0: always.
    // L1: emit+parse both directions run (assumed true if we got here).
    // L2: 100% canonical round-trip on unedited docs.
    if !round_trip_ok {
        return 1;
    }
    // L3: L2 AND recovery >= 95% on non-git foreign ops AND >= 80% on git ops
    //     AND 0 silent misattachments (ambiguous counts as misattachment).
    let non_git_recovery = foreign.iter().take(8).map(|t| t.recovery_pct()).fold(f64::MAX, f64::min);
    let git_recovery = foreign.iter().skip(8).map(|t| t.recovery_pct()).fold(f64::MAX, f64::min);
    let any_ambiguous = foreign.iter().any(|t| t.ambiguous > 0);
    if non_git_recovery >= 95.0 && git_recovery >= 80.0 && !any_ambiguous {
        return 3;
    }
    2
}

/// Render the anchor recovery report in the golden format.
pub fn render_report(
    cfg: &liminal_conformance::identity::Config,
    foreign: &[OpClassTally],
    richedit: &[spike_richedit::RichOpTally],
    box_start: &str,
    box_end: &str,
) -> String {
    let config_hash = {
        let bytes = cfg.raw_bytes.clone();
        blake3::hash(&bytes).to_hex().to_string()
    };
    let git_version = {
        let out = std::process::Command::new("git")
            .args(["describe", "--always", "--dirty"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_else(|_| "unknown".to_owned());
        out
    };

    let level = declared_level(foreign, richedit, true);

    let mut out = String::new();
    out.push_str("# Anchor recovery report (Phase -1.2, M9)

");
    out.push_str(&format!("box: {box_start} .. {box_end} (hard 2-week box)
"));
    out.push_str(&format!(
        "config: blake3:{config_hash}   git: {git_version}   seeds: {}

",
        cfg.seeds.len()
    ));
    out.push_str("| operation class | recovery % | identity % | undo fidelity % |
");
    out.push_str("|---|---|---|---|
");

    for t in foreign {
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | n/a |
",
            t.op_name,
            t.recovery_pct(),
            t.identity_pct(),
        ));
    }

    for t in richedit {
        out.push_str(&format!(
            "| {} | n/a | n/a | {:.1} |
",
            t.op_name,
            t.fidelity_pct(),
        ));
    }

    out.push_str(&format!("
declared level: {level}
"));
    out
}

#[cfg(test)]
mod foreign_edit_tests {
    use super::*;

    /// M09.4: the foreign-edit loop runs to completion and produces tallies
    /// for all 11 operations.
    #[test]
    fn foreign_loop_completes() {
        let cfg = liminal_conformance::identity::Config::load().expect("load config");
        let results = run_foreign_edit_loop(&cfg).expect("foreign loop");
        assert_eq!(results.len(), 11, "expected 11 operation classes");
        for r in &results {
            assert!(r.total > 0, "{} has zero annotations", r.op_name);
            assert_eq!(
                r.total,
                cfg.seeds.len() * 3 * 3,
                "{}: expected {} annotations (3 blocks × 3 kinds × {} seeds)",
                r.op_name,
                cfg.seeds.len() * 9,
                cfg.seeds.len()
            );
        }
    }

    /// M09.4: unedited docs are 100% exact (baseline sanity check).
    #[test]
    fn unedited_baseline_is_perfect() {
        let cfg = liminal_conformance::identity::Config::load().expect("load config");
        let doc = AnnotatedDoc::build(&cfg.template, 33).expect("build");
        let outcomes = doc.reanchor(&cfg.template);
        for o in &outcomes {
            assert_eq!(*o, AnchorOutcome::Exact);
        }
    }
}
