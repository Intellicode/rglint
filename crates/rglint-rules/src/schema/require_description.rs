use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Default, Debug, Deserialize)]
struct Opts {
    #[serde(default)]
    types: Option<bool>,
    #[serde(rename = "ObjectTypeDefinition", default)]
    object_type_definition: Option<bool>,
    #[serde(rename = "InterfaceTypeDefinition", default)]
    interface_type_definition: Option<bool>,
    #[serde(rename = "EnumTypeDefinition", default)]
    enum_type_definition: Option<bool>,
    #[serde(rename = "ScalarTypeDefinition", default)]
    scalar_type_definition: Option<bool>,
    #[serde(rename = "InputObjectTypeDefinition", default)]
    input_object_type_definition: Option<bool>,
    #[serde(rename = "UnionTypeDefinition", default)]
    union_type_definition: Option<bool>,
    #[serde(rename = "DirectiveDefinition", default)]
    directive_definition: Option<bool>,
    #[serde(rename = "FieldDefinition", default)]
    field_definition: Option<bool>,
    #[serde(rename = "InputValueDefinition", default)]
    input_value_definition: Option<bool>,
    #[serde(rename = "EnumValueDefinition", default)]
    enum_value_definition: Option<bool>,
    #[serde(rename = "OperationDefinition", default)]
    operation_definition: Option<bool>,
}

fn is_type_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_EXTENSION
    )
}

impl Opts {
    fn is_enabled(&self, kind: SyntaxKind) -> bool {
        let specific = match kind {
            k if is_type_kind(k) => match k {
                SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => {
                    self.object_type_definition
                }
                SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => {
                    self.interface_type_definition
                }
                SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => {
                    self.enum_type_definition
                }
                SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => {
                    self.scalar_type_definition
                }
                SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
                | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => self.input_object_type_definition,
                SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => {
                    self.union_type_definition
                }
                _ => unreachable!(),
            },
            SyntaxKind::DIRECTIVE_DEFINITION => self.directive_definition,
            SyntaxKind::SCHEMA_DEFINITION | SyntaxKind::SCHEMA_EXTENSION => None,
            SyntaxKind::FIELD_DEFINITION => self.field_definition,
            SyntaxKind::INPUT_VALUE_DEFINITION => self.input_value_definition,
            SyntaxKind::ENUM_VALUE_DEFINITION => self.enum_value_definition,
            SyntaxKind::OPERATION_DEFINITION => self.operation_definition,
            _ => None,
        };
        specific
            .or(self.types.filter(|_| is_type_kind(kind)))
            .unwrap_or(false)
    }
}

fn is_type_def_kind(kind: SyntaxKind) -> bool {
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
    )
}

#[derive(Rule)]
#[rule(
    id = "require-description",
    category = "schema",
    kinds = "DESCRIPTION|ENUM_VALUE|OBJECT_TYPE_DEFINITION|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_DEFINITION|INTERFACE_TYPE_EXTENSION|ENUM_TYPE_DEFINITION|ENUM_TYPE_EXTENSION|SCALAR_TYPE_DEFINITION|SCALAR_TYPE_EXTENSION|INPUT_OBJECT_TYPE_DEFINITION|INPUT_OBJECT_TYPE_EXTENSION|UNION_TYPE_DEFINITION|UNION_TYPE_EXTENSION|DIRECTIVE_DEFINITION|SCHEMA_DEFINITION|SCHEMA_EXTENSION|FIELD_DEFINITION|INPUT_VALUE_DEFINITION|ENUM_VALUE_DEFINITION|OPERATION_DEFINITION"
)]
pub struct RequireDescription;

impl RequireDescription {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(RequireDescriptionHandler {
            opts,
            candidates: Vec::new(),
            described_nodes: Vec::new(),
            enum_value_names: HashMap::new(),
        })
    }
}

struct Violation {
    kind: SyntaxKind,
    name: Option<String>,
    span: Span,
    container_kind: SyntaxKind,
    container_name: Option<String>,
}

struct RequireDescriptionHandler {
    opts: Opts,
    candidates: Vec<Violation>,
    /// Span offsets of definition nodes that have a DESCRIPTION child
    described_nodes: Vec<usize>,
    /// ENUM_VALUE_DEFINITION span offset → value name
    enum_value_names: HashMap<usize, Option<String>>,
}

impl Handler for RequireDescriptionHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::DESCRIPTION => {
                if let Some(p) = parent {
                    if let Some(span) = p.span {
                        self.described_nodes.push(span.offset);
                    }
                }
            }
            SyntaxKind::ENUM_VALUE => {
                if let Some(p) = parent {
                    if p.kind == SyntaxKind::ENUM_VALUE_DEFINITION {
                        if let Some(span) = p.span {
                            self.enum_value_names.insert(span.offset, node.name.clone());
                        }
                    }
                }
            }
            _ => {
                let kind = node.kind;
                if !self.opts.is_enabled(kind) {
                    return;
                }
                let span = match node.span {
                    Some(s) => s,
                    None => return,
                };
                let name = node.name.clone();
                let (container_kind, container_name) = find_container(node);
                self.candidates.push(Violation {
                    kind,
                    name,
                    span,
                    container_kind,
                    container_name,
                });
            }
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        // Only report candidates whose span offset is NOT in the described set
        let violations: Vec<Violation> = self
            .candidates
            .drain(..)
            .filter(|v| !self.described_nodes.contains(&v.span.offset))
            .collect();
        let source = ctx.source_code();
        let source_text = source.source().to_owned();
        let path = source.path().to_path_buf();
        let rule_id = ctx.rule_id();
        for v in violations {
            let actual_name = if v.kind == SyntaxKind::ENUM_VALUE_DEFINITION && v.name.is_none() {
                self.enum_value_names
                    .get(&v.span.offset)
                    .and_then(|n| n.clone())
            } else {
                v.name
            };
            let node_name = if v.kind == SyntaxKind::OPERATION_DEFINITION {
                let op_type = operation_type_label(&source_text, v.span);
                match &actual_name {
                    Some(n) => format!("{op_type} \"{n}\""),
                    None => op_type.to_owned(),
                }
            } else if v.kind == SyntaxKind::ENUM_VALUE_DEFINITION {
                let self_str = enum_value_display(&actual_name);
                let container_str = display_node_name(v.container_kind, &v.container_name);
                format!("{self_str} in {container_str}")
            } else {
                get_node_name(v.kind, &actual_name, v.container_kind, &v.container_name)
            };
            let message = format!("Description is required for {node_name}");
            let report_span = if v.kind == SyntaxKind::OPERATION_DEFINITION {
                v.span
            } else {
                name_span(&source_text, v.kind, &actual_name, v.span)
            };
            ctx.report(DiagnosticBuilder::new(
                rule_id,
                path.clone(),
                report_span,
                message,
            ));
        }
    }
}

fn find_container<'a>(node: &'a Node<'a>) -> (SyntaxKind, Option<String>) {
    let mut current = node;
    loop {
        if is_type_def_kind(current.kind) {
            return (current.kind, current.name.clone());
        }
        match current.parent {
            Some(p) => current = p,
            None => return (SyntaxKind::TOMBSTONE, None),
        }
    }
}

fn operation_type_label(source_text: &str, span: Span) -> &'static str {
    let start = &source_text[span.offset..];
    if start.starts_with("mutation") {
        "mutation"
    } else if start.starts_with("subscription") {
        "subscription"
    } else {
        "query"
    }
}

fn display_kind_label(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "type",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "interface",
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "enum",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "scalar",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => {
            "input"
        }
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "union",
        SyntaxKind::DIRECTIVE_DEFINITION => "directive",
        SyntaxKind::SCHEMA_DEFINITION | SyntaxKind::SCHEMA_EXTENSION => "schema",
        SyntaxKind::FIELD_DEFINITION => "field",
        SyntaxKind::INPUT_VALUE_DEFINITION => "input value",
        SyntaxKind::ENUM_VALUE_DEFINITION => "enum value",
        SyntaxKind::OPERATION_DEFINITION => "operation",
        _ => "",
    }
}

fn display_node_name(kind: SyntaxKind, name: &Option<String>) -> String {
    let label = display_kind_label(kind);
    match name {
        Some(n) => format!("{label} \"{n}\""),
        None => label.to_owned(),
    }
}

fn enum_value_display(name: &Option<String>) -> String {
    match name {
        Some(n) => format!("enum value \"{n}\""),
        None => "enum value".to_owned(),
    }
}

fn get_node_name(
    parent_kind: SyntaxKind,
    parent_name: &Option<String>,
    container_kind: SyntaxKind,
    container_name: &Option<String>,
) -> String {
    match parent_kind {
        SyntaxKind::FIELD_DEFINITION | SyntaxKind::INPUT_VALUE_DEFINITION => {
            let self_str = display_node_name(parent_kind, parent_name);
            let container_str = display_node_name(container_kind, container_name);
            format!("{self_str} in {container_str}")
        }
        _ => display_node_name(parent_kind, parent_name),
    }
}

fn name_span(source_text: &str, _kind: SyntaxKind, name: &Option<String>, span: Span) -> Span {
    if let Some(n) = name {
        if span.offset + span.len <= source_text.len() {
            let node_text = &source_text[span.offset..span.end()];
            if let Some(offset) = node_text.find(n.as_str()) {
                return Span::new(span.offset + offset, n.len());
            }
        }
    }
    span
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_025() {
        let rule = RequireDescription;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-description");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn option_deserializes_from_json() {
        let opts: Opts = serde_json::from_value(serde_json::json!({"ObjectTypeDefinition": true}))
            .unwrap_or_default();
        assert!(opts.is_enabled(SyntaxKind::OBJECT_TYPE_DEFINITION));
        assert!(!opts.is_enabled(SyntaxKind::INTERFACE_TYPE_DEFINITION));

        let opts: Opts = serde_json::from_value(
            serde_json::json!({"types": true, "ObjectTypeDefinition": false}),
        )
        .unwrap_or_default();
        assert!(!opts.is_enabled(SyntaxKind::OBJECT_TYPE_DEFINITION));
        assert!(opts.is_enabled(SyntaxKind::INTERFACE_TYPE_DEFINITION));
        assert!(opts.is_enabled(SyntaxKind::ENUM_TYPE_DEFINITION));
    }

    #[test]
    fn option_deserializes_operation() {
        let opts: Opts = serde_json::from_value(serde_json::json!({"OperationDefinition": true}))
            .unwrap_or_default();
        assert!(opts.is_enabled(SyntaxKind::OPERATION_DEFINITION));
        assert!(!opts.is_enabled(SyntaxKind::FIELD_DEFINITION));
    }
}
