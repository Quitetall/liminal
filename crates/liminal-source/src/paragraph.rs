//! The ~50-line toy paragraph grammar (the ENTIRE Phase -1 grammar).
//!
//! Blank-line-separated blocks with optional `{#id}` markers. Parse is TOTAL —
//! never errors (Law 3B: capture is never rejected). Malformed markers stay
//! literal.

use crate::range::SourceRange;

/// One parsed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Block text (marker stripped from the last line if well-formed).
    pub text: String,
    /// Durable id, if the `{#id}` marker was well-formed.
    pub id: Option<String>,
    /// Byte span of the block in the input (inclusive/exclusive), including
    /// marker bytes, excluding surrounding blank lines.
    pub range: SourceRange,
}

/// Parse input into blocks. TOTAL — never errors (Law 3B).
///
/// Grammar:
/// - A block is a maximal run of non-blank lines.
/// - Blank lines (empty or whitespace-only) separate blocks.
/// - Leading/trailing blank lines produce no blocks.
/// - ID marker: last line ends with `{#id}` where id matches `[A-Za-z0-9_-]+`,
///   optionally preceded by spaces/tabs. Only the LAST suffix is a marker.
/// - Malformed markers (unclosed, empty id, invalid chars, not at end-of-block)
///   → `id = None`, text unchanged.
pub fn parse(input: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let lines: Vec<(usize, &str)> = input.lines().enumerate().collect();

    let mut block_start_line: Option<usize> = None;
    let mut block_lines: Vec<(usize, &str)> = Vec::new();

    for (i, line) in &lines {
        let is_blank = line.trim().is_empty();

        if is_blank {
            if let Some(start) = block_start_line {
                // End of block.
                let block = build_block(input, start, &block_lines);
                blocks.push(block);
                block_start_line = None;
                block_lines.clear();
            }
        } else {
            if block_start_line.is_none() {
                block_start_line = Some(*i);
            }
            block_lines.push((*i, line));
        }
    }

    // Trailing block (no trailing blank line).
    if let Some(start) = block_start_line {
        let block = build_block(input, start, &block_lines);
        blocks.push(block);
    }

    blocks
}

/// Build one Block from collected lines.
fn build_block(input: &str, start_line: usize, lines: &[(usize, &str)]) -> Block {
    assert!(!lines.is_empty(), "block must have at least one line");

    // Try to extract an ID marker from the last line.
    let last_line = lines.last().unwrap().1;
    let (id, last_line_cleaned) = extract_marker(last_line);

    // Build text: all lines joined by \n, with the last line cleaned.
    let mut text_lines: Vec<&str> = lines.iter().map(|(_, l)| *l).collect();
    if id.is_some() {
        // Replace last line with cleaned version.
        let last_idx = text_lines.len() - 1;
        let cleaned = last_line_cleaned.as_deref().unwrap_or(last_line);
        if cleaned.is_empty() {
            text_lines.pop();
        } else {
            text_lines[last_idx] = cleaned;
        }
    }
    let text = text_lines.join("\n");

    // Compute byte range. The range includes the original marker bytes and
    // excludes surrounding blank lines.
    let first_line_start = input
        .lines()
        .nth(start_line)
        .map_or(0, |_| byte_offset(input, start_line));
    let last_line_end = lines
        .last()
        .and_then(|(i, _)| {
            let line_start = byte_offset(input, *i);
            input.lines().nth(*i).map(|l| line_start + l.len())
        })
        .unwrap_or(input.len());

    Block {
        text,
        id,
        range: SourceRange {
            start: first_line_start as u64,
            end: last_line_end as u64,
        },
    }
}

/// Extract an ID marker from the last line of a block.
///
/// Returns `(Some(id), Some(cleaned_line))` if the marker is well-formed,
/// `(None, None)` if malformed (text unchanged).
fn extract_marker(line: &str) -> (Option<String>, Option<String>) {
    // Find the last `{#` in the line.
    let Some(marker_start) = line.rfind("{#") else {
        return (None, None);
    };

    // The marker must end with `}`.
    let after_hash = &line[marker_start + 2..];
    let Some(close) = after_hash.find('}') else {
        return (None, None); // unclosed
    };

    let id_str = &after_hash[..close];

    // ID must be nonempty and match `[A-Za-z0-9_-]+`.
    if id_str.is_empty()
        || !id_str
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return (None, None);
    }

    // The `}` must be at end-of-line (possibly with trailing whitespace).
    let after_close = &after_hash[close + 1..];
    if !after_close.trim().is_empty() {
        return (None, None);
    }

    // The marker must be preceded only by spaces/tabs (the marker occupies
    // the end of the line; text before it on the same line is preserved).
    let before_marker = &line[..marker_start];
    let cleaned = before_marker.trim_end().to_owned();

    (Some(id_str.to_owned()), Some(cleaned))
}

/// Byte offset of line `n` in `input`.
fn byte_offset(input: &str, n: usize) -> usize {
    input
        .char_indices()
        .scan(0, |line_count, (pos, ch)| {
            if *line_count == n {
                return Some(Some(pos));
            }
            if ch == '\n' {
                *line_count += 1;
            }
            Some(None)
        })
        .flatten()
        .next()
        .unwrap_or(input.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_block_no_id() {
        let blocks = parse("hello world");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "hello world");
        assert_eq!(blocks[0].id, None);
    }

    #[test]
    fn two_blocks_separated_by_blank() {
        let blocks = parse("first\n\nsecond");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "first");
        assert_eq!(blocks[1].text, "second");
    }

    #[test]
    fn id_marker_on_last_line() {
        let blocks = parse("some text\nmore text {#my-id}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, Some("my-id".to_owned()));
        assert_eq!(blocks[0].text, "some text\nmore text");
    }

    #[test]
    fn id_marker_with_leading_whitespace() {
        let blocks = parse("text\n  \t{#anchor}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, Some("anchor".to_owned()));
        assert_eq!(blocks[0].text, "text");
    }

    #[test]
    fn malformed_unclosed_marker_stays_literal() {
        let blocks = parse("text {#unclosed");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, None);
        assert_eq!(blocks[0].text, "text {#unclosed");
    }

    #[test]
    fn malformed_empty_id_stays_literal() {
        let blocks = parse("text {#}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, None);
    }

    #[test]
    fn malformed_invalid_chars_stays_literal() {
        let blocks = parse("text {#bad id!}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, None);
    }

    #[test]
    fn multiple_markers_only_last_is_id() {
        let blocks = parse("foo {#a} {#b}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, Some("b".to_owned()));
        assert_eq!(blocks[0].text, "foo {#a}");
    }

    #[test]
    fn leading_trailing_blanks_produce_no_blocks() {
        let blocks = parse("\n\n  \nhello\n\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "hello");
    }

    #[test]
    fn empty_input_produces_no_blocks() {
        assert!(parse("").is_empty());
    }

    #[test]
    fn range_covers_block_bytes() {
        let input = "aaa\n\nbbb {#x}";
        let blocks = parse(input);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].range.start, 0);
        assert_eq!(blocks[0].range.end, 3);
        assert_eq!(blocks[1].range.start, 5);
        assert_eq!(blocks[1].range.end, input.len() as u64);
    }

    #[test]
    fn sound_session_fixture_parses() {
        let input = "\
The Fourier transform decomposes a signal
into its constituent frequencies. {#p-fourier}

The Laplace transform generalizes it
to the complex plane. {#p-laplace}
";
        let blocks = parse(input);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, Some("p-fourier".to_owned()));
        assert_eq!(blocks[1].id, Some("p-laplace".to_owned()));
        assert!(blocks[0].text.contains("constituent frequencies."));
        assert!(!blocks[0].text.contains("{#"));
    }
}
