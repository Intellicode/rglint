//! `no-scalar-result-type-on-mutation` (spec-037).

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    allowed: Vec<String>,
}

#[derive(Rule)]
#[rule(
    id = "no-scalar-result-type-on-mutation",
    category = "schema",
    requires_schema = true,
    kinds = "NAMED_TYPE"
)]
pub struct NoScalarResultTypeOnMutation;

impl NoScalarResultTypeOnMutation {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoScalarResultTypeOnMutationHandler {
            candidates: Vec::new(),
        })
    }
}

struct Candidate {
    field_name: String,
    type_name: String,
    base_type_name: String,
    span: Span,
}

struct NoScalarResultTypeOnMutationHandler {
    candidates: Vec<Candidate>,
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

fn find_type_def<'a>(field_def: &'a Node<'a>) -> Option<&'a Node<'a>> {
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

fn resolve_base_type_name(ty: &apollo_compiler::ast::Type) -> Option<&str> {
    match ty {
        apollo_compiler::ast::Type::Named(name)
        | apollo_compiler::ast::Type::NonNullNamed(name) => Some(name.as_str()),
        apollo_compiler::ast::Type::List(inner)
        | apollo_compiler::ast::Type::NonNullList(inner) => resolve_base_type_name(inner),
    }
}

impl Handler for NoScalarResultTypeOnMutationHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let base_type_name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };

        let field_def = match find_field_def(node) {
            Some(fd) => fd,
            None => return,
        };
        let field_name = match &field_def.name {
            Some(n) => n.clone(),
            None => return,
        };

        let type_def = match find_type_def(field_def) {
            Some(td) => td,
            None => return,
        };
        let type_name = match &type_def.name {
            Some(n) => n.clone(),
            None => return,
        };

        self.candidates.push(Candidate {
            field_name,
            type_name,
            base_type_name,
            span,
        });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(s) => s,
            None => return,
        };

        let opts: Opts = ctx.option().unwrap_or_default();

        let mutation_type_name = schema
            .schema_definition
            .mutation
            .as_ref()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Mutation".into());

        let mutation_obj = match schema.get_object(&mutation_type_name) {
            Some(obj) => obj,
            None => return,
        };

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for c in &self.candidates {
            if c.type_name != mutation_type_name {
                continue;
            }

            if mutation_obj.fields.get(c.field_name.as_str()).is_none() {
                continue;
            }

            if opts.allowed.iter().any(|a| a == &c.base_type_name) {
                continue;
            }

            if !is_scalar_type(c.base_type_name.as_str(), schema) {
                continue;
            }

            ctx.report(DiagnosticBuilder::new(
                rule_id,
                path.clone(),
                c.span,
                format!(
                    "Unexpected scalar result type `{}` for field \"{}\" in type \"{}\"",
                    c.base_type_name, c.field_name, c.type_name
                ),
            ));
        }
    }
}

fn is_scalar_type(type_name: &str, schema: &apollo_compiler::Schema) -> bool {
    match type_name {
        "String" | "Int" | "Float" | "Boolean" | "ID" => return true,
        _ => {}
    }
    match schema.types.get(type_name) {
        Some(ext_type) => matches!(ext_type, apollo_compiler::schema::ExtendedType::Scalar(_)),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_037() {
        let rule = NoScalarResultTypeOnMutation;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-scalar-result-type-on-mutation");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn option_deserializes_empty() {
        let opts: Opts =
            serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert!(opts.allowed.is_empty());
    }

    #[test]
    fn option_deserializes_allowed() {
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "allowed": ["String", "ID"]
        }))
        .unwrap_or_default();
        assert_eq!(opts.allowed, vec!["String", "ID"]);
    }
}
