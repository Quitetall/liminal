//! Pandoc JSON AST adapter boundary (Phase -1.4 spike, M10; v4 §-1.4, §14,
//! §8.4, §107, §118A; Law 8).
//!
//! Pandoc is used strictly as a conversion ADAPTER for this measurement —
//! never the canonical graph (v4 §118A). This module holds:
//! - a minimal serde model of the Pandoc JSON AST (the D10.1 subset:
//!   `Para`, `Div`, `RawBlock` blocks; `Str`, `Space`, `SoftBreak` inlines);
//! - [`emit`]: toy paragraph blocks -> Pandoc AST (D10.1's mapping, with the
//!   foreign-node probe `RawBlock` appended; comment Relations are declared
//!   loss BY OMISSION — no code path here models them at all);
//! - [`reimport`]: a Pandoc JSON string -> the serde model;
//! - [`normalize`]: flattens a Pandoc AST's paragraph-shaped blocks into
//!   `(id, text)` pairs for measurement (the loss-report renderer itself is
//!   M10.4's job, not this module's).
//!
//! `run_pandoc` (the D10.2 CLI pipeline) and the D10.3 version pin land in
//! M10.3.

use serde::{Deserialize, Serialize};

use liminal_source::paragraph::Block as ParagraphBlock;

/// pandoc-types API version this milestone hardcodes when emitting AST JSON
/// (D10.2: pinned for pandoc-types 3.6.x, independent of whatever an
/// installed `pandoc` itself reports on its own `-t json` output — see
/// [`reimport`], which must tolerate that longer, real-world array).
pub const PANDOC_API_VERSION: [u32; 3] = [1, 23, 1];

/// The foreign-node probe payload (D10.1): a raw HTML comment that must
/// survive the full pipeline byte-identical.
pub const PROBE_RAW_HTML: &str = "<!--liminal:probe-->";

/// Pandoc's `Attr`: `(identifier, classes, key-value pairs)`.
pub type Attr = (String, Vec<String>, Vec<(String, String)>);

/// Minimal Pandoc JSON AST document (the top-level `Pandoc` value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pandoc {
    /// `pandoc-api-version`: an array of 3 (what this milestone emits, D10.2)
    /// or 4 components (what an installed `pandoc` itself reports).
    #[serde(rename = "pandoc-api-version")]
    pub pandoc_api_version: Vec<u32>,
    /// Document metadata (always empty for this milestone's fixtures).
    pub meta: serde_json::Map<String, serde_json::Value>,
    /// The document body.
    pub blocks: Vec<Block>,
}

/// Pandoc block-level AST nodes (the subset D10.1 needs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum Block {
    /// A plain paragraph (no Attr in the Pandoc AST — D10.1).
    Para(Vec<Inline>),
    /// An attributed container: `{#id}` paragraphs round-trip through this,
    /// Pandoc's only attributed block (D10.1).
    Div(Attr, Vec<Block>),
    /// A raw passthrough block in a named format (the foreign-node probe
    /// uses `"html"`).
    RawBlock(String, String),
}

/// Pandoc inline AST nodes (the subset D10.1 needs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub enum Inline {
    /// A run of non-whitespace text.
    Str(String),
    /// An inter-word space (D10.1: text is split on whitespace into
    /// alternating `Str`/`Space`).
    Space,
    /// A line break inside a paragraph that carries no semantic significance
    /// (Pandoc writer/reader artifact; flattens to a space like `Space`).
    SoftBreak,
}

impl Pandoc {
    /// An empty document carrying the pinned [`PANDOC_API_VERSION`].
    fn empty() -> Self {
        Pandoc {
            pandoc_api_version: PANDOC_API_VERSION.to_vec(),
            meta: serde_json::Map::new(),
            blocks: Vec::new(),
        }
    }
}

/// D10.1: split `text` on whitespace into alternating `Str`/`Space` inlines.
fn text_to_inlines(text: &str) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for (i, word) in text.split_whitespace().enumerate() {
        if i > 0 {
            inlines.push(Inline::Space);
        }
        inlines.push(Inline::Str(word.to_owned()));
    }
    inlines
}

/// Emit the D10.1 Pandoc AST for a parsed toy document: each paragraph block
/// becomes `Para` (no id) or `Div ("<id>",[],[]) [Para […]]` (with id).
/// Comment Relations are declared loss BY OMISSION (D10.1: "none — DECLARED
/// loss … dropped at emit") — this function never receives them, so there is
/// no code path to drop. The foreign-node probe is appended last as a raw
/// HTML `RawBlock`.
#[must_use]
pub fn emit(blocks: &[ParagraphBlock]) -> Pandoc {
    let mut doc = Pandoc::empty();
    for block in blocks {
        let para = Block::Para(text_to_inlines(&block.text));
        let emitted = if let Some(id) = &block.id {
            Block::Div((id.clone(), Vec::new(), Vec::new()), vec![para])
        } else {
            para
        };
        doc.blocks.push(emitted);
    }
    doc.blocks.push(Block::RawBlock(
        "html".to_owned(),
        PROBE_RAW_HTML.to_owned(),
    ));
    doc
}

/// Parse a Pandoc JSON AST string into the serde model.
///
/// # Errors
/// If `json` is not a valid Pandoc AST document in this module's subset.
pub fn reimport(json: &str) -> serde_json::Result<Pandoc> {
    serde_json::from_str(json)
}

/// Flatten one inline run back to text: `Str` contributes its text verbatim;
/// `Space`/`SoftBreak` contribute one ASCII space (D10.1: "reimport flattens
/// `Str`->text, `Space`/`SoftBreak`->` `").
fn flatten_inlines(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(s) => text.push_str(s),
            Inline::Space | Inline::SoftBreak => text.push(' '),
        }
    }
    text
}

/// Normalize a Pandoc AST's paragraph-shaped blocks (`Para`, `Div [Para]`)
/// into positional `(id, text)` pairs for measurement. `RawBlock`s (the
/// foreign-node probe) are excluded here — they are measured separately for
/// byte identity, not folded into text/id fidelity.
#[must_use]
pub fn normalize(ast: &Pandoc) -> Vec<(Option<String>, String)> {
    ast.blocks
        .iter()
        .filter_map(|block| match block {
            Block::Para(inlines) => Some((None, flatten_inlines(inlines))),
            Block::Div((id, _, _), children) => {
                let text = children
                    .iter()
                    .filter_map(|child| match child {
                        Block::Para(inlines) => Some(flatten_inlines(inlines)),
                        Block::Div(..) | Block::RawBlock(..) => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let id = if id.is_empty() {
                    None
                } else {
                    Some(id.clone())
                };
                Some((id, text))
            }
            Block::RawBlock(..) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> Pandoc {
        Pandoc {
            pandoc_api_version: PANDOC_API_VERSION.to_vec(),
            meta: serde_json::Map::new(),
            blocks: vec![
                Block::Para(vec![
                    Inline::Str("Hello".to_owned()),
                    Inline::Space,
                    Inline::Str("world.".to_owned()),
                ]),
                Block::Div(
                    ("myid".to_owned(), Vec::new(), Vec::new()),
                    vec![Block::Para(vec![
                        Inline::Str("Body".to_owned()),
                        Inline::SoftBreak,
                        Inline::Str("text.".to_owned()),
                    ])],
                ),
                Block::RawBlock("html".to_owned(), PROBE_RAW_HTML.to_owned()),
            ],
        }
    }

    /// The serde-model round-trip unit test (M10.2 exit): serialize, reparse,
    /// and the value comes back identical.
    #[test]
    fn pandoc_model_round_trips_through_json() {
        let doc = sample_doc();
        let json = serde_json::to_string(&doc).expect("serialize");
        let reparsed: Pandoc = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(doc, reparsed);
    }

    /// The wire shape matches REAL Pandoc JSON exactly (empirically captured
    /// from `pandoc 3.6.1 -f markdown -t json` on an equivalent document),
    /// not just self-consistent serde round-tripping.
    #[test]
    fn pandoc_model_matches_real_wire_shape() {
        let doc = Pandoc {
            pandoc_api_version: PANDOC_API_VERSION.to_vec(),
            meta: serde_json::Map::new(),
            blocks: vec![
                Block::Div(
                    ("myid".to_owned(), Vec::new(), Vec::new()),
                    vec![Block::Para(vec![
                        Inline::Str("Hello".to_owned()),
                        Inline::Space,
                        Inline::Str("world.".to_owned()),
                    ])],
                ),
                Block::RawBlock("html".to_owned(), PROBE_RAW_HTML.to_owned()),
            ],
        };
        let actual = serde_json::to_value(&doc).expect("serialize");
        let expected: serde_json::Value = serde_json::from_str(
            r#"{"pandoc-api-version":[1,23,1],"meta":{},"blocks":[
                {"t":"Div","c":[["myid",[],[]],[{"t":"Para","c":
                    [{"t":"Str","c":"Hello"},{"t":"Space"},{"t":"Str","c":"world."}]}]]},
                {"t":"RawBlock","c":["html","<!--liminal:probe-->"]}
            ]}"#,
        )
        .expect("expected json parses");
        assert_eq!(actual, expected);
    }

    /// `emit` maps id-bearing blocks to `Div` and anonymous ones to bare
    /// `Para`, and always appends the foreign-node probe last (D10.1).
    #[test]
    fn emit_maps_ids_to_div_and_appends_probe() {
        let blocks = vec![
            ParagraphBlock {
                text: "no id here".to_owned(),
                id: None,
                range: liminal_source::SourceRange { start: 0, end: 0 },
            },
            ParagraphBlock {
                text: "has an id".to_owned(),
                id: Some("p-one".to_owned()),
                range: liminal_source::SourceRange { start: 0, end: 0 },
            },
        ];
        let doc = emit(&blocks);
        assert_eq!(doc.pandoc_api_version, PANDOC_API_VERSION.to_vec());
        assert_eq!(doc.blocks.len(), 3);
        assert!(matches!(doc.blocks[0], Block::Para(_)));
        assert!(matches!(&doc.blocks[1], Block::Div(attr, _) if attr.0 == "p-one"));
        assert!(
            matches!(&doc.blocks[2], Block::RawBlock(fmt, raw) if fmt == "html" && raw == PROBE_RAW_HTML)
        );
    }

    /// `normalize` recovers positional `(id, text)` pairs, dropping the probe
    /// `RawBlock` entirely.
    #[test]
    fn normalize_recovers_positional_id_text_pairs() {
        let doc = sample_doc();
        let pairs = normalize(&doc);
        assert_eq!(
            pairs,
            vec![
                (None, "Hello world.".to_owned()),
                (Some("myid".to_owned()), "Body text.".to_owned()),
            ]
        );
    }
}
