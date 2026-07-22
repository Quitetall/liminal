//! Frozen M19.1 Human IR schema and source-map validation (v4 §§10–17, 22).

use std::collections::BTreeMap;

use liminal_cst::Parse;
use serde::{Deserialize, Serialize};

use crate::SourceRange;
use liminal_source::{SourceBasis, paragraph};

/// One of v4 section 17's explicit syntax forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyntaxForm {
    /// Scalar literal.
    Literal,
    /// Node construction.
    NodeConstruction,
    /// Relation construction.
    RelationConstruction,
    /// Attribute assignment.
    AttributeAssignment,
    /// Ordered block.
    OrderedBlock,
    /// Symbolic reference.
    Reference,
    /// Unevaluated expression.
    Expression,
    /// Unexpanded macro invocation.
    MacroInvocation,
}

/// Frontend dialect accepted by HIR lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDialect {
    /// Compact Markdown sugar or explicit marker.
    CompactOrExplicitV1,
    /// Explicit form requiring marker.
    ExplicitV1,
}

/// Source-map annotation class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnnotationKind {
    /// Recognized form.
    Form,
    /// Lexical token.
    Token,
    /// Attribute span.
    Attribute,
    /// Reference span.
    Reference,
    /// Error/recovery span.
    Error,
    /// Provenance span.
    Provenance,
}

/// One source-map annotation tied to exact source basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    /// Annotation class.
    pub kind: AnnotationKind,
    /// Exact UTF-8 byte range.
    pub range: SourceRange,
    /// Associated HIR item, if any.
    pub hir: Option<HirId>,
}

/// Ephemeral source map over one exact source revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMap {
    /// Holder identity and content hash.
    pub basis: SourceBasis,
    /// Canonically sorted annotations.
    pub annotations: Vec<Annotation>,
}

/// Dense HIR item identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HirId(pub u32);

/// Symbolic or locally resolved reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HirReference {
    /// Resolved local item.
    Resolved(HirId),
    /// Exact unresolved spelling.
    Unresolved(String),
}

/// Typed HIR value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HirValue {
    /// Null.
    Null,
    /// Boolean.
    Bool(bool),
    /// Signed integer.
    Integer(i64),
    /// UTF-8 string.
    String(String),
    /// Symbolic or resolved reference.
    Reference(HirReference),
    /// Ordered list.
    List(Vec<HirValue>),
}

/// Semantic HIR item kind preserving authorial syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HirItemKind {
    /// Literal text.
    Literal {
        /// Literal text value.
        value: String,
    },
    /// Named node construction.
    NodeConstruction {
        /// Declared node name.
        name: String,
    },
    /// Typed relation construction.
    RelationConstruction {
        /// Source endpoint.
        source: HirReference,
        /// Relation kind spelling.
        kind: String,
        /// Target endpoint.
        target: HirReference,
    },
    /// Attribute assignment.
    AttributeAssignment {
        /// Nearest enclosing owner, or `None` at top level.
        owner: Option<HirId>,
        /// Attribute name.
        name: String,
        /// Attribute value.
        value: HirValue,
    },
    /// Ordered block.
    OrderedBlock,
    /// Reference shorthand.
    Reference {
        /// Referenced target.
        target: HirReference,
    },
    /// Unevaluated expression.
    Expression {
        /// Expression source spelling.
        source: String,
    },
    /// Unexpanded macro invocation.
    MacroInvocation {
        /// Macro name.
        name: String,
        /// Ordered arguments.
        arguments: Vec<HirValue>,
    },
}

/// One dense HIR item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HirItem {
    /// Dense item ID.
    pub id: HirId,
    /// Exact source range.
    pub range: SourceRange,
    /// Preserved semantic kind.
    pub kind: HirItemKind,
    /// Effective attributes.
    pub attributes: BTreeMap<String, HirValue>,
    /// Ordered child IDs.
    pub children: Vec<HirId>,
}

/// Human IR document tied to one source basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HirDocument {
    /// Exact source basis.
    pub basis: SourceBasis,
    /// Root item IDs.
    pub roots: Vec<HirId>,
    /// Dense items sorted by ID.
    pub items: Vec<HirItem>,
    /// Ephemeral source map.
    pub source_map: SourceMap,
}

/// Nonfatal HIR diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HirDiagnosticCode {
    /// Unknown explicit form.
    UnknownForm,
    /// Integer overflow.
    IntegerOverflow,
    /// Malformed syntax.
    MalformedSyntax,
    /// Ownerless attribute.
    AttributeWithoutOwner,
}

/// Nonfatal lowering diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HirDiagnostic {
    /// Stable code.
    pub code: HirDiagnosticCode,
    /// Exact source range.
    pub range: SourceRange,
    /// Deterministic message.
    pub message: String,
}

/// Result of lossless CST-to-HIR lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHir {
    /// Lowered document.
    pub document: HirDocument,
    /// Nonfatal diagnostics in source order.
    pub diagnostics: Vec<HirDiagnostic>,
}

type LowerParts = (
    Vec<HirItem>,
    Vec<HirId>,
    Vec<HirDiagnostic>,
    Vec<Annotation>,
);

/// Fatal HIR invariant violation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HirError {
    /// Source-map invariant failed.
    #[error("invalid source map: {0}")]
    InvalidSourceMap(String),
    /// ID does not refer to a dense item.
    #[error("dangling HIR id {0:?}")]
    Dangling(HirId),
}

/// Validate source-map ranges and canonical annotation order.
pub fn validate_source_map(map: &SourceMap, source_len: u64) -> Result<(), HirError> {
    let mut previous: Option<(u64, u64, AnnotationKind, Option<HirId>)> = None;
    for annotation in &map.annotations {
        let range = annotation.range;
        if range.start > range.end || range.end > source_len {
            return Err(HirError::InvalidSourceMap(format!(
                "annotation range {}..{} outside source length {source_len}",
                range.start, range.end
            )));
        }
        let key = (range.start, range.end, annotation.kind, annotation.hir);
        if previous.is_some_and(|old| old > key) {
            return Err(HirError::InvalidSourceMap(
                "annotations are not sorted by range, kind, and HIR id".into(),
            ));
        }
        previous = Some(key);
    }
    Ok(())
}

/// Lower one M18 lossless parse into deterministic minimal HIR.
pub fn lower(cst: &Parse, dialect: SourceDialect) -> Result<LoweredHir, HirError> {
    let source = cst.emit_lossless();
    let end = u64::try_from(source.len()).expect("usize fits u64");
    let explicit = source.starts_with("#!liminal-explicit-v1");
    if matches!(dialect, SourceDialect::ExplicitV1) && !explicit {
        return Err(HirError::InvalidSourceMap(
            "explicit dialect requires #!liminal-explicit-v1 marker".to_owned(),
        ));
    }
    let (mut items, roots, mut diagnostics, mut annotations) = if explicit {
        lower_explicit(&source)
    } else {
        lower_compact(&source)
    };
    for error in cst.errors() {
        annotations.push(Annotation {
            kind: AnnotationKind::Error,
            range: error.range,
            hir: None,
        });
        diagnostics.push(HirDiagnostic {
            code: HirDiagnosticCode::MalformedSyntax,
            range: error.range,
            message: format!("parser recovery: {:?}", error.code),
        });
    }
    annotations.sort_by_key(|annotation| {
        (
            annotation.range.start,
            annotation.range.end,
            annotation.kind,
            annotation.hir,
        )
    });
    let basis = cst.basis().clone();
    let source_map = SourceMap {
        basis: basis.clone(),
        annotations,
    };
    validate_source_map(&source_map, end)?;
    items.sort_by_key(|item| item.id);
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic.range.start,
            diagnostic.range.end,
            diagnostic.code,
            diagnostic.message.clone(),
        )
    });
    Ok(LoweredHir {
        document: HirDocument {
            basis,
            roots,
            items,
            source_map,
        },
        diagnostics,
    })
}

fn lower_compact(
    source: &str,
) -> (
    Vec<HirItem>,
    Vec<HirId>,
    Vec<HirDiagnostic>,
    Vec<Annotation>,
) {
    let blocks = paragraph::parse(source);
    let mut items = Vec::with_capacity(blocks.len() * 2);
    let mut roots = Vec::with_capacity(blocks.len());
    let mut annotations = Vec::with_capacity(blocks.len() * 2);
    for block in blocks {
        let node_id = HirId(u32::try_from(items.len()).expect("HIR item count fits u32"));
        let mut attributes = BTreeMap::new();
        if let Some(id) = block.id {
            attributes.insert("id".to_owned(), HirValue::String(id));
        }
        let (name, value, extra) = compact_projection(&block.text, &mut attributes);
        let mut children = Vec::new();
        let literal_id = HirId(node_id.0 + 1);
        items.push(HirItem {
            id: node_id,
            range: block.range,
            kind: HirItemKind::NodeConstruction { name },
            attributes,
            children: vec![literal_id],
        });
        items.push(HirItem {
            id: literal_id,
            range: block.range,
            kind: HirItemKind::Literal { value },
            attributes: BTreeMap::new(),
            children: Vec::new(),
        });
        children.push(literal_id);
        for (child_name, child_value) in extra {
            let child_id = HirId(u32::try_from(items.len()).expect("HIR item count fits u32"));
            let literal = HirId(child_id.0 + 1);
            items.push(HirItem {
                id: child_id,
                range: block.range,
                kind: HirItemKind::NodeConstruction { name: child_name },
                attributes: BTreeMap::new(),
                children: vec![literal],
            });
            items.push(HirItem {
                id: literal,
                range: block.range,
                kind: HirItemKind::Literal { value: child_value },
                attributes: BTreeMap::new(),
                children: Vec::new(),
            });
            children.push(child_id);
        }
        if let Some(item) = items.iter_mut().find(|item| item.id == node_id) {
            item.children = children;
        }
        roots.push(node_id);
        annotations.push(Annotation {
            kind: AnnotationKind::Form,
            range: block.range,
            hir: Some(node_id),
        });
        annotations.push(Annotation {
            kind: AnnotationKind::Token,
            range: block.range,
            hir: Some(literal_id),
        });
    }
    (items, roots, Vec::new(), annotations)
}

fn compact_projection(
    text: &str,
    attributes: &mut BTreeMap<String, HirValue>,
) -> (String, String, Vec<(String, String)>) {
    let first = text.lines().next().unwrap_or_default();
    let heading = first
        .trim_start()
        .chars()
        .take_while(|ch| *ch == '#')
        .count();
    if heading > 0 && first.chars().nth(heading) == Some(' ') {
        attributes.insert(
            "level".into(),
            HirValue::Integer(i64::try_from(heading).unwrap_or(i64::MAX)),
        );
        return (
            "heading".into(),
            first[heading + 1..].to_owned(),
            Vec::new(),
        );
    }
    if let Some(info) = first.strip_prefix("```") {
        attributes.insert("info".into(), HirValue::String(info.trim().to_owned()));
        let body = text
            .lines()
            .skip(1)
            .take_while(|line| !line.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n");
        return ("code-block".into(), body, Vec::new());
    }
    if text.lines().all(|line| line.trim_start().starts_with('>')) {
        let body = text
            .lines()
            .map(|line| line.trim_start().trim_start_matches('>').trim_start())
            .collect::<Vec<_>>()
            .join("\n");
        return ("block-quote".into(), body, Vec::new());
    }
    if text
        .lines()
        .all(|line| line.trim_start().starts_with("- ") || line.trim_start().starts_with("* "))
    {
        let extra = text
            .lines()
            .map(|line| ("list-item".into(), line.trim_start()[2..].to_owned()))
            .collect();
        return ("unordered-list".into(), String::new(), extra);
    }
    if text.lines().all(|line| {
        line.trim_start()
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
    }) && text.lines().all(|line| line.contains(". "))
    {
        let extra = text
            .lines()
            .map(|line| {
                let (_, value) = line.trim_start().split_once(". ").unwrap_or(("", line));
                ("list-item".into(), value.to_owned())
            })
            .collect();
        return ("ordered-list".into(), String::new(), extra);
    }
    ("paragraph".into(), text.to_owned(), Vec::new())
}

fn lower_explicit(source: &str) -> LowerParts {
    let mut items = Vec::new();
    let mut roots = Vec::new();
    let mut diagnostics = Vec::new();
    let mut annotations = Vec::new();
    let marker_len = source.find('\n').map_or(source.len(), |offset| offset + 1);
    let mut offset = marker_len;
    let mut stack: Vec<HirId> = Vec::new();
    for raw in source[marker_len..].split_inclusive('\n') {
        let line = raw.trim_end_matches(['\r', '\n']).trim();
        let line_len = u64::try_from(raw.len()).expect("line length fits u64");
        let range = SourceRange {
            start: u64::try_from(offset).expect("offset fits u64"),
            end: u64::try_from(offset).expect("offset fits u64") + line_len,
        };
        offset += raw.len();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            if stack.pop().is_none() {
                diagnostics.push(HirDiagnostic {
                    code: HirDiagnosticCode::MalformedSyntax,
                    range,
                    message: "unexpected closing brace".to_owned(),
                });
            }
            continue;
        }
        let opens = line.ends_with('{');
        let statement = line.trim_end_matches('{').trim();
        let id = HirId(u32::try_from(items.len()).expect("HIR item count fits u32"));
        let parent = stack.last().copied();
        let (kind, attributes) = parse_explicit_kind(statement, range, parent, &mut diagnostics);
        items.push(HirItem {
            id,
            range,
            kind,
            attributes,
            children: Vec::new(),
        });
        if let Some(parent) = parent {
            if let Some(owner) = items.iter_mut().find(|item| item.id == parent) {
                owner.children.push(id);
            }
        } else {
            roots.push(id);
        }
        annotations.push(Annotation {
            kind: AnnotationKind::Form,
            range,
            hir: Some(id),
        });
        if opens
            && matches!(
                items.last().map(|item| &item.kind),
                Some(HirItemKind::NodeConstruction { .. } | HirItemKind::OrderedBlock)
            )
        {
            stack.push(id);
        }
    }
    if !stack.is_empty() {
        diagnostics.push(HirDiagnostic {
            code: HirDiagnosticCode::MalformedSyntax,
            range: SourceRange {
                start: 0,
                end: u64::try_from(source.len()).expect("source length fits u64"),
            },
            message: format!("unclosed braces: {} block(s) never closed", stack.len()),
        });
    }
    (items, roots, diagnostics, annotations)
}

#[allow(clippy::too_many_lines)]
fn parse_explicit_kind(
    line: &str,
    range: SourceRange,
    owner: Option<HirId>,
    diagnostics: &mut Vec<HirDiagnostic>,
) -> (HirItemKind, BTreeMap<String, HirValue>) {
    let (keyword, rest) = line.split_once(' ').unwrap_or((line, ""));
    match keyword {
        "literal" => (
            HirItemKind::Literal {
                value: parse_json_string(rest.trim_end_matches(';').trim()).unwrap_or_else(|| {
                    diagnostics.push(HirDiagnostic {
                        code: HirDiagnosticCode::MalformedSyntax,
                        range,
                        message: "literal requires JSON string".to_owned(),
                    });
                    rest.trim_end_matches(';').to_owned()
                }),
            },
            BTreeMap::new(),
        ),
        "node" => {
            let (name, attributes) = parse_named_attributes(rest);
            (HirItemKind::NodeConstruction { name }, attributes)
        }
        "relation" => {
            let relation = rest.trim().trim_end_matches(';');
            let parts = relation.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 3 {
                (
                    HirItemKind::RelationConstruction {
                        source: HirReference::Unresolved(
                            parts[0].trim_start_matches('@').to_owned(),
                        ),
                        kind: parts[1].trim_matches(['-', '[', ']']).to_owned(),
                        target: HirReference::Unresolved(
                            parts.last().unwrap().trim_start_matches('@').to_owned(),
                        ),
                    },
                    parse_trailing_attributes(relation),
                )
            } else {
                diagnostics.push(HirDiagnostic {
                    code: HirDiagnosticCode::MalformedSyntax,
                    range,
                    message: "relation requires source, kind, and target".to_owned(),
                });
                (
                    HirItemKind::Literal {
                        value: line.to_owned(),
                    },
                    BTreeMap::new(),
                )
            }
        }
        "attribute" => {
            let (name, value) = rest.split_once('=').unwrap_or((rest, "null"));
            if owner.is_none() {
                diagnostics.push(HirDiagnostic {
                    code: HirDiagnosticCode::AttributeWithoutOwner,
                    range,
                    message: "top-level attribute has no owner".to_owned(),
                });
            }
            (
                HirItemKind::AttributeAssignment {
                    owner,
                    name: name.trim().to_owned(),
                    value: parse_value(value.trim().trim_end_matches(';')),
                },
                BTreeMap::new(),
            )
        }
        "ordered" => (HirItemKind::OrderedBlock, BTreeMap::new()),
        "reference" => (
            HirItemKind::Reference {
                target: HirReference::Unresolved(
                    rest.trim()
                        .trim_start_matches('@')
                        .trim_end_matches(';')
                        .to_owned(),
                ),
            },
            BTreeMap::new(),
        ),
        "expression" => (
            HirItemKind::Expression {
                source: parse_json_string(rest.trim_end_matches(';'))
                    .unwrap_or_else(|| rest.trim_end_matches(';').to_owned()),
            },
            BTreeMap::new(),
        ),
        "macro" => {
            // M19 arguments are flat typed values; nested invocation syntax
            // belongs to the later macro-expansion phase.
            let (name_part, args_part) = rest.split_once('(').unwrap_or((rest, ""));
            let name = name_part.trim().to_owned();
            let args = args_part.trim_end_matches(';').trim_end_matches(')').trim();
            (
                HirItemKind::MacroInvocation {
                    name,
                    arguments: if args.is_empty() {
                        Vec::new()
                    } else {
                        split_top_level(args, ',')
                            .into_iter()
                            .map(|value| parse_value(value.trim()))
                            .collect()
                    },
                },
                BTreeMap::new(),
            )
        }
        _ => {
            diagnostics.push(HirDiagnostic {
                code: HirDiagnosticCode::UnknownForm,
                range,
                message: format!("unknown explicit form {keyword:?}"),
            });
            (
                HirItemKind::Literal {
                    value: line.to_owned(),
                },
                BTreeMap::new(),
            )
        }
    }
}

fn parse_named_attributes(rest: &str) -> (String, BTreeMap<String, HirValue>) {
    let rest = rest.trim().trim_end_matches(';').trim();
    let (name, attrs) = rest.split_once('(').map_or((rest, ""), |(name, attrs)| {
        (name.trim(), attrs.trim_end_matches(')').trim())
    });
    (name.to_owned(), parse_attributes(attrs))
}

fn parse_trailing_attributes(rest: &str) -> BTreeMap<String, HirValue> {
    rest.split_once('(')
        .map_or_else(BTreeMap::new, |(_, attrs)| {
            parse_attributes(attrs.trim_end_matches(';').trim_end_matches(')').trim())
        })
}

fn parse_attributes(value: &str) -> BTreeMap<String, HirValue> {
    split_top_level(value, ',')
        .into_iter()
        .filter_map(|part| part.split_once('='))
        .map(|(name, value)| (name.trim().to_owned(), parse_value(value.trim())))
        .collect()
}

/// Split a comma-separated explicit-syntax field without splitting inside a
/// JSON string or nested list/parenthesized value.
fn split_top_level(value: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                parts.push(&value[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

fn parse_json_string(value: &str) -> Option<String> {
    serde_json::from_str::<String>(value.trim()).ok()
}

fn parse_value(value: &str) -> HirValue {
    if let Some(string) = parse_json_string(value) {
        return HirValue::String(string);
    }
    match value {
        "null" => HirValue::Null,
        "true" => HirValue::Bool(true),
        "false" => HirValue::Bool(false),
        value if value.starts_with('@') => HirValue::Reference(HirReference::Unresolved(
            value.trim_start_matches('@').to_owned(),
        )),
        value => value
            .parse::<i64>()
            .map_or_else(|_| HirValue::String(value.to_owned()), HirValue::Integer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liminal_id::{ContentHash, SourceId};

    fn map(annotations: Vec<Annotation>) -> SourceMap {
        SourceMap {
            basis: SourceBasis {
                source: SourceId::from_name("hir-test"),
                content_hash: ContentHash::of(b"abc"),
            },
            annotations,
        }
    }

    #[test]
    fn source_map_rejects_out_of_bounds_and_unsorted_ranges() {
        let out_of_bounds = map(vec![Annotation {
            kind: AnnotationKind::Token,
            range: SourceRange { start: 0, end: 4 },
            hir: None,
        }]);
        assert!(matches!(
            validate_source_map(&out_of_bounds, 3),
            Err(HirError::InvalidSourceMap(_))
        ));

        let unsorted = map(vec![
            Annotation {
                kind: AnnotationKind::Token,
                range: SourceRange { start: 2, end: 3 },
                hir: None,
            },
            Annotation {
                kind: AnnotationKind::Form,
                range: SourceRange { start: 0, end: 1 },
                hir: None,
            },
        ]);
        assert!(matches!(
            validate_source_map(&unsorted, 3),
            Err(HirError::InvalidSourceMap(_))
        ));

        let boundary_and_empty = map(vec![
            Annotation {
                kind: AnnotationKind::Token,
                range: SourceRange { start: 0, end: 3 },
                hir: None,
            },
            Annotation {
                kind: AnnotationKind::Token,
                range: SourceRange { start: 3, end: 3 },
                hir: None,
            },
            Annotation {
                kind: AnnotationKind::Token,
                range: SourceRange { start: 3, end: 3 },
                hir: None,
            },
        ]);
        assert!(validate_source_map(&boundary_and_empty, 3).is_ok());

        let reversed = map(vec![Annotation {
            kind: AnnotationKind::Token,
            range: SourceRange { start: 3, end: 2 },
            hir: None,
        }]);
        assert!(matches!(
            validate_source_map(&reversed, 3),
            Err(HirError::InvalidSourceMap(_))
        ));
    }

    #[test]
    fn syntax_form_has_exact_eight_variants() {
        let forms = [
            SyntaxForm::Literal,
            SyntaxForm::NodeConstruction,
            SyntaxForm::RelationConstruction,
            SyntaxForm::AttributeAssignment,
            SyntaxForm::OrderedBlock,
            SyntaxForm::Reference,
            SyntaxForm::Expression,
            SyntaxForm::MacroInvocation,
        ];
        assert_eq!(forms.len(), 8);
        assert!(forms.windows(2).all(|pair| pair[0] != pair[1]));
    }
}
