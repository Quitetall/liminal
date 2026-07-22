//! Deterministic source formatting — the engine behind `lim fmt`.
//!
//! Liminal ships a built-in Ruff-like toolchain; `lim fmt` is its
//! deterministic source formatter (v4 §20). Two formatter laws are
//! constitutional (v4 §20, restated as correctness properties in §112):
//!
//! 1. **Formatting is idempotent** — formatting already-formatted source
//!    changes nothing; as a §112 law, `parse(format(parse(x))) ≡ parse(x)`.
//! 2. **Parsing canonical formatting recovers the same semantic graph** —
//!    the formatter may normalize spelling, never meaning.
//!
//! Phase 1 ships a deliberately bounded paragraph-compatible implementation.
//! It is not a full Markdown grammar: unsupported constructs remain literal
//! paragraph text until a grammar ADR expands this surface.

use liminal_source::paragraph::{self, Block};

/// Parsed document for the Phase 1 paragraph-compatible formatter.
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    /// Blocks in source order. Byte ranges remain provenance metadata and do
    /// not participate in semantic document equality.
    pub blocks: Vec<Block>,
}

impl PartialEq for MarkdownDocument {
    fn eq(&self, other: &Self) -> bool {
        self.blocks
            .iter()
            .map(|block| (&block.text, &block.id))
            .eq(other.blocks.iter().map(|block| (&block.text, &block.id)))
    }
}

impl Eq for MarkdownDocument {}

/// Formatter error. The Phase 1 parser is total, so this is reserved for API
/// compatibility and future bounded diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownError;

impl std::fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("markdown formatting error")
    }
}

impl std::error::Error for MarkdownError {}

/// Deterministic paragraph-compatible formatter.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownFormatter;

impl Formatter for MarkdownFormatter {
    type Doc = MarkdownDocument;
    type Error = MarkdownError;

    fn parse(&self, source: &str) -> Result<Self::Doc, Self::Error> {
        Ok(MarkdownDocument {
            blocks: paragraph::parse(source),
        })
    }

    fn emit(&self, doc: &Self::Doc) -> Result<String, Self::Error> {
        Ok(doc
            .blocks
            .iter()
            .map(block_source)
            .collect::<Vec<_>>()
            .join("\n\n"))
    }

    fn format(&self, source: &str) -> Result<String, Self::Error> {
        let doc = self.parse(source)?;
        self.emit(&doc)
    }
}

fn block_source(block: &Block) -> String {
    match &block.id {
        Some(id) => format!("{} {{#{id}}}", block.text),
        None => block.text.clone(),
    }
}

/// Deterministic HTML renderer for the same bounded document projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render paragraph blocks to stable HTML. Text is escaped before it is
    /// placed in the document; IDs become stable `data-node-id` attributes.
    pub fn render(&self, source: &str) -> Result<String, MarkdownError> {
        let doc = MarkdownFormatter.parse(source)?;
        self.render_blocks(&doc.blocks)
    }

    /// Render already compiled paragraph blocks. This is the output side of
    /// the incremental compiler seam; it does not reparse source text.
    pub fn render_blocks(&self, blocks: &[Block]) -> Result<String, MarkdownError> {
        let mut out = String::from("<article>\n");
        for block in blocks {
            let id = block
                .id
                .as_deref()
                .map(|value| format!(" data-node-id=\"{}\"", escape_html(value)))
                .unwrap_or_default();
            let text = escape_html(&block.text).replace('\n', "<br />\n");
            out.push_str("<p");
            out.push_str(&id);
            out.push('>');
            out.push_str(&text);
            out.push_str("</p>\n");
        }
        out.push_str("</article>\n");
        Ok(out)
    }
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The formatter seam over an opaque parsed document (v4 §20).
///
/// SHAPE PROVISIONAL — exists only so the §112 formatter laws compile as
/// generic conformance tests now; Phase 1 may reshape it freely (in
/// particular, `parse` here is a formatting-front-end convenience and will be
/// reconciled with the real CST pipeline in `liminal-cst`).
///
/// Laws (v4 §20, §112):
/// - `format` is idempotent: `format(format(s)?)? == format(s)?`.
/// - Parsing canonical formatting recovers the same semantic graph:
///   `parse(format(parse(x))) ≡ parse(x)`. `Doc` intentionally carries no
///   `PartialEq` bound yet — Phase 1 decides whether document equality is
///   structural or graph-semantic (v4 §119 grammar ADR).
pub trait Formatter {
    /// Opaque parsed-document handle.
    type Doc;
    /// Formatter failure; malformed source must degrade, never panic
    /// (v4 §113 "malformed source recovery tests").
    type Error;

    /// Parse source text into a document.
    fn parse(&self, source: &str) -> Result<Self::Doc, Self::Error>;

    /// Emit canonical text for a document.
    fn emit(&self, doc: &Self::Doc) -> Result<String, Self::Error>;

    /// Format source text: canonically re-emit it (`emit(parse(source))`).
    ///
    /// Must be idempotent (v4 §20).
    fn format(&self, source: &str) -> Result<String, Self::Error>;
}
