//! `relay-arguments` (spec-045).

use std::collections::HashMap;
use std::ops::Deref;

use apollo_compiler::{ast, schema, Schema};
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

use crate::shared::RelayOpts;

const MISSING_ARGUMENTS: &str = "A field that returns a Connection type must include forward pagination arguments (`first` and `after`), backward pagination arguments (`last` and `before`), or both.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_include_both")]
    include_both: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            include_both: default_include_both(),
        }
    }
}

fn default_include_both() -> bool {
    true
}

#[derive(Rule)]
#[rule(
    id = "relay-arguments",
    category = "schema",
    requires_schema = true,
    kinds = "FIELD_DEFINITION|INPUT_VALUE_DEFINITION"
)]
pub struct RelayArguments;

impl RelayArguments {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RelayArgumentsHandler {
            opts: ctx.option().unwrap_or_default(),
            relay_opts: RelayOpts::default(),
            fields: Vec::new(),
        })
    }
}

struct RelayArgumentsHandler {
    opts: Opts,
    relay_opts: RelayOpts,
    fields: Vec<FieldCandidate>,
}

struct FieldCandidate {
    type_name: String,
    field_name: String,
    field_span: Span,
    argument_spans: HashMap<String, Span>,
}

impl Handler for RelayArgumentsHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let Some(name) = node.name.clone() else {
            return;
        };
        let Some(span) = node.span else {
            return;
        };
        match node.kind {
            SyntaxKind::FIELD_DEFINITION => {
                let Some(type_name) = containing_type_name(node) else {
                    return;
                };
                self.fields.push(FieldCandidate {
                    type_name,
                    field_name: name,
                    field_span: span,
                    argument_spans: HashMap::new(),
                });
            }
            SyntaxKind::INPUT_VALUE_DEFINITION => {
                if !is_field_argument(node) {
                    return;
                }
                let Some(field_definition) = field_definition_ancestor(node) else {
                    return;
                };
                let Some(type_name) = containing_type_name(field_definition) else {
                    return;
                };
                let Some(field_name) = field_definition.name.clone() else {
                    return;
                };

                if let Some(field) =
                    self.fields.iter_mut().rev().find(|field| {
                        field.type_name == type_name && field.field_name == field_name
                    })
                {
                    field.argument_spans.insert(name, span);
                }
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(schema) = ctx.schema else {
            return;
        };
        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for candidate in &self.fields {
            let Some(field_definition) =
                schema_field(schema, &candidate.type_name, &candidate.field_name)
            else {
                continue;
            };

            // The upstream selector is intentionally name-based: the
            // connection shape itself belongs to relay-connection-types.
            if !self
                .relay_opts
                .connection_pattern
                .is_match(field_definition.ty.inner_named_type().as_str())
            {
                continue;
            }

            let has_arg = |name: &str| candidate.argument_spans.contains_key(name);
            let has_forward = has_arg("first") && has_arg("after");
            let has_backward = has_arg("last") && has_arg("before");

            if !has_forward && !has_backward {
                report(ctx, rule_id, &path, candidate.field_span, MISSING_ARGUMENTS);
                continue;
            }

            if self.opts.include_both || has_arg("first") || has_arg("after") {
                check_argument(ctx, schema, field_definition, candidate, "Int", "first");
                check_argument(ctx, schema, field_definition, candidate, "String", "after");
            }
            if self.opts.include_both || has_arg("last") || has_arg("before") {
                check_argument(ctx, schema, field_definition, candidate, "Int", "last");
                check_argument(ctx, schema, field_definition, candidate, "String", "before");
            }
        }
    }
}

fn containing_type_name(node: &Node<'_>) -> Option<String> {
    let mut current = node.parent;
    while let Some(parent) = current {
        if matches!(
            parent.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION
                | SyntaxKind::OBJECT_TYPE_EXTENSION
                | SyntaxKind::INTERFACE_TYPE_DEFINITION
                | SyntaxKind::INTERFACE_TYPE_EXTENSION
        ) {
            return parent.name.clone();
        }
        current = parent.parent;
    }
    None
}

fn field_definition_ancestor<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent;
    while let Some(parent) = current {
        if parent.kind == SyntaxKind::FIELD_DEFINITION {
            return Some(parent);
        }
        current = parent.parent;
    }
    None
}

fn is_field_argument(argument: &Node<'_>) -> bool {
    argument.parent.is_some_and(|arguments| {
        arguments.kind == SyntaxKind::ARGUMENTS_DEFINITION
            && arguments
                .parent
                .is_some_and(|field| field.kind == SyntaxKind::FIELD_DEFINITION)
    })
}

fn schema_field<'a>(
    schema: &'a Schema,
    type_name: &str,
    field_name: &str,
) -> Option<&'a ast::FieldDefinition> {
    match schema.types.get(type_name)? {
        schema::ExtendedType::Object(object) => object
            .fields
            .get(field_name)
            .map(|field| field.deref().deref()),
        schema::ExtendedType::Interface(interface) => interface
            .fields
            .get(field_name)
            .map(|field| field.deref().deref()),
        _ => None,
    }
}

fn check_argument(
    ctx: &mut RuleContext,
    schema: &Schema,
    field: &ast::FieldDefinition,
    candidate: &FieldCandidate,
    expected: &str,
    argument_name: &str,
) {
    let present = candidate.argument_spans.contains_key(argument_name);
    let argument = field.arguments.iter().find(|arg| arg.name == argument_name);
    let valid =
        present && argument.is_some_and(|arg| argument_type_allowed(schema, &arg.ty, expected));
    if valid {
        return;
    }

    let return_type = if expected == "String" {
        "String or Scalar"
    } else {
        expected
    };
    let (span, message) = match argument {
        _ if present => (
            candidate
                .argument_spans
                .get(argument_name)
                .copied()
                .unwrap_or(candidate.field_span),
            format!("Argument `{argument_name}` must return {return_type}."),
        ),
        _ => (
            candidate.field_span,
            format!(
                "Field `{}` must contain an argument `{argument_name}`, that return {return_type}.",
                candidate.field_name
            ),
        ),
    };
    let rule_id = ctx.rule_id();
    let path = ctx.file.path().to_path_buf();
    report(ctx, rule_id, &path, span, message);
}

fn argument_type_allowed(schema: &Schema, ty: &ast::Type, expected: &str) -> bool {
    let Some(type_name) = named_type(ty) else {
        return false;
    };
    if expected == "Int" {
        return type_name == "Int";
    }
    type_name == "String" || is_scalar_type(schema, type_name)
}

fn named_type(ty: &ast::Type) -> Option<&str> {
    match ty {
        ast::Type::Named(name) | ast::Type::NonNullNamed(name) => Some(name.as_str()),
        ast::Type::List(_) | ast::Type::NonNullList(_) => None,
    }
}

fn is_scalar_type(schema: &Schema, type_name: &str) -> bool {
    matches!(type_name, "String" | "Int" | "Float" | "Boolean" | "ID")
        || matches!(
            schema.types.get(type_name),
            Some(schema::ExtendedType::Scalar(_))
        )
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
    fn rule_meta_matches_spec_045() {
        let rule = RelayArguments;
        let meta = rule.meta();
        assert_eq!(meta.id, "relay-arguments");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn default_options_require_both_directions() {
        assert!(Opts::default().include_both);
        let int = apollo_compiler::Name::new("Int").unwrap();
        assert_eq!(
            named_type(&ast::Type::NonNullNamed(int.clone())),
            Some("Int")
        );
        assert!(named_type(&ast::Type::List(Box::new(ast::Type::Named(int)))).is_none());
    }
}
