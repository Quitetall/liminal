//! Bounded rowan-backed lossless parser (v4 §9, §22).

use liminal_source::{SourceRange, Utf8HolderView};
use rowan::{GreenNode, GreenNodeBuilder, Language, SyntaxKind as RawKind};
use std::ops::Range;

/// Maximum nesting depth accepted before a typed error is emitted.
pub const MAX_NESTING: u16 = 256;
/// Maximum retained syntax errors per parse.
pub const MAX_ERRORS: usize = 1024;

/// Marker type for rowan's typed syntax tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LiminalLanguage {}

/// Stable syntax kinds for the bounded Markdown-compatible frontend.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntaxKind {
    /// Document root.
    Root,
    /// ATX heading.
    Heading,
    /// Paragraph block.
    Paragraph,
    /// Ordered list.
    OrderedList,
    /// Unordered list.
    UnorderedList,
    /// List item.
    ListItem,
    /// Block quote.
    BlockQuote,
    /// Fenced code.
    Fence,
    /// Inline construct.
    Inline,
    /// Link/reference.
    Reference,
    /// Explicit ID marker.
    ExplicitId,
    /// Ordinary text.
    Text,
    /// Whitespace.
    Whitespace,
    /// Comment.
    Comment,
    /// Delimiter.
    Delimiter,
    /// Newline.
    Newline,
    /// Opaque unsupported content.
    Opaque,
    /// Error-recovery content.
    Error,
}

impl LiminalLanguage {
    fn to_raw(kind: SyntaxKind) -> RawKind {
        RawKind(kind as u16)
    }

    fn from_raw(raw: RawKind) -> SyntaxKind {
        match raw.0 {
            0 => SyntaxKind::Root,
            1 => SyntaxKind::Heading,
            2 => SyntaxKind::Paragraph,
            3 => SyntaxKind::OrderedList,
            4 => SyntaxKind::UnorderedList,
            5 => SyntaxKind::ListItem,
            6 => SyntaxKind::BlockQuote,
            7 => SyntaxKind::Fence,
            8 => SyntaxKind::Inline,
            9 => SyntaxKind::Reference,
            10 => SyntaxKind::ExplicitId,
            11 => SyntaxKind::Text,
            12 => SyntaxKind::Whitespace,
            13 => SyntaxKind::Comment,
            14 => SyntaxKind::Delimiter,
            15 => SyntaxKind::Newline,
            16 => SyntaxKind::Opaque,
            _ => SyntaxKind::Error,
        }
    }
}

impl Language for LiminalLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: RawKind) -> Self::Kind {
        Self::from_raw(raw)
    }

    fn kind_to_raw(kind: Self::Kind) -> RawKind {
        Self::to_raw(kind)
    }
}

/// Typed parser diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorCode {
    /// Nesting bound exceeded.
    NestingLimit,
    /// Error retention bound exceeded.
    ErrorLimit,
    /// Delimiter never closed.
    UnclosedDelimiter,
    /// Explicit ID is malformed.
    InvalidAttribute,
    /// Link/reference is malformed.
    InvalidReference,
    /// Construct is outside bounded frontend.
    UnsupportedConstruct,
}

/// One stable parse diagnostic and exact source range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Diagnostic code.
    pub code: ParseErrorCode,
    /// Exact UTF-8 byte range.
    pub range: SourceRange,
}

/// Lossless parse result tied to one exact source basis.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<ParseError>,
    basis: liminal_source::SourceBasis,
}

/// Typed rowan syntax node alias.
pub type SyntaxNode = rowan::SyntaxNode<LiminalLanguage>;

/// Parse one validated Holder view without rejecting malformed syntax.
#[must_use]
pub fn parse(source: &Utf8HolderView) -> Parse {
    let text = source.to_string();
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(LiminalLanguage::to_raw(SyntaxKind::Root));
    if !text.is_empty() {
        builder.token(LiminalLanguage::to_raw(SyntaxKind::Text), &text);
    }
    builder.finish_node();
    let errors = diagnose(&text);
    Parse {
        green: builder.finish(),
        errors,
        basis: source.basis().clone(),
    }
}

impl Parse {
    /// Return typed root syntax node.
    #[must_use]
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Stable diagnostics in source order.
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Exact source basis used for this parse.
    #[must_use]
    pub fn basis(&self) -> &liminal_source::SourceBasis {
        &self.basis
    }

    /// Concatenate rowan token text without normalization or synthesis.
    #[must_use]
    pub fn emit_lossless(&self) -> String {
        self.syntax()
            .descendants_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .map(|token| token.text().to_owned())
            .collect()
    }
}

fn diagnose(source: &str) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let mut delimiter_depth = 0u16;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        if let Some(marker) = line.find("{#") {
            let suffix = &line[marker + 2..];
            let valid_close = suffix.find('}').is_some_and(|close| {
                suffix[close + 1..]
                    .trim_matches(['\n', '\r', ' ', '\t'])
                    .is_empty()
            });
            if !valid_close {
                push_error(
                    &mut errors,
                    ParseErrorCode::UnclosedDelimiter,
                    offset + marker,
                    offset + line.trim_end_matches(['\r', '\n']).len(),
                );
            }
        }
        let marker_range = line.find("{#").map(|marker| {
            let suffix = &line[marker + 2..];
            suffix
                .find('}')
                .map_or(marker..line.len(), |close| marker..marker + 2 + close + 1)
        });
        let mut nesting_reported = false;
        for (byte, ch) in line.char_indices() {
            if marker_range
                .as_ref()
                .is_some_and(|range: &Range<usize>| range.contains(&byte))
            {
                continue;
            }
            if matches!(ch, '(' | '[' | '{') {
                delimiter_depth = delimiter_depth.saturating_add(1);
                if delimiter_depth > MAX_NESTING && !nesting_reported {
                    push_error(
                        &mut errors,
                        ParseErrorCode::NestingLimit,
                        offset,
                        offset + line.len(),
                    );
                    nesting_reported = true;
                }
            } else if matches!(ch, ')' | ']' | '}') {
                delimiter_depth = delimiter_depth.saturating_sub(1);
            }
        }
        offset += line.len();
    }
    errors.sort_by_key(|error| (error.range.start, error.range.end));
    errors
}

fn push_error(errors: &mut Vec<ParseError>, code: ParseErrorCode, start: usize, end: usize) {
    if errors.len() < MAX_ERRORS {
        errors.push(ParseError {
            code,
            range: SourceRange {
                start: u64::try_from(start).expect("usize fits u64"),
                end: u64::try_from(end).expect("usize fits u64"),
            },
        });
    } else if !errors
        .iter()
        .any(|error| error.code == ParseErrorCode::ErrorLimit)
    {
        errors.push(ParseError {
            code: ParseErrorCode::ErrorLimit,
            range: SourceRange {
                start: u64::try_from(start).expect("usize fits u64"),
                end: u64::try_from(end).expect("usize fits u64"),
            },
        });
    }
}
