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
use liminal_source::paragraph::{self, Block};

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
                let start = edit.start.min(source.len());
                let end = edit.end.min(source.len()).max(start);
                (source.is_char_boundary(start) && source.is_char_boundary(end)).then_some((
                    start,
                    end,
                    edit.replacement.as_str(),
                ))
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
        // Correctness first: this implementation deliberately shares the
        // canonical parser with full compilation until subtree reuse has its
        // own benchmark and oracle evidence.
        let edited = self.apply(source, edits);
        self.full(&edited, basis)
    }
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
    use super::{IncrementalCompiler, ParagraphCompiler, SourceEdit};

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
}
