//! `require-field-of-type-query-in-mutation-result` (spec-039).

use std::ops::Deref;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

fn default_query() -> String {
    "Query".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_query")]
    query_type_name: String,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            query_type_name: default_query(),
        }
    }
}

#[derive(Rule)]
#[rule(
    id = "require-field-of-type-query-in-mutation-result",
    category = "schema",
    requires_schema = true,
    kinds = "NAMED_TYPE"
)]
pub struct RequireFieldOfTypeQueryInMutationResult;

impl RequireFieldOfTypeQueryInMutationResult {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(RequireFieldOfTypeQueryInMutationResultHandler {
            opts,
            candidates: Vec::new(),
        })
    }
}

struct Candidate {
    field_name: String,
    container_type_name: String,
    result_type_name: String,
    span: Span,
}

struct RequireFieldOfTypeQueryInMutationResultHandler {
    opts: Opts,
    candidates: Vec<Candidate>,
}

impl Handler for RequireFieldOfTypeQueryInMutationResultHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let result_type_name = match &node.name {
            Some(name) => name.clone(),
            None => return,
        };
        let span = match node.span {
            Some(span) => span,
            None => return,
        };
        let field_def = match find_field_def(node) {
            Some(field_def) => field_def,
            None => return,
        };
        let field_name = match &field_def.name {
            Some(name) => name.clone(),
            None => return,
        };
        let type_def = match find_object_type_def(field_def) {
            Some(type_def) => type_def,
            None => return,
        };
        let container_type_name = match &type_def.name {
            Some(name) => name.clone(),
            None => return,
        };

        self.candidates.push(Candidate {
            field_name,
            container_type_name,
            result_type_name,
            span,
        });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(schema) => schema,
            None => return,
        };
        let mutation_type_name = schema
            .schema_definition
            .mutation
            .as_ref()
            .map(|name| name.as_str().to_owned())
            .unwrap_or_else(|| "Mutation".to_owned());
        let Some(mutation_type) = schema.get_object(mutation_type_name.as_str()) else {
            return;
        };
        let query_type_name = schema
            .schema_definition
            .query
            .as_ref()
            .map(|name| name.as_str().to_owned())
            .unwrap_or_else(|| self.opts.query_type_name.clone());
        if schema.get_object(query_type_name.as_str()).is_none() {
            return;
        }

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for candidate in &self.candidates {
            if candidate.container_type_name != mutation_type_name {
                continue;
            }
            if mutation_type
                .fields
                .get(candidate.field_name.as_str())
                .is_none()
            {
                continue;
            }
            let Some(result_type) = schema.get_object(candidate.result_type_name.as_str()) else {
                continue;
            };
            if object_has_field_of_type(result_type, query_type_name.as_str()) {
                continue;
            }

            ctx.report(DiagnosticBuilder::new(
                rule_id,
                path.clone(),
                candidate.span,
                format!(
                    "Mutation result type \"{}\" must contain field of type \"{}\"",
                    candidate.result_type_name, query_type_name
                ),
            ));
        }
    }
}

fn find_field_def<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent?;
    loop {
        if current.kind == SyntaxKind::INPUT_VALUE_DEFINITION {
            return None;
        }
        if current.kind == SyntaxKind::FIELD_DEFINITION {
            return Some(current);
        }
        current = current.parent?;
    }
}

fn find_object_type_def<'a>(field_def: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = field_def.parent?;
    loop {
        if matches!(
            current.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return Some(current);
        }
        current = current.parent?;
    }
}

fn object_has_field_of_type(
    object_type: &apollo_compiler::schema::ObjectType,
    query_type_name: &str,
) -> bool {
    object_type.fields.values().any(|field| {
        resolve_base_type_name(&field.deref().deref().ty)
            .is_some_and(|type_name| type_name == query_type_name)
    })
}

fn resolve_base_type_name(ty: &apollo_compiler::ast::Type) -> Option<&str> {
    match ty {
        apollo_compiler::ast::Type::Named(name)
        | apollo_compiler::ast::Type::NonNullNamed(name) => Some(name.as_str()),
        apollo_compiler::ast::Type::List(inner)
        | apollo_compiler::ast::Type::NonNullList(inner) => resolve_base_type_name(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_039() {
        let rule = RequireFieldOfTypeQueryInMutationResult;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-field-of-type-query-in-mutation-result");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn option_deserializes_default_query_type_name() {
        let opts: Opts = serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert_eq!(opts.query_type_name, "Query");
    }

    #[test]
    fn option_deserializes_custom_query_type_name() {
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "queryTypeName": "RootQuery"
        }))
        .unwrap_or_default();
        assert_eq!(opts.query_type_name, "RootQuery");
    }
}
