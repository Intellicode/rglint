//! `require-nullable-result-in-root` (spec-038).

use std::collections::HashSet;

use apollo_compiler::schema::ExtendedType;
use rglint_core::{DiagnosticBuilder, Fix, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

fn all_roots() -> Vec<RootKind> {
    vec![RootKind::Query, RootKind::Mutation, RootKind::Subscription]
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
enum RootKind {
    Query,
    Mutation,
    Subscription,
}

impl RootKind {
    fn type_name(&self, schema: &apollo_compiler::Schema) -> String {
        let configured = match self {
            RootKind::Query => schema.schema_definition.query.as_ref(),
            RootKind::Mutation => schema.schema_definition.mutation.as_ref(),
            RootKind::Subscription => schema.schema_definition.subscription.as_ref(),
        };
        configured
            .map(|n| n.as_str().to_owned())
            .unwrap_or_else(|| self.default_name().to_owned())
    }

    fn default_name(&self) -> &'static str {
        match self {
            RootKind::Query => "Query",
            RootKind::Mutation => "Mutation",
            RootKind::Subscription => "Subscription",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "all_roots")]
    root: Vec<RootKind>,
}

impl Default for Opts {
    fn default() -> Self {
        Self { root: all_roots() }
    }
}

#[derive(Rule)]
#[rule(
    id = "require-nullable-result-in-root",
    category = "schema",
    requires_schema = true,
    has_suggestions = true,
    kinds = "NAMED_TYPE"
)]
pub struct RequireNullableResultInRoot;

impl RequireNullableResultInRoot {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(RequireNullableResultInRootHandler {
            opts,
            candidates: Vec::new(),
        })
    }
}

struct Candidate {
    root_type_name: String,
    result_type_name: String,
    span: Span,
}

struct RequireNullableResultInRootHandler {
    opts: Opts,
    candidates: Vec<Candidate>,
}

impl Handler for RequireNullableResultInRootHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let result_type_name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };
        let field_def = match direct_non_null_field_def(node) {
            Some(fd) => fd,
            None => return,
        };
        let type_def = match find_object_type_def(field_def) {
            Some(td) => td,
            None => return,
        };
        let root_type_name = match &type_def.name {
            Some(n) => n.clone(),
            None => return,
        };

        self.candidates.push(Candidate {
            root_type_name,
            result_type_name,
            span,
        });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(s) => s,
            None => return,
        };

        let root_names: HashSet<String> = self
            .opts
            .root
            .iter()
            .map(|kind| kind.type_name(schema))
            .collect();
        if root_names.is_empty() {
            return;
        }

        let source = ctx.source_code().source().to_owned();
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();

        for c in &self.candidates {
            if !root_names.contains(c.root_type_name.as_str()) {
                continue;
            }
            let Some(bang_span) = trailing_bang_span(&source, c.span) else {
                continue;
            };

            let kind_label = type_kind_label(c.result_type_name.as_str(), schema);
            let message = format!(
                "Unexpected non-null result {kind_label} \"{}\" in type \"{}\"",
                c.result_type_name, c.root_type_name
            );
            let suggestion = if kind_label == "scalar" {
                format!("Make {} nullable", c.result_type_name)
            } else {
                format!("Make {kind_label} \"{}\" nullable", c.result_type_name)
            };

            ctx.report(
                DiagnosticBuilder::new(rule_id, path.clone(), c.span, message)
                    .suggestion(suggestion, Fix::Remove { span: bang_span }),
            );
        }
    }
}

fn direct_non_null_field_def<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let non_null = node.parent?;
    if non_null.kind != SyntaxKind::NON_NULL_TYPE {
        return None;
    }

    let mut current = non_null.parent?;
    loop {
        match current.kind {
            SyntaxKind::FIELD_DEFINITION => return Some(current),
            SyntaxKind::LIST_TYPE | SyntaxKind::INPUT_VALUE_DEFINITION => return None,
            _ => current = current.parent?,
        }
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

fn trailing_bang_span(source: &str, type_span: Span) -> Option<Span> {
    let tail = source.get(type_span.end()..)?;
    let whitespace_len = tail
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
        .unwrap_or(tail.len());
    let bang_offset = type_span.end() + whitespace_len;
    source
        .as_bytes()
        .get(bang_offset)
        .filter(|b| **b == b'!')
        .map(|_| Span::new(bang_offset, 1))
}

fn type_kind_label(type_name: &str, schema: &apollo_compiler::Schema) -> &'static str {
    match type_name {
        "String" | "Int" | "Float" | "Boolean" | "ID" => return "scalar",
        _ => {}
    }

    match schema.types.get(type_name) {
        Some(ExtendedType::Scalar(_)) => "scalar",
        Some(ExtendedType::Object(_)) => "type",
        Some(ExtendedType::Interface(_)) => "interface",
        Some(ExtendedType::Union(_)) => "union",
        Some(ExtendedType::Enum(_)) => "enum",
        Some(ExtendedType::InputObject(_)) => "input",
        None => "type",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_038() {
        let rule = RequireNullableResultInRoot;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-nullable-result-in-root");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(meta.has_suggestions);
    }

    #[test]
    fn option_deserializes_default_roots() {
        let opts: Opts = serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert_eq!(opts.root.len(), 3);
    }

    #[test]
    fn option_deserializes_selected_roots() {
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "root": ["Mutation"]
        }))
        .unwrap_or_default();
        assert_eq!(opts.root, vec![RootKind::Mutation]);
    }

    #[test]
    fn trailing_bang_allows_space_before_bang() {
        let source = "type Query { user: User ! }";
        let span = Span::new(19, 4);
        assert_eq!(trailing_bang_span(source, span), Some(Span::new(24, 1)));
    }
}
