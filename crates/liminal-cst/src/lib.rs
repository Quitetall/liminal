//! L0 — the lossless Concrete Syntax Tree and the coarse structural scan.
//!
//! The CST preserves exact bytes, tokens, whitespace, comments, delimiters,
//! attribute order, alias spelling, invalid syntax, source ranges, and
//! embedded-language boundaries (v4 §9). The coarse structural scan cheaply
//! identifies block boundaries and leaves unneeded regions as [`OpaqueBlock`]
//! stubs so that large workspaces are never eagerly elaborated in full
//! (v4 §22, §23 demand-driven parsing).
//!
//! **RESERVED — implementation begins Phase 1** (v4 Part XXII). Law 14 forbids
//! any production parser, green-tree, rope, or incremental subtree machinery
//! before the Phase -1 falsification gates pass; the Phase -1 toy paragraph
//! format is a deliberate ~50-line hand parser living in the lab, not here.
//!
//! This crate exists now so the constitutional vocabulary — the §22
//! `OpaqueBlock` sketch and its coarse kinds — has a compiled, greppable home
//! that downstream seams (fuzz targets, §112 laws, conformance fixtures) can
//! name today.

use liminal_id::ContentHash;
use liminal_source::SourceRange;
use serde::{Deserialize, Serialize};

/// An unelaborated region left as a stub by the coarse structural scan
/// (verbatim from the v4 §22 sketch).
///
/// Unneeded regions remain opaque: only their exact byte range, content hash,
/// and coarse classification are known until demand-driven parsing (v4 §23)
/// elaborates them because they are visible, edited, queried, referenced,
/// required by a pass, or needed by a backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueBlock {
    /// Exact byte range of the block within its source basis (v4 §22).
    pub range: SourceRange,
    /// Content hash of the raw bytes, for structural sharing and change
    /// detection across revisions (v4 §9, §22).
    pub hash: ContentHash,
    /// Cheap classification produced by the coarse scan (v4 §22).
    pub coarse_kind: CoarseKind,
}

/// Coarse classification produced by the first structural pass (v4 §22 list).
///
/// SHAPE PROVISIONAL — the variant set transcribes the §22 bullet list
/// ("files and document roots, block boundaries, headings, lists, fences,
/// directives, embedded language regions, resource references"); Phase 1
/// grammar work (ADR pending, v4 §119) may reshape it freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoarseKind {
    /// A file or document root (v4 §22).
    DocumentRoot,
    /// A heading block (v4 §22).
    Heading,
    /// An ordinary paragraph-like block (v4 §22 "block boundaries").
    ParagraphLike,
    /// A list block (v4 §22).
    List,
    /// A fenced block (v4 §22).
    Fence,
    /// A directive block (v4 §22).
    Directive,
    /// An embedded-language region (v4 §9, §22).
    EmbeddedLanguage,
    /// A resource reference (v4 §22).
    ResourceReference,
    /// Anything the coarse scan cannot classify; error-tolerant parsing never
    /// rejects bytes (v4 §9 "invalid syntax").
    Unknown,
}
