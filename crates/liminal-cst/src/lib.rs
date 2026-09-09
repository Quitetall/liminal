//! L0 — the lossless Concrete Syntax Tree and the coarse structural scan.
//!
//! The CST preserves exact bytes, tokens, whitespace, comments, delimiters,
//! attribute order, alias spelling, invalid syntax, source ranges, and
//! embedded-language boundaries (v4 §9). The coarse structural scan cheaply
//! identifies block boundaries and leaves unneeded regions as [`OpaqueBlock`]
//! stubs so that large workspaces are never eagerly elaborated in full
//! (v4 §22, §23 demand-driven parsing).
//!
//! Phase 1 error-tolerant coarse CST. This is intentionally not a full
//! grammar: it records exact source bytes and conservative block ranges while
//! deferring semantic elaboration.

use liminal_id::ContentHash;
use liminal_source::SourceRange;
use serde::{Deserialize, Serialize};

mod parser;
pub use parser::{
    LiminalLanguage, MAX_ERRORS, MAX_NESTING, Parse, ParseError, ParseErrorCode, SyntaxKind,
    SyntaxNode, parse,
};

/// Lossless coarse CST document. `source` is authoritative for emit, so even
/// malformed or unsupported syntax survives byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstDocument {
    /// Original UTF-8 source bytes.
    pub source: String,
    /// Coarse blocks discovered without rejecting input.
    pub blocks: Vec<OpaqueBlock>,
}

/// Parse source without rejection or panics.
#[must_use]
pub fn coarse_parse(source: &str) -> CstDocument {
    let mut blocks = Vec::new();
    let mut block_start = 0usize;
    let mut cursor = 0usize;
    let mut in_block = false;
    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let blank = content.trim().is_empty();
        if blank {
            if in_block {
                blocks.push(block(source, block_start, cursor));
                in_block = false;
            }
        } else if !in_block {
            block_start = cursor;
            in_block = true;
        }
        cursor += line.len();
    }
    if in_block {
        blocks.push(block(source, block_start, source.len()));
    }
    CstDocument {
        source: source.to_owned(),
        blocks,
    }
}

/// Emit exact original source bytes.
#[must_use]
pub fn emit(document: &CstDocument) -> &str {
    &document.source
}

fn block(source: &str, start: usize, end: usize) -> OpaqueBlock {
    let text = &source[start..end];
    let first = text.lines().next().unwrap_or_default().trim_start();
    // M17.5 F-62: `#` alone made every hash-prefixed line a heading, including
    // `#!liminal-explicit-v1`, which selects a dialect and is not a heading.
    // `-` alone made `-not-a-list` a list while `*` already required its
    // space. Both now match the formatter's own rule: a marker, then a space.
    let hashes = first.chars().take_while(|ch| *ch == '#').count();
    let heading = hashes > 0 && first[hashes..].starts_with(' ');
    let coarse_kind = if first.starts_with("```") {
        CoarseKind::Fence
    } else if heading {
        CoarseKind::Heading
    } else if first.starts_with("- ") || first.starts_with("* ") {
        CoarseKind::List
    } else if first.starts_with('@') {
        CoarseKind::Directive
    } else if first.starts_with("![") || first.contains("](") {
        CoarseKind::ResourceReference
    } else {
        CoarseKind::ParagraphLike
    };
    OpaqueBlock {
        range: SourceRange {
            start: start as u64,
            end: end as u64,
        },
        hash: ContentHash::of(text.as_bytes()),
        coarse_kind,
    }
}

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
