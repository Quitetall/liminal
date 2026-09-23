//! Read-only coordinate proposals. A proposal is not authorization or review.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use quote::ToTokens;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::process::Command;
use syn::{spanned::Spanned, visit::Visit};

/// Untrusted evidence for subsequent authenticated review and batch admission.
#[derive(Debug, Serialize)]
pub struct CoordinateProposal {
    schema_version: u32,
    status: &'static str,
    authority: &'static str,
    independent_review: &'static str,
    base_commit: String,
    candidate_commit: String,
    pub(crate) source: String,
    pub(crate) old_line: usize,
    pub(crate) new_line: usize,
    anchor: String,
    enclosing_symbol: String,
    enclosing_sha256: String,
    old_source_sha256: String,
    new_source_sha256: String,
}

/// Supports only blank-line relocation of a whole-line expression in a top-level
/// function. Other shapes are refused, not inferred equivalent. This does not
/// establish registry membership, mutation behavior, review, or apply eligibility.
pub fn propose_coordinate(
    root: &Utf8Path,
    base: &str,
    candidate: &str,
    source: &Utf8Path,
    line: usize,
    anchor: &str,
) -> Result<CoordinateProposal> {
    ensure!(
        source.extension() == Some("rs") && !source.is_absolute(),
        "expected relative Rust source path"
    );
    ensure!(
        source
            .components()
            .all(|c| matches!(c, camino::Utf8Component::Normal(_))),
        "unsafe source path"
    );
    ensure!(
        !source.as_str().contains("conformance/corpora/heldout"),
        "held-out source refused"
    );
    ensure!(
        line > 0 && !anchor.trim().is_empty() && !anchor.contains(['\n', '\r']),
        "invalid exact line anchor"
    );
    let old = committed_source(root, base, source)?;
    let new = committed_source(root, candidate, source)?;
    let old_lines: Vec<_> = old.lines().collect();
    let new_lines: Vec<_> = new.lines().collect();
    ensure!(
        old_lines.get(line - 1).copied() == Some(anchor),
        "old coordinate does not match exact anchor"
    );
    let locations = |lines: &[&str]| {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, text)| (*text == anchor).then_some(i + 1))
            .collect::<Vec<_>>()
    };
    ensure!(locations(&old_lines).len() == 1, "ambiguous old target");
    let targets = locations(&new_lines);
    ensure!(targets.len() == 1, "missing or ambiguous candidate target");
    let new_line = targets[0];
    let old_context = enclosing_function(&old, line, anchor)?;
    let new_context = enclosing_function(&new, new_line, anchor)?;
    ensure!(
        old_context == new_context,
        "enclosing implementation changed"
    );
    // Reject changed nonblank lines elsewhere in this source. This textual
    // screen is not equivalence: blank lines can be literal data elsewhere.
    // Independent target/mutation review and admission remain unimplemented.
    ensure!(
        old_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .eq(new_lines.iter().filter(|l| !l.trim().is_empty())),
        "unsupported nonblank source change"
    );
    ensure!(
        token_stream(&old)? == token_stream(&new)?,
        "source token stream changed"
    );
    Ok(CoordinateProposal {
        schema_version: 1,
        status: "proposal-only",
        authority: "none",
        independent_review: "not-established",
        base_commit: base.to_owned(),
        candidate_commit: candidate.to_owned(),
        source: source.to_string(),
        old_line: line,
        new_line,
        anchor: anchor.to_owned(),
        enclosing_symbol: old_context.0,
        enclosing_sha256: digest(old_context.1.as_bytes()),
        old_source_sha256: digest(old.as_bytes()),
        new_source_sha256: digest(new.as_bytes()),
    })
}

/// Compare parsed Rust tokens after allowing formatting-only whitespace and
/// ordinary comments (which `syn` intentionally discards). Raw-string
/// contents, literals, attributes and macro tokens remain part of the stream,
/// so blank-line edits inside source data cannot masquerade as coordinate
/// moves. Pinned `syn`, `quote` and `proc-macro2` versions define this contract.
fn token_stream(text: &str) -> Result<String> {
    let syntax = syn::parse_file(text).context("unsupported Rust syntax")?;
    let mut tokens = proc_macro2::TokenStream::new();
    syntax.to_tokens(&mut tokens);
    Ok(tokens.to_string())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn committed_source(root: &Utf8Path, revision: &str, source: &Utf8Path) -> Result<String> {
    ensure!(
        revision.len() == 40
            && revision
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "full lowercase commit ID required"
    );
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let mut command = Command::new("git");
        // Repository selection and object lookup must not inherit caller Git
        // overrides. Do not inspect or log environment values.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        let output = command
            .current_dir(root)
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .args(args)
            .output()
            .context("read committed source")?;
        ensure!(output.status.success(), "committed source lookup failed");
        Ok(output.stdout)
    };
    ensure!(
        git(&["cat-file", "-t", revision])? == b"commit\n",
        "revision is not a commit"
    );
    let spec = format!("{revision}:{source}");
    // Reject symlink blobs instead of interpreting their target as source code.
    let entry = git(&["ls-tree", revision, "--", source.as_str()])?;
    ensure!(
        entry.starts_with(b"100644 blob ") || entry.starts_with(b"100755 blob "),
        "source must be a regular committed file"
    );
    String::from_utf8(git(&["cat-file", "blob", &spec])?).context("source is not UTF-8")
}

fn enclosing_function(text: &str, line: usize, anchor: &str) -> Result<(String, String)> {
    let syntax = syn::parse_file(text).context("unsupported Rust syntax")?;
    let mut matches = Vec::new();
    for item in &syntax.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        let span = function.span();
        if span.start().line <= line && span.end().line >= line {
            let mut expressions = ExactExpression {
                line,
                start: anchor.chars().take_while(|c| c.is_whitespace()).count(),
                end: anchor.trim_end().chars().count(),
                count: 0,
            };
            expressions.visit_block(&function.block);
            ensure!(
                expressions.count == 1,
                "anchor must identify one whole-line expression"
            );
            let lines: Vec<_> = text.split_inclusive('\n').collect();
            // Retain complete lines, including attributes, comments and whitespace.
            let context = lines[span.start().line - 1..span.end().line].concat();
            matches.push((function.sig.ident.to_string(), context));
        }
    }
    ensure!(
        matches.len() == 1,
        "target is not in one supported top-level function"
    );
    Ok(matches.remove(0))
}

struct ExactExpression {
    line: usize,
    start: usize,
    end: usize,
    count: usize,
}

impl<'ast> Visit<'ast> for ExactExpression {
    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        let span = expression.span();
        if span.start().line == self.line
            && span.end().line == self.line
            && span.start().column == self.start
            && span.end().column == self.end
        {
            self.count += 1;
        }
        syn::visit::visit_expr(self, expression);
    }
}
