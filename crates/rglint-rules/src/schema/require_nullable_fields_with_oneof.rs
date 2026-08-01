//! `require-nullable-fields-with-oneof` (spec-050).

use std::ops::Deref;

use apollo_compiler::{ast, schema::ExtendedType, Schema};
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "require-nullable-fields-with-oneof",
    category = "schema",
    requires_schema = true,
    kinds = "DIRECTIVE|NAME"
)]
pub struct RequireNullableFieldsWithOneof;

impl RequireNullableFieldsWithOneof {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RequireNullableFieldsWithOneofHandler {
            one_of_definitions: Vec::new(),
            fields: Vec::new(),
        })
    }
}

#[derive(Clone, Copy)]
struct Definition {
    span: Span,
    kind: SyntaxKind,
}

struct FieldCandidate {
    definition: Definition,
    container_name: String,
    field_name: String,
    name_span: Span,
}

struct RequireNullableFieldsWithOneofHandler {
    one_of_definitions: Vec<Definition>,
    fields: Vec<FieldCandidate>,
}

impl Handler for RequireNullableFieldsWithOneofHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::DIRECTIVE if node.name.as_deref() == Some("oneOf") => {
                let Some(definition) = find_container(node) else {
                    return;
                };
                if let (Some(span), Some(kind)) = (definition.span, one_of_kind(definition.kind)) {
                    self.one_of_definitions.push(Definition { span, kind });
                }
            }
            SyntaxKind::NAME => {
                let Some(parent) = parent else { return };
                if !matches!(
                    parent.kind,
                    SyntaxKind::FIELD_DEFINITION | SyntaxKind::INPUT_VALUE_DEFINITION
                ) {
                    return;
                }
                let Some(definition_node) = find_container(parent) else {
                    return;
                };
                let Some(definition_span) = definition_node.span else {
                    return;
                };
                let Some(kind) = one_of_kind(definition_node.kind) else {
                    return;
                };
                // INPUT_VALUE_DEFINITION also represents field arguments. Only
                // input-object fields belong to this rule's `input` branch.
                if parent.kind == SyntaxKind::INPUT_VALUE_DEFINITION
                    && kind != SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
                {
                    return;
                }
                let Some(container_name) = definition_node.name.clone() else {
                    return;
                };
                let Some(name_span) = node.span else { return };
                self.fields.push(FieldCandidate {
                    definition: Definition {
                        span: definition_span,
                        kind,
                    },
                    container_name,
                    field_name: parent.name.clone().unwrap_or_default(),
                    name_span,
                });
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(schema) = ctx.schema else { return };
        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for field in &self.fields {
            if !self
                .one_of_definitions
                .iter()
                .any(|definition| definition.span == field.definition.span)
            {
                continue;
            }
            let Some(field_type) = schema_field_type(
                schema,
                field.definition.kind,
                &field.container_name,
                &field.field_name,
            ) else {
                continue;
            };
            if !is_outer_non_null(field_type) {
                continue;
            }

            let container_kind = match field.definition.kind {
                SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION => "input",
                SyntaxKind::OBJECT_TYPE_DEFINITION => "type",
                _ => continue,
            };
            let message = format!(
                "field \"{}\" in {} \"{}\" must be nullable when \"@oneOf\" is in use",
                field.field_name, container_kind, field.container_name
            );
            ctx.report(DiagnosticBuilder::new(
                rule_id,
                path.clone(),
                field.name_span,
                message,
            ));
        }
    }
}

fn one_of_kind(kind: SyntaxKind) -> Option<SyntaxKind> {
    match kind {
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_DEFINITION => Some(kind),
        _ => None,
    }
}

fn find_container<'a>(start: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = Some(start);
    while let Some(node) = current {
        if one_of_kind(node.kind).is_some() {
            return Some(node);
        }
        current = node.parent;
    }
    None
}

fn schema_field_type<'a>(
    schema: &'a Schema,
    kind: SyntaxKind,
    container_name: &str,
    field_name: &str,
) -> Option<&'a ast::Type> {
    match (kind, schema.types.get(container_name)?) {
        (SyntaxKind::OBJECT_TYPE_DEFINITION, ExtendedType::Object(object)) => object
            .fields
            .get(field_name)
            .map(|field| &field.deref().deref().ty),
        (SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION, ExtendedType::InputObject(input)) => input
            .fields
            .get(field_name)
            .map(|field| field.deref().deref().ty.deref()),
        _ => None,
    }
}

fn is_outer_non_null(ty: &ast::Type) -> bool {
    matches!(ty, ast::Type::NonNullNamed(_) | ast::Type::NonNullList(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_050() {
        let rule = RequireNullableFieldsWithOneof;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-nullable-fields-with-oneof");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn only_outer_non_null_is_rejected() {
        let string = || apollo_compiler::Name::new("String").unwrap();
        assert!(is_outer_non_null(&ast::Type::NonNullNamed(string())));
        assert!(is_outer_non_null(&ast::Type::NonNullList(Box::new(
            ast::Type::Named(string()),
        ))));
        assert!(!is_outer_non_null(&ast::Type::List(Box::new(
            ast::Type::NonNullNamed(string()),
        ))));
    }
}
