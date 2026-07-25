//! Deterministic Phase 1 source formatter and the legacy paragraph renderer.
#![allow(missing_docs)]
#![allow(
    clippy::format_push_string,
    clippy::semicolon_if_nothing_returned,
    clippy::unnecessary_wraps
)]

use std::collections::BTreeMap;

use liminal_cir::{
    DerivedGraph, ResolveDiagnostic, ResolveError, ResolvedGraph, basis_for_source, resolve,
};
use liminal_hir::{
    HirDiagnostic, HirDocument, HirError, HirItem, HirItemKind, HirReference, HirValue, LoweredHir,
    SourceDialect, lower,
};
use liminal_id::{ContentHash, JurisdictionKey, PathId, SourceId, TransactionId};
use liminal_revision::{BasisComponent, BasisPerspective, WorkspaceBasis};
use liminal_source::{
    SourceBasis, SourceLoadError,
    paragraph::{self, Block},
};

/// Parsed legacy paragraph projection used by [`MarkdownRenderer`].
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub blocks: Vec<Block>,
}
impl PartialEq for MarkdownDocument {
    fn eq(&self, other: &Self) -> bool {
        self.blocks
            .iter()
            .map(|b| (&b.text, &b.id))
            .eq(other.blocks.iter().map(|b| (&b.text, &b.id)))
    }
}
impl Eq for MarkdownDocument {}

/// Legacy renderer error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownError;
impl std::fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("markdown formatting error")
    }
}
impl std::error::Error for MarkdownError {}

/// Non-fatal Phase 1 diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase1Diagnostic {
    Hir(HirDiagnostic),
    Resolve(ResolveDiagnostic),
}

/// Fatal Phase 1 formatting error.
#[derive(Debug, thiserror::Error)]
pub enum Phase1FormatError {
    #[error(transparent)]
    Source(#[from] SourceLoadError),
    #[error(transparent)]
    Hir(#[from] HirError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error("canonical emission failed: {0}")]
    Emit(String),
}

/// Parsed, source-mapped HIR plus its immutable derived graph.
#[derive(Debug, Clone)]
pub struct Phase1Document {
    pub holder: JurisdictionKey,
    pub hir: HirDocument,
    pub graph: DerivedGraph,
    pub diagnostics: Vec<Phase1Diagnostic>,
}

impl PartialEq for Phase1Document {
    fn eq(&self, other: &Self) -> bool {
        self.holder == other.holder
            && basis_semantic_eq(&self.graph.basis, &other.graph.basis, &self.holder)
            && self.graph.nodes == other.graph.nodes
            && self.graph.relations == other.graph.relations
    }
}
impl Eq for Phase1Document {}

fn basis_semantic_eq(a: &WorkspaceBasis, b: &WorkspaceBasis, holder: &JurisdictionKey) -> bool {
    if a.transaction != b.transaction
        || a.perspective != b.perspective
        || a.components.len() != b.components.len()
    {
        return false;
    }
    a.components
        .iter()
        .all(|(key, av)| match (key, b.components.get(key)) {
            (key, Some(bv)) if key == holder => component_address_eq(av, bv),
            (_, Some(bv)) => av == bv,
            _ => false,
        })
}
fn component_address_eq(a: &BasisComponent, b: &BasisComponent) -> bool {
    match (a, b) {
        (
            BasisComponent::FileContent { path: ap, .. },
            BasisComponent::FileContent { path: bp, .. },
        ) => ap == bp,
        (
            BasisComponent::BufferGeneration {
                client: ac,
                buffer: ab,
                epoch: ae,
                generation: ag,
                base_file_hash: ah,
                ..
            },
            BasisComponent::BufferGeneration {
                client: bc,
                buffer: bb,
                epoch: be,
                generation: bg,
                base_file_hash: bh,
                ..
            },
        ) => (ac, ab, ae, ag, ah) == (bc, bb, be, bg, bh),
        _ => a == b,
    }
}

/// Phase 1 formatter. Parsing is always pinned to the formatter's source,
/// Holder, and transaction Basis; emission is canonical explicit syntax.
#[derive(Debug, Clone)]
pub struct MarkdownFormatter {
    source: SourceId,
    holder: JurisdictionKey,
    basis: WorkspaceBasis,
    dialect: SourceDialect,
}

impl MarkdownFormatter {
    #[must_use]
    pub fn new(
        source: SourceId,
        holder: JurisdictionKey,
        basis: WorkspaceBasis,
        dialect: SourceDialect,
    ) -> Self {
        Self {
            source,
            holder,
            basis,
            dialect,
        }
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        let source = SourceId::from_name("liminal:phase1:default");
        let holder = JurisdictionKey::Path(PathId("phase1.md".into()));
        let basis = WorkspaceBasis {
            transaction: TransactionId::new(),
            perspective: BasisPerspective::DurableOnly,
            components: BTreeMap::from([(
                holder.clone(),
                BasisComponent::FileContent {
                    path: PathId("phase1.md".into()),
                    hash: ContentHash::of(b""),
                },
            )]),
        };
        Self::new(source, holder, basis, SourceDialect::CompactOrExplicitV1)
    }
}

impl Formatter for MarkdownFormatter {
    type Doc = Phase1Document;
    type Error = Phase1FormatError;

    fn parse(&self, source: &str) -> Result<Self::Doc, Self::Error> {
        let hash = ContentHash::of(source.as_bytes());
        let source_basis = SourceBasis {
            source: self.source,
            content_hash: hash,
        };
        let view = liminal_source::Utf8HolderView::from_bytes(source_basis, source.as_bytes())?;
        let cst = liminal_cst::parse(&view);
        let LoweredHir {
            document: hir,
            diagnostics: hir_diagnostics,
        } = lower(&cst, self.dialect)?;
        let transient = basis_for_source(&self.basis, &self.holder, hash)?;
        let ResolvedGraph {
            graph,
            diagnostics: resolve_diagnostics,
        } = resolve(&hir, &transient)?;
        let mut diagnostics = hir_diagnostics
            .into_iter()
            .map(Phase1Diagnostic::Hir)
            .collect::<Vec<_>>();
        diagnostics.extend(
            resolve_diagnostics
                .into_iter()
                .map(Phase1Diagnostic::Resolve),
        );
        Ok(Phase1Document {
            holder: self.holder.clone(),
            hir,
            graph,
            diagnostics,
        })
    }

    fn emit(&self, doc: &Self::Doc) -> Result<String, Self::Error> {
        let mut out = String::from("#!liminal-explicit-v1\n");
        let items = doc
            .hir
            .items
            .iter()
            .map(|item| (item.id, item))
            .collect::<BTreeMap<_, _>>();
        for root in &doc.hir.roots {
            emit_item(
                &mut out,
                items
                    .get(root)
                    .ok_or_else(|| Phase1FormatError::Emit(format!("dangling root {root:?}")))?,
                &items,
                0,
            )?;
        }
        Ok(out)
    }

    fn format(&self, source: &str) -> Result<String, Self::Error> {
        self.emit(&self.parse(source)?)
    }

    /// AM-17.3: emit the compact surface when the caller asks for it AND the
    /// document is compact-representable; otherwise fall back to the explicit
    /// surface, which can express every form losslessly.
    fn emit_in_dialect(
        &self,
        doc: &Self::Doc,
        dialect: SourceDialect,
    ) -> Result<String, Self::Error> {
        match dialect {
            SourceDialect::ExplicitV1 => self.emit(doc),
            SourceDialect::CompactOrExplicitV1 => {
                emit_compact(&doc.hir).map_or_else(|| self.emit(doc), Ok)
            }
        }
    }
}

/// Render `hir` in the compact surface, or `None` when any root is not
/// compact-representable.
///
/// The compact surface expresses exactly one shape: a `paragraph` node whose
/// only child is a literal, carrying at most an `id` attribute — i.e. what
/// `alpha {#a}` parses to. Anything richer (relations, nested nodes, ordered
/// blocks, other attributes) has no compact spelling, so the whole document
/// falls back rather than emitting a partial or lossy rendering.
fn emit_compact(hir: &HirDocument) -> Option<String> {
    let items: BTreeMap<liminal_hir::HirId, &HirItem> =
        hir.items.iter().map(|item| (item.id, item)).collect();

    let mut blocks = Vec::with_capacity(hir.roots.len());
    for root in &hir.roots {
        let item = items.get(root)?;
        let HirItemKind::NodeConstruction { name } = &item.kind else {
            return None;
        };
        if name != "paragraph" || item.children.len() != 1 {
            return None;
        }
        let child = items.get(&item.children[0])?;
        let HirItemKind::Literal { value } = &child.kind else {
            return None;
        };
        if !child.attributes.is_empty() || !child.children.is_empty() {
            return None;
        }

        let id = match item.attributes.len() {
            0 => None,
            1 => match item.attributes.get("id") {
                Some(HirValue::String(id)) => Some(id.clone()),
                _ => return None,
            },
            _ => return None,
        };

        blocks.push(match id {
            Some(id) => format!("{value} {{#{id}}}"),
            None => value.clone(),
        });
    }

    Some(format!("{}\n", blocks.join("\n\n")))
}

fn emit_item(
    out: &mut String,
    item: &HirItem,
    items: &BTreeMap<liminal_hir::HirId, &HirItem>,
    depth: usize,
) -> Result<(), Phase1FormatError> {
    let indent = "  ".repeat(depth);
    match &item.kind {
        HirItemKind::Literal { value } => {
            out.push_str(&format!("{indent}literal {};\n", json_string(value)))
        }
        HirItemKind::NodeConstruction { name } => {
            out.push_str(&format!("{indent}node {name}"));
            emit_attrs(out, &item.attributes)?;
            if item.children.is_empty() {
                out.push_str(";\n");
            } else {
                out.push_str(" {\n");
                for child in &item.children {
                    emit_item(
                        out,
                        items.get(child).ok_or_else(|| {
                            Phase1FormatError::Emit(format!("dangling child {child:?}"))
                        })?,
                        items,
                        depth + 1,
                    )?;
                }
                out.push_str(&format!("{indent}}}\n"));
            }
        }
        HirItemKind::RelationConstruction {
            source,
            kind,
            target,
        } => {
            out.push_str(&format!(
                "{indent}relation {} -[{kind}]-> {}",
                ref_text(source),
                ref_text(target)
            ));
            emit_attrs(out, &item.attributes)?;
            out.push_str(";\n");
        }
        HirItemKind::AttributeAssignment { name, value, .. } => out.push_str(&format!(
            "{indent}attribute {name} = {};\n",
            value_text(value)
        )),
        HirItemKind::OrderedBlock => {
            out.push_str(&format!("{indent}ordered"));
            if item.children.is_empty() {
                out.push_str(";\n");
            } else {
                out.push_str(" {\n");
                for child in &item.children {
                    emit_item(
                        out,
                        items.get(child).ok_or_else(|| {
                            Phase1FormatError::Emit(format!("dangling child {child:?}"))
                        })?,
                        items,
                        depth + 1,
                    )?;
                }
                out.push_str(&format!("{indent}}}\n"));
            }
        }
        HirItemKind::Reference { target } => {
            out.push_str(&format!("{indent}reference {};\n", ref_text(target)))
        }
        HirItemKind::Expression { source } => {
            out.push_str(&format!("{indent}expression {};\n", json_string(source)))
        }
        HirItemKind::MacroInvocation { name, arguments } => out.push_str(&format!(
            "{indent}macro {name}({});\n",
            arguments
                .iter()
                .map(value_text)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
    Ok(())
}
fn emit_attrs(
    out: &mut String,
    attrs: &BTreeMap<String, HirValue>,
) -> Result<(), Phase1FormatError> {
    if !attrs.is_empty() {
        out.push_str(" (");
        out.push_str(
            &attrs
                .iter()
                .map(|(k, v)| format!("{k} = {}", value_text(v)))
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push(')');
    }
    Ok(())
}
fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}
fn ref_text(value: &HirReference) -> String {
    match value {
        HirReference::Resolved(id) => format!("@hir{}", id.0),
        HirReference::Unresolved(name) => format!("@{name}"),
    }
}
fn value_text(value: &HirValue) -> String {
    match value {
        HirValue::Null => "null".into(),
        HirValue::Bool(v) => v.to_string(),
        HirValue::Integer(v) => v.to_string(),
        HirValue::String(v) => json_string(v),
        HirValue::Reference(r) => ref_text(r),
        HirValue::List(values) => format!(
            "[{}]",
            values.iter().map(value_text).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Deterministic paragraph-compatible renderer kept for existing HTML tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownRenderer;
impl MarkdownRenderer {
    pub fn render(&self, source: &str) -> Result<String, MarkdownError> {
        self.render_blocks(&paragraph::parse(source))
    }
    pub fn render_blocks(&self, blocks: &[Block]) -> Result<String, MarkdownError> {
        let mut out = String::from("<article>\n");
        for block in blocks {
            let id = block
                .id
                .as_deref()
                .map(|v| format!(" data-node-id=\"{}\"", escape_html(v)))
                .unwrap_or_default();
            let text = escape_html(&block.text).replace('\n', "<br />\n");
            out.push_str(&format!("<p{id}>{text}</p>\n"));
        }
        out.push_str("</article>\n");
        Ok(out)
    }
}
fn escape_html(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&#39;".into(),
            _ => ch.to_string(),
        })
        .collect()
}

/// Generic formatter seam.
pub trait Formatter {
    type Doc;
    type Error;
    fn parse(&self, source: &str) -> Result<Self::Doc, Self::Error>;
    fn emit(&self, doc: &Self::Doc) -> Result<String, Self::Error>;
    fn format(&self, source: &str) -> Result<String, Self::Error>;

    /// Emit `doc` in a REQUESTED surface dialect (AM-17.3, additive).
    ///
    /// [`emit`](Formatter::emit) is the canonical projection and always
    /// produces the explicit surface; this is the dialect-aware counterpart
    /// M19's "canonical round-trip across compact and explicit" and M20's
    /// `lim fmt` need — a formatter that rewrote a reader's compact document
    /// into `#!liminal-explicit-v1` on every save would destroy the surface
    /// they chose.
    ///
    /// The default implementation delegates to `emit`, so this addition
    /// cannot change the behavior of any existing implementor. An
    /// implementation that cannot express `doc` in `dialect` MUST fall back
    /// to the canonical surface rather than emit a lossy approximation.
    ///
    /// # Errors
    /// Whatever the underlying emission reports.
    fn emit_in_dialect(
        &self,
        doc: &Self::Doc,
        dialect: SourceDialect,
    ) -> Result<String, Self::Error> {
        let _ = dialect;
        self.emit(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_surface_covers_all_eight_forms() {
        let source = concat!(
            "#!liminal-explicit-v1\n",
            "node root (id = \"root\") {\n",
            "  literal \"value\";\n",
            "  attribute color = \"blue\";\n",
            "  reference @root;\n",
            "  expression \"x + 1\";\n",
            "  macro expand(1, true);\n",
            "}\n",
            "relation @root -[links]-> @root;\n",
            "ordered {\n  literal \"tail\";\n}\n",
        );
        let document = MarkdownFormatter::default()
            .parse(source)
            .expect("explicit source parses");
        let forms = document
            .hir
            .items
            .iter()
            .map(|item| match item.kind {
                HirItemKind::Literal { .. } => 1,
                HirItemKind::NodeConstruction { .. } => 2,
                HirItemKind::RelationConstruction { .. } => 3,
                HirItemKind::AttributeAssignment { .. } => 4,
                HirItemKind::OrderedBlock => 5,
                HirItemKind::Reference { .. } => 6,
                HirItemKind::Expression { .. } => 7,
                HirItemKind::MacroInvocation { .. } => 8,
            })
            .collect::<Vec<_>>();
        assert!(
            forms.contains(&1) && forms.contains(&2) && forms.contains(&3) && forms.contains(&4)
        );
        assert!(
            forms.contains(&5) && forms.contains(&6) && forms.contains(&7) && forms.contains(&8)
        );
        let macro_item = document
            .hir
            .items
            .iter()
            .find_map(|item| match &item.kind {
                HirItemKind::MacroInvocation { arguments, .. } => Some(arguments),
                _ => None,
            })
            .expect("macro form is present");
        assert_eq!(macro_item, &[HirValue::Integer(1), HirValue::Bool(true)]);
    }

    #[test]
    fn formatter_emits_explicit_forms_and_renderer_escapes_html() {
        let source = concat!(
            "#!liminal-explicit-v1\n",
            "node root (id = \"root\") {\n",
            "  literal \"value\";\n",
            "  attribute flags = [null, false, @root, \"a,b\"];\n",
            "  reference @root;\n",
            "  expression \"x + 1\";\n",
            "  macro expand(1, true);\n",
            "}\n",
            "relation @root -[links]-> @root;\n",
            "ordered {\n",
            "  literal \"tail\";\n",
            "}\n",
        );
        let formatter = MarkdownFormatter::default();
        let emitted = formatter.format(source).expect("explicit source formats");
        assert!(emitted.starts_with("#!liminal-explicit-v1\n"));
        for expected in [
            "node root",
            "literal \"value\";",
            "attribute flags = ",
            "reference @root;",
            "expression \"x + 1\";",
            "macro expand(1, true);",
            "relation ",
            "ordered {",
        ] {
            assert!(
                emitted.contains(expected),
                "missing emitted form {expected}"
            );
        }
        assert!(emitted.contains("[null, false"));
        assert_eq!(
            formatter.emit(&formatter.parse(source).unwrap()).unwrap(),
            emitted
        );

        let rendered = MarkdownRenderer
            .render("line\n<&>\"' {#node}")
            .expect("HTML renders");
        assert!(rendered.contains("&lt;&amp;&gt;&quot;&#39;"));
        assert!(rendered.contains("data-node-id=\"node\""));
        assert!(rendered.contains("<br />"));
    }

    #[test]
    fn markdown_document_equality_ignores_ranges_and_error_display_is_stable() {
        let left = MarkdownDocument {
            blocks: vec![Block {
                text: "same".into(),
                id: Some("id".into()),
                range: liminal_source::SourceRange { start: 0, end: 4 },
            }],
        };
        let mut right = left.clone();
        right.blocks[0].range = liminal_source::SourceRange { start: 10, end: 14 };
        assert_eq!(left, right);
        right.blocks[0].text = "different".into();
        assert_ne!(left, right);
        assert_eq!(MarkdownError.to_string(), "markdown formatting error");

        let formatter = MarkdownFormatter::default();
        let base = formatter.parse("plain").expect("phase1 parse");
        let mut same_semantics = base.clone();
        let holder = same_semantics.holder.clone();
        if let Some(BasisComponent::FileContent { hash, .. }) =
            same_semantics.graph.basis.components.get_mut(&holder)
        {
            *hash = ContentHash::of(b"different source hash");
        }
        assert_eq!(base, same_semantics);
        let mut different_path = base.clone();
        if let Some(BasisComponent::FileContent { path, .. }) =
            different_path.graph.basis.components.get_mut(&holder)
        {
            *path = PathId("other.md".into());
        }
        assert_ne!(base, different_path);
        let mut different_transaction = base.clone();
        different_transaction.graph.basis.transaction = TransactionId::new();
        assert_ne!(base, different_transaction);
    }
}

#[cfg(test)]
mod dialect_tests {
    use super::*;

    const COMPACT: &str = "alpha {#a}\n\nbeta {#b}\n\nunmarked block\n";

    /// AM-17.3: asking for the compact dialect returns the COMPACT surface,
    /// and it round-trips back to the same document. `lim fmt` on a reader's
    /// Markdown must not hand back `#!liminal-explicit-v1`.
    #[test]
    fn compact_dialect_round_trips_without_lowering_to_explicit() {
        let formatter = MarkdownFormatter::default();
        let doc = formatter.parse(COMPACT).expect("compact source parses");
        let emitted = formatter
            .emit_in_dialect(&doc, SourceDialect::CompactOrExplicitV1)
            .expect("compact emission");

        assert!(
            !emitted.contains("#!liminal-explicit-v1") && !emitted.contains("node paragraph"),
            "compact request must not lower to the explicit surface: {emitted:?}"
        );
        assert!(
            emitted.contains("alpha {#a}") && emitted.contains("beta {#b}"),
            "compact emission must keep the compact id spelling: {emitted:?}"
        );
        let reparsed = formatter
            .parse(&emitted)
            .expect("compact emission reparses");
        assert_eq!(reparsed, doc, "compact emission must round-trip");
    }

    /// The explicit dialect keeps the canonical projection unchanged — `emit`
    /// was not mutated by this amendment.
    #[test]
    fn explicit_dialect_matches_untouched_emit() {
        let formatter = MarkdownFormatter::default();
        let doc = formatter.parse(COMPACT).expect("parses");
        assert_eq!(
            formatter
                .emit_in_dialect(&doc, SourceDialect::ExplicitV1)
                .expect("explicit emission"),
            formatter.emit(&doc).expect("canonical emission"),
        );
    }

    /// A document with no compact spelling falls back to the explicit surface
    /// rather than emitting a lossy approximation.
    #[test]
    fn non_compact_representable_document_falls_back_to_explicit() {
        let formatter = MarkdownFormatter::default();
        let source = concat!(
            "#!liminal-explicit-v1\n",
            "node root (id = \"root\") {\n",
            "  literal \"value\";\n",
            "  attribute color = \"blue\";\n",
            "}\n",
        );
        let doc = formatter.parse(source).expect("explicit source parses");
        let emitted = formatter
            .emit_in_dialect(&doc, SourceDialect::CompactOrExplicitV1)
            .expect("emission");
        assert!(
            emitted.contains("#!liminal-explicit-v1"),
            "a document the compact surface cannot express must fall back: {emitted:?}"
        );
        assert_eq!(
            formatter.parse(&emitted).expect("fallback reparses"),
            doc,
            "the fallback must still round-trip"
        );
    }
}
