use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Fix, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    style: DescStyle,
}

#[derive(Default, Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
enum DescStyle {
    #[default]
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "inline")]
    Single,
}

#[derive(Rule)]
#[rule(
    id = "description-style",
    category = "schema",
    has_suggestions = true,
    kinds = "DESCRIPTION|ENUM_VALUE"
)]
pub struct DescriptionStyle;

impl DescriptionStyle {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        let want_block = opts.style == DescStyle::Block;
        Box::new(DescriptionStyleHandler {
            want_block,
            items: Vec::new(),
            enum_value_names: HashMap::new(),
        })
    }
}

struct Item {
    desc_span: Span,
    parent_kind: SyntaxKind,
    parent_name: Option<String>,
    container_kind: SyntaxKind,
    container_name: Option<String>,
}

struct DescriptionStyleHandler {
    want_block: bool,
    /// DESCRIPTION items buffered during the walk; processed in finalize.
    items: Vec<Item>,
    /// ENUM_VALUE_DEFINITION span offset → enum value name.
    /// The name sits in an `ENUM_VALUE` child (not on the definition itself),
    /// so we record it when the walk visits `ENUM_VALUE` and look it up when
    /// building the message for the parent `ENUM_VALUE_DEFINITION`.
    enum_value_names: HashMap<usize, String>,
}

/// True when `kind` is a type-definition-like node that can serve as a
/// container for `FIELD_DEFINITION` / `INPUT_VALUE_DEFINITION` /
/// `ENUM_VALUE_DEFINITION` in "X in Y" messages.
fn is_container_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_EXTENSION
            | SyntaxKind::DIRECTIVE_DEFINITION
            | SyntaxKind::SCHEMA_DEFINITION
            | SyntaxKind::SCHEMA_EXTENSION
            | SyntaxKind::FRAGMENT_DEFINITION
            | SyntaxKind::OPERATION_DEFINITION
            | SyntaxKind::SELECTION_SET
    )
}

impl Handler for DescriptionStyleHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::DESCRIPTION => {
                let Some(p) = parent else { return };
                let desc_span = match node.span {
                    Some(s) => s,
                    None => return,
                };

                let parent_name = p.name.clone();
                let parent_kind = p.kind;
                let (container_kind, container_name) = find_container(parent);

                self.items.push(Item {
                    desc_span,
                    parent_kind,
                    parent_name,
                    container_kind,
                    container_name,
                });
            }

            // ENUM_VALUE children carry the name that `ENUM_VALUE_DEFINITION`
            // itself lacks. Record the name keyed by the definition's span
            // offset so the DESCRIPTION handler can look it up in finalize.
            SyntaxKind::ENUM_VALUE => {
                let Some(p) = parent else { return };
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                let key = match p.span {
                    Some(s) => s.offset,
                    None => return,
                };
                self.enum_value_names.insert(key, name);
            }

            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for item in self.items.drain(..) {
            let raw = ctx.source_code().slice(item.desc_span);

            // Detect description style. For ENUM_VALUE_DEFINITION, the span
            // covers the whole definition; skip if no string marker.
            let is_block = if raw.starts_with("\"\"\"") {
                true
            } else if raw.starts_with('"') {
                false
            } else {
                continue;
            };

            let mismatched = if self.want_block { !is_block } else { is_block };
            if !mismatched {
                continue;
            }

            let style_label = if self.want_block { "block" } else { "inline" };
            let unexpected_style = if is_block { "block" } else { "inline" };

            // For ENUM_VALUE_DEFINITION look up the value name recorded when
            // the walk visited the child ENUM_VALUE node.
            let parent_name = if item.parent_kind == SyntaxKind::ENUM_VALUE_DEFINITION
                && item.parent_name.is_none()
            {
                self.enum_value_names.get(&item.desc_span.offset).cloned()
            } else {
                item.parent_name
            };

            let parent_node_name = get_node_name(
                item.parent_kind,
                &parent_name,
                item.container_kind,
                &item.container_name,
            );
            let message =
                format!("Unexpected {unexpected_style} description for {parent_node_name}");

            let new_text = if self.want_block {
                convert_to_block(raw)
            } else {
                convert_to_inline(raw)
            };

            ctx.report(
                DiagnosticBuilder::new(
                    ctx.rule_id(),
                    ctx.source_code().path().to_path_buf(),
                    item.desc_span,
                    message,
                )
                .suggestion(
                    format!("Change to {style_label} style description"),
                    Fix::Replace {
                        span: item.desc_span,
                        text: new_text,
                    },
                ),
            );
        }
    }
}

fn find_container<'a>(parent: Option<&'a Node<'a>>) -> (SyntaxKind, Option<String>) {
    let Some(mut current) = parent else {
        return (SyntaxKind::TOMBSTONE, None);
    };
    loop {
        if is_container_kind(current.kind) {
            return (current.kind, current.name.clone());
        }
        match current.parent {
            Some(p) => current = p,
            None => return (SyntaxKind::TOMBSTONE, None),
        }
    }
}

fn display_kind_label(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "type",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "interface",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => {
            "input"
        }
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "enum",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "scalar",
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "union",
        SyntaxKind::DIRECTIVE_DEFINITION => "directive",
        SyntaxKind::SCHEMA_DEFINITION | SyntaxKind::SCHEMA_EXTENSION => "schema",
        SyntaxKind::FIELD_DEFINITION => "field",
        SyntaxKind::INPUT_VALUE_DEFINITION => "input value",
        SyntaxKind::ENUM_VALUE => "enum value",
        SyntaxKind::ENUM_VALUE_DEFINITION => "enum value",
        SyntaxKind::FRAGMENT_DEFINITION => "fragment",
        SyntaxKind::FRAGMENT_SPREAD => "fragment spread",
        SyntaxKind::OPERATION_DEFINITION => "operation",
        SyntaxKind::SELECTION_SET => "selection set",
        _ => "",
    }
}

fn display_node_name(kind: SyntaxKind, name: &Option<String>) -> String {
    let label = display_kind_label(kind);
    match name {
        Some(n) => format!("{label} \"{n}\""),
        None => format!("{label} \"\""),
    }
}

fn get_node_name(
    parent_kind: SyntaxKind,
    parent_name: &Option<String>,
    container_kind: SyntaxKind,
    container_name: &Option<String>,
) -> String {
    match parent_kind {
        SyntaxKind::FIELD_DEFINITION
        | SyntaxKind::INPUT_VALUE_DEFINITION
        | SyntaxKind::ENUM_VALUE
        | SyntaxKind::ENUM_VALUE_DEFINITION => {
            let self_str = display_node_name(parent_kind, parent_name);
            let container_str = display_node_name(container_kind, container_name);
            format!("{self_str} in {container_str}")
        }
        _ => display_node_name(parent_kind, parent_name),
    }
}

fn convert_to_block(raw: &str) -> String {
    let inner = raw
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(raw);
    format!("\"\"\"{}\"\"\"", inner)
}

fn convert_to_inline(raw: &str) -> String {
    let inner = raw
        .strip_prefix("\"\"\"")
        .and_then(|s| s.strip_suffix("\"\"\""))
        .unwrap_or(raw);
    let collapsed: String = inner.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{collapsed}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_023() {
        let rule = DescriptionStyle;
        let meta = rule.meta();
        assert_eq!(meta.id, "description-style");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(meta.has_suggestions);
    }

    #[test]
    fn option_deserializes_from_json() {
        // Default (null) → Block
        let opts: Opts = serde_json::from_value(serde_json::Value::Null).unwrap_or_default();
        assert_eq!(opts.style, DescStyle::Block, "null should default to Block");

        // Explicit "block"
        let opts: Opts =
            serde_json::from_value(serde_json::json!({"style": "block"})).unwrap_or_default();
        assert_eq!(opts.style, DescStyle::Block, "style=block -> Block");

        // Explicit "inline"
        let opts: Opts =
            serde_json::from_value(serde_json::json!({"style": "inline"})).unwrap_or_default();
        assert_eq!(opts.style, DescStyle::Single, "style=inline -> Single");

        // Empty object → default
        let opts: Opts = serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert_eq!(
            opts.style,
            DescStyle::Block,
            "empty object should default to Block"
        );
    }
}
