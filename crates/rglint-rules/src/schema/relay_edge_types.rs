//! `relay-edge-types` (spec-047).

use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use apollo_compiler::{ast, schema, Schema};
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

const NODE_TYPE_MESSAGE: &str = "either a Scalar, Enum, Object, Interface, Union, or a non-null wrapper around one of those types.";
const CURSOR_TYPE_MESSAGE: &str =
    "either a String, Scalar, or a non-null wrapper wrapper around one of those types.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_true")]
    with_edge_suffix: bool,
    #[serde(default = "default_true")]
    should_implement_node: bool,
    #[serde(default = "default_true")]
    list_type_can_wrap_only_edge_type: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            with_edge_suffix: true,
            should_implement_node: true,
            list_type_can_wrap_only_edge_type: true,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Rule)]
#[rule(
    id = "relay-edge-types",
    category = "schema",
    requires_schema = true,
    option_schema = r#"{
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "withEdgeSuffix": {"type": "boolean", "default": true},
        "shouldImplementNode": {"type": "boolean", "default": true},
        "listTypeCanWrapOnlyEdgeType": {"type": "boolean", "default": true}
      }
    }"#,
    default_options = r#"{
      "withEdgeSuffix": true,
      "shouldImplementNode": true,
      "listTypeCanWrapOnlyEdgeType": true
    }"#,
    kinds = "NAME|OBJECT_TYPE_DEFINITION|OBJECT_TYPE_EXTENSION|FIELD_DEFINITION|NAMED_TYPE|LIST_TYPE|NON_NULL_TYPE"
)]
pub struct RelayEdgeTypes;

impl RelayEdgeTypes {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RelayEdgeTypesHandler {
            opts: ctx.option().unwrap_or_default(),
            definitions: Vec::new(),
            fields: HashMap::new(),
            edge_references: Vec::new(),
            list_fields: Vec::new(),
        })
    }
}

struct RelayEdgeTypesHandler {
    opts: Opts,
    definitions: Vec<Definition>,
    fields: HashMap<Span, Vec<Field>>,
    edge_references: Vec<TypeReference>,
    list_fields: Vec<ListField>,
}

struct Definition {
    name: String,
    definition_span: Span,
    name_span: Span,
}

struct Field {
    name: String,
    span: Span,
}

struct TypeReference {
    type_name: String,
    span: Span,
}

struct ListField {
    object_name: String,
    field_name: String,
    span: Span,
}

impl Handler for RelayEdgeTypesHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => {
                self.record_definition(node);
            }
            SyntaxKind::NAME => self.record_definition_name(node),
            SyntaxKind::FIELD_DEFINITION => self.record_field(node),
            SyntaxKind::NAMED_TYPE => self.record_edge_reference(node),
            SyntaxKind::LIST_TYPE | SyntaxKind::NON_NULL_TYPE => self.record_list_field(node),
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(schema) = ctx.schema else { return };
        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();
        let edge_types = self.edge_types(schema);

        // This is the listener that reports a non-object type used by a
        // Connection's edges field. Keep the named-type span: graphql-eslint
        // points at AEdge, not at the surrounding list punctuation.
        for reference in &self.edge_references {
            if !is_object_type(schema, &reference.type_name) {
                report(
                    ctx,
                    rule_id,
                    &path,
                    reference.span,
                    "Edge type must be an Object type.",
                );
            }
        }

        if self.opts.list_type_can_wrap_only_edge_type {
            for field in &self.list_fields {
                let Some(definition) = schema_field(schema, &field.object_name, &field.field_name)
                else {
                    continue;
                };
                if definition.ty.is_list()
                    && !edge_types.contains(definition.ty.inner_named_type().as_str())
                {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        field.span,
                        "A list type should only wrap an edge type.",
                    );
                }
            }
        }

        for definition in &self.definitions {
            if !edge_types.contains(&definition.name) {
                continue;
            }

            let fields = self.fields.get(&definition.definition_span);
            check_node_field(ctx, schema, &path, rule_id, definition, fields, &self.opts);
            check_cursor_field(ctx, schema, &path, rule_id, definition, fields);

            if self.opts.with_edge_suffix && !definition.name.ends_with("Edge") {
                report(
                    ctx,
                    rule_id,
                    &path,
                    definition.name_span,
                    "Edge type must have \"Edge\" suffix.",
                );
            }
        }
    }
}

impl RelayEdgeTypesHandler {
    fn record_definition(&mut self, node: &Node<'_>) {
        let (Some(name), Some(span)) = (node.name.clone(), node.span) else {
            return;
        };
        self.definitions.push(Definition {
            name,
            definition_span: span,
            name_span: span,
        });
    }

    fn record_definition_name(&mut self, node: &Node<'_>) {
        let Some(parent) = node.parent else { return };
        if !matches!(
            parent.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return;
        }
        let (Some(parent_span), Some(name_span)) = (parent.span, node.span) else {
            return;
        };
        if let Some(definition) = self
            .definitions
            .iter_mut()
            .find(|definition| definition.definition_span == parent_span)
        {
            definition.name_span = name_span;
        }
    }

    fn record_field(&mut self, node: &Node<'_>) {
        let (Some(name), Some(span), Some(object)) =
            (node.name.clone(), node.span, containing_object(node))
        else {
            return;
        };
        let Some(object_span) = object.span else {
            return;
        };
        self.fields
            .entry(object_span)
            .or_default()
            .push(Field { name, span });
    }

    fn record_edge_reference(&mut self, node: &Node<'_>) {
        let (Some(type_name), Some(span), Some(field)) =
            (node.name.clone(), node.span, field_ancestor(node))
        else {
            return;
        };
        if field.name.as_deref() != Some("edges") {
            return;
        }
        let Some(object) = containing_object(field) else {
            return;
        };
        if !object
            .name
            .as_deref()
            .is_some_and(|name| name.ends_with("Connection"))
        {
            return;
        }
        self.edge_references.push(TypeReference { type_name, span });
    }

    fn record_list_field(&mut self, node: &Node<'_>) {
        let Some(field) = node
            .parent
            .filter(|p| p.kind == SyntaxKind::FIELD_DEFINITION)
        else {
            return;
        };
        let (Some(object_name), Some(field_name), Some(span), Some(_object)) = (
            containing_type_name(field),
            field.name.clone(),
            node.span,
            containing_object(field),
        ) else {
            return;
        };
        self.list_fields.push(ListField {
            object_name,
            field_name,
            span,
        });
    }

    fn edge_types(&self, schema: &Schema) -> HashSet<String> {
        self.edge_references
            .iter()
            .filter(|reference| is_object_type(schema, &reference.type_name))
            .map(|reference| reference.type_name.clone())
            .collect()
    }
}

fn check_node_field(
    ctx: &mut RuleContext,
    schema: &Schema,
    path: &std::path::Path,
    rule_id: &str,
    definition: &Definition,
    fields: Option<&Vec<Field>>,
    opts: &Opts,
) {
    let Some(node_field) =
        fields.and_then(|fields| fields.iter().find(|field| field.name == "node"))
    else {
        report(
            ctx,
            rule_id,
            path,
            definition.name_span,
            format!("Edge type must contain a field `node` that return {NODE_TYPE_MESSAGE}"),
        );
        return;
    };

    let Some(field) = schema_field(schema, &definition.name, "node") else {
        return;
    };
    if !is_named_or_non_null_named(&field.ty) {
        report(
            ctx,
            rule_id,
            path,
            node_field.span,
            format!("Field `node` must return {NODE_TYPE_MESSAGE}"),
        );
        return;
    }

    if opts.should_implement_node {
        let type_name = field.ty.inner_named_type();
        if let Some(schema::ExtendedType::Object(object)) = schema.types.get(type_name.as_str()) {
            if !object
                .implements_interfaces
                .iter()
                .any(|interface| interface.as_str() == "Node")
            {
                report(
                    ctx,
                    rule_id,
                    path,
                    definition.name_span,
                    "Edge type's field `node` must implement `Node` interface.",
                );
            }
        }
    }
}

fn check_cursor_field(
    ctx: &mut RuleContext,
    schema: &Schema,
    path: &std::path::Path,
    rule_id: &str,
    definition: &Definition,
    fields: Option<&Vec<Field>>,
) {
    let Some(cursor_field) =
        fields.and_then(|fields| fields.iter().find(|field| field.name == "cursor"))
    else {
        report(
            ctx,
            rule_id,
            path,
            definition.name_span,
            format!("Edge type must contain a field `cursor` that return {CURSOR_TYPE_MESSAGE}"),
        );
        return;
    };

    let Some(field) = schema_field(schema, &definition.name, "cursor") else {
        return;
    };
    let type_name = field.ty.inner_named_type();
    if !is_named_or_non_null_named(&field.ty)
        || (type_name != "String" && !is_scalar_type(schema, type_name.as_str()))
    {
        report(
            ctx,
            rule_id,
            path,
            cursor_field.span,
            format!("Field `cursor` must return {CURSOR_TYPE_MESSAGE}"),
        );
    }
}

fn containing_object<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent;
    while let Some(parent) = current {
        if matches!(
            parent.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return Some(parent);
        }
        current = parent.parent;
    }
    None
}

fn containing_type_name(node: &Node<'_>) -> Option<String> {
    containing_object(node).and_then(|object| object.name.clone())
}

fn field_ancestor<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent;
    while let Some(parent) = current {
        if parent.kind == SyntaxKind::FIELD_DEFINITION {
            return Some(parent);
        }
        current = parent.parent;
    }
    None
}

fn schema_field<'a>(
    schema: &'a Schema,
    object_name: &str,
    field_name: &str,
) -> Option<&'a ast::FieldDefinition> {
    match schema.types.get(object_name)? {
        schema::ExtendedType::Object(object) => object
            .fields
            .get(field_name)
            .map(|field| field.deref().deref()),
        _ => None,
    }
}

fn is_object_type(schema: &Schema, type_name: &str) -> bool {
    matches!(
        schema.types.get(type_name),
        Some(schema::ExtendedType::Object(_))
    )
}

fn is_scalar_type(schema: &Schema, type_name: &str) -> bool {
    matches!(type_name, "String" | "Int" | "Float" | "Boolean" | "ID")
        || matches!(
            schema.types.get(type_name),
            Some(schema::ExtendedType::Scalar(_))
        )
}

fn is_named_or_non_null_named(ty: &ast::Type) -> bool {
    matches!(ty, ast::Type::Named(_) | ast::Type::NonNullNamed(_))
}

fn report(
    ctx: &mut RuleContext,
    rule_id: &str,
    path: &std::path::Path,
    span: Span,
    message: impl Into<String>,
) {
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        span,
        message,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_047() {
        let rule = RelayEdgeTypes;
        let meta = rule.meta();
        assert_eq!(meta.id, "relay-edge-types");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn options_default_to_upstream_values() {
        let opts: Opts = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(opts.with_edge_suffix);
        assert!(opts.should_implement_node);
        assert!(opts.list_type_can_wrap_only_edge_type);
    }

    #[test]
    fn node_accepts_only_named_and_non_null_named_types() {
        let name = apollo_compiler::Name::new("User").unwrap();
        assert!(is_named_or_non_null_named(&ast::Type::Named(name.clone())));
        assert!(is_named_or_non_null_named(&ast::Type::NonNullNamed(name)));
        assert!(!is_named_or_non_null_named(&ast::Type::List(Box::new(
            ast::Type::Named(apollo_compiler::Name::new("User").unwrap()),
        ))));
    }
}
