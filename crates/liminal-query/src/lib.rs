//! Pure incremental queries over an explicit Workspace Basis.
//!
//! Every expensive pure computation is a revisioned query over a declared
//! [`WorkspaceBasis`]: deterministic for that basis, immutable, memoized,
//! dependency-tracked, and replayable from frozen Jurisdiction inputs
//! (v4 §24). External resolution is deliberately absent: a resolver performs
//! effects OUTSIDE the query engine and submits a new Jurisdiction
//! transaction; pure queries then consume that accepted input (v4 §24, §8.6).
//! This is the Salsa boundary adopted verbatim — pure tracked queries versus
//! external input mutation, extended with Workspace Basis and freshness
//! rather than performing effects inside queries (v4 §118A).
//!
//! **RESERVED — implementation begins Phase -1.3** (v4 Part XXII), where the
//! revision/Basis/invalidation prototype promotes a tiny pure query layer.
//! No memoization, no Salsa dependency, no parallel scheduling before then
//! (Law 14: interpretive reference semantics only).
//!
//! This crate exists now so the constitutional vocabulary has a compiled,
//! greppable home: the trait seams below let the §112 correctness laws
//! (`incremental_compile(x, edits) ≡ full_compile(apply(x, edits))`) compile
//! as generic functions in the conformance suite today, years before the
//! first real implementation plugs in.

pub mod memo;

pub use memo::{MemoEntry, MemoTable, entry_over_basis};

use liminal_revision::{ComponentDeps, WorkspaceBasis};
use liminal_source::{
    SourceRange,
    paragraph::{self, Block},
};

/// UTF-8 byte edit used by the Phase 1 incremental compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEdit {
    /// Inclusive byte start in original source.
    pub start: usize,
    /// Exclusive byte end in original source.
    pub end: usize,
    /// Replacement text.
    pub replacement: String,
}

/// Bounded paragraph compiler used by Phase 1 equivalence laws.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParagraphCompiler;

impl IncrementalCompiler for ParagraphCompiler {
    type Source = String;
    type Edit = SourceEdit;
    type Output = Vec<Block>;

    fn full(&self, source: &Self::Source, _basis: &WorkspaceBasis) -> Self::Output {
        paragraph::parse(source)
    }

    fn apply(&self, source: &Self::Source, edits: &[Self::Edit]) -> Self::Source {
        let mut output = source.clone();
        let mut ordered = edits
            .iter()
            .filter_map(|edit| {
                (edit.start <= edit.end
                    && edit.end <= source.len()
                    && source.is_char_boundary(edit.start)
                    && source.is_char_boundary(edit.end))
                .then_some((edit.start, edit.end, edit.replacement.as_str()))
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));

        for index in 0..ordered.len() {
            let (start, end, replacement) = ordered[index];
            // Edits use original-source offsets. Overlapping edits have no
            // deterministic sequential meaning, so discard every member of
            // an overlap set fail-closed.
            let overlaps =
                ordered
                    .iter()
                    .enumerate()
                    .any(|(other, &(other_start, other_end, _))| {
                        other != index && start < other_end && other_start < end
                    });
            if overlaps {
                continue;
            }
            output.replace_range(start..end, replacement);
        }
        output
    }

    fn incremental(
        &self,
        source: &Self::Source,
        edits: &[Self::Edit],
        basis: &WorkspaceBasis,
    ) -> Self::Output {
        // Keep this deliberately independent from `full`: equivalence tests
        // need an oracle capable of exposing a divergence in the canonical
        // parser rather than merely calling it twice.
        let edited = self.apply(source, edits);
        let _ = basis;
        incremental_paragraph_oracle(&edited)
    }
}

/// Independently scan the tiny paragraph grammar used as the Phase 1
/// incremental oracle. This intentionally does not call `paragraph::parse`.
fn incremental_paragraph_oracle(input: &str) -> Vec<Block> {
    let line_starts = std::iter::once(0)
        .chain(input.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut current_lines = Vec::new();
    let mut block_start = None;
    let mut block_end = 0;

    for (line_index, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            if let Some(start) = block_start.take() {
                blocks.push(build_oracle_block(&current_lines, start, block_end));
                current_lines.clear();
            }
            continue;
        }

        let line_start = line_starts[line_index];
        block_start.get_or_insert(line_start);
        block_end = line_start + line.len();
        current_lines.push(line);
    }

    if let Some(start) = block_start {
        blocks.push(build_oracle_block(&current_lines, start, block_end));
    }

    blocks
}

fn build_oracle_block(lines: &[&str], start: usize, end: usize) -> Block {
    let last_line = lines.last().expect("oracle blocks are non-empty");
    let (id, cleaned_last_line) = extract_oracle_marker(last_line);
    let mut text_lines = lines.to_vec();

    if let Some(cleaned) = cleaned_last_line.as_deref() {
        if cleaned.is_empty() {
            text_lines.pop();
        } else {
            *text_lines.last_mut().expect("oracle blocks are non-empty") = cleaned;
        }
    }

    Block {
        text: text_lines.join("\n"),
        id,
        range: SourceRange {
            start: start as u64,
            end: end as u64,
        },
    }
}

/// Independently implement paragraph's terminal `{#id}` grammar.
fn extract_oracle_marker(line: &str) -> (Option<String>, Option<String>) {
    let Some(marker_start) = line.rfind("{#") else {
        return (None, None);
    };
    let marker = &line[marker_start + 2..];
    let Some(close) = marker.find('}') else {
        return (None, None);
    };
    let id = &marker[..close];

    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        || !marker[close + 1..].trim().is_empty()
    {
        return (None, None);
    }

    let cleaned = line[..marker_start].trim_end().to_owned();
    (Some(id.to_owned()), Some(cleaned))
}

/// A pure revisioned computation over a declared Workspace Basis (v4 §24).
///
/// SHAPE PROVISIONAL — this seam exists so §112 laws and the conformance
/// harness compile as generics now; Phase -1.3/0 may reshape it freely.
///
/// Contract (v4 §24): results are deterministic for a declared basis and
/// immutable once produced. Implementations must record every Basis component
/// they read into `deps` — component-granular dependency tracking is what
/// makes precise invalidation possible (v4 §7.5; R4 §10 "buffer generations
/// invalidate only dependent queries"). Effects are forbidden inside
/// `execute` (v4 §8.6, §118A).
pub trait Query {
    /// The immutable query result.
    type Output;

    /// Execute the query at `basis`, recording every component read into
    /// `deps` (v4 §24 "dependency tracking", "precise invalidation").
    fn execute(&self, basis: &WorkspaceBasis, deps: &mut ComponentDeps) -> Self::Output;
}

/// The incremental-equals-full compilation seam (v4 §112).
///
/// SHAPE PROVISIONAL — exists only so the §112 law compiles as a generic
/// conformance test now; Phase -1.3/1 may reshape it freely.
///
/// Law (v4 §112): at the same Basis,
/// `incremental(x, edits)` must equal `full(apply(x, edits))` —
/// an incremental compile may never diverge from the from-scratch compile of
/// the edited source. `Output: PartialEq` exists precisely so the law is
/// checkable, and so unchanged results can stop invalidation cascades
/// (v4 §24 "equality checks to prevent cascading unchanged results").
pub trait IncrementalCompiler {
    /// The source representation being compiled.
    type Source;
    /// One edit applicable to the source.
    type Edit;
    /// The compilation result; `PartialEq` so the §112 law is checkable.
    type Output: PartialEq;

    /// Compile `source` from scratch at `basis`.
    fn full(&self, source: &Self::Source, basis: &WorkspaceBasis) -> Self::Output;

    /// Apply `edits` to `source`, producing the edited source.
    fn apply(&self, source: &Self::Source, edits: &[Self::Edit]) -> Self::Source;

    /// Incrementally recompile `source` under `edits` at `basis`.
    ///
    /// Must equal `self.full(&self.apply(source, edits), basis)` (v4 §112).
    fn incremental(
        &self,
        source: &Self::Source,
        edits: &[Self::Edit],
        basis: &WorkspaceBasis,
    ) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use super::{IncrementalCompiler, ParagraphCompiler, SourceEdit, incremental_paragraph_oracle};
    use liminal_source::{SourceRange, paragraph};

    #[test]
    fn multiple_original_offset_edits_apply_back_to_front() {
        let source = "abcdef".to_owned();
        let edits = [
            SourceEdit {
                start: 0,
                end: 1,
                replacement: "A".to_owned(),
            },
            SourceEdit {
                start: 4,
                end: 6,
                replacement: "EF!".to_owned(),
            },
        ];
        assert_eq!(ParagraphCompiler.apply(&source, &edits), "AbcdEF!");
    }

    #[test]
    fn overlapping_original_offset_edits_are_ignored() {
        let source = "abcdef".to_owned();
        let edits = [
            SourceEdit {
                start: 1,
                end: 4,
                replacement: "X".to_owned(),
            },
            SourceEdit {
                start: 3,
                end: 5,
                replacement: "Y".to_owned(),
            },
        ];
        assert_eq!(ParagraphCompiler.apply(&source, &edits), "abcdef");
    }

    #[test]
    fn out_of_bounds_original_offset_edits_are_ignored() {
        let source = "abcdef".to_owned();
        let edits = [SourceEdit {
            start: source.len() + 1,
            end: source.len() + 2,
            replacement: "!".to_owned(),
        }];
        assert_eq!(ParagraphCompiler.apply(&source, &edits), source);
    }

    #[test]
    fn incremental_oracle_preserves_marker_blank_and_range_semantics() {
        let source = "\nalpha {#a}\n\n  \nβeta\n  {#second_id}\n\nmalformed {#bad id!}\n";

        let blocks = incremental_paragraph_oracle(source);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].text, "alpha");
        assert_eq!(blocks[0].id.as_deref(), Some("a"));
        assert_eq!(blocks[0].range, SourceRange { start: 1, end: 11 });
        assert_eq!(blocks[1].text, "βeta");
        assert_eq!(blocks[1].id.as_deref(), Some("second_id"));
        assert_eq!(blocks[1].range, SourceRange { start: 16, end: 36 });
        assert_eq!(blocks[2].text, "malformed {#bad id!}");
        assert_eq!(blocks[2].id, None);
        assert_eq!(blocks[2].range, SourceRange { start: 38, end: 58 });
    }

    #[test]
    fn independent_oracle_agrees_with_canonical_parser_and_exposes_divergence() {
        let source = "one {#one}\n\ntwo\n  {#two}";
        let canonical = paragraph::parse(source);
        let oracle = incremental_paragraph_oracle(source);
        assert_eq!(oracle, canonical);

        let mut diverged = oracle;
        diverged[1].range.end -= 1;
        assert_ne!(diverged, canonical, "range divergences must be observable");
    }
}
