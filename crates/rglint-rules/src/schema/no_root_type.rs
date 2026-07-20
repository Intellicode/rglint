//! `no-root-type` (spec-032).

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span};
use rglint_derive::Rule;
use serde::Deserialize;

fn default_forbidden() -> Vec<RootKind> {
    vec![RootKind::Query, RootKind::Mutation, RootKind::Subscription]
}

#[derive(Debug, Clone, Deserialize)]
enum RootKind {
    Query,
    Mutation,
    Subscription,
}

impl RootKind {
    fn type_name(&self, schema: Option<&apollo_compiler::Schema>) -> String {
        let schema = match schema {
            Some(s) => s,
            None => return self.default_name(),
        };
        let name = match self {
            RootKind::Query => schema.schema_definition.query.as_ref(),
            RootKind::Mutation => schema.schema_definition.mutation.as_ref(),
            RootKind::Subscription => schema.schema_definition.subscription.as_ref(),
        };
        name.map(|n| n.as_str().to_owned())
            .unwrap_or_else(|| self.default_name())
    }

    fn default_name(&self) -> String {
        match self {
            RootKind::Query => "Query",
            RootKind::Mutation => "Mutation",
            RootKind::Subscription => "Subscription",
        }
        .to_owned()
    }
}

#[derive(Debug, Deserialize)]
struct Opts {
    #[serde(default = "default_forbidden")]
    forbidden: Vec<RootKind>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            forbidden: default_forbidden(),
        }
    }
}

struct TypeDef {
    name: String,
    span: Span,
}

#[derive(Rule)]
#[rule(
    id = "no-root-type",
    category = "schema",
    requires_schema = true,
    kinds = "OBJECT_TYPE_DEFINITION|OBJECT_TYPE_EXTENSION"
)]
pub struct NoRootType;

impl NoRootType {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(NoRootTypeHandler {
            opts,
            type_defs: Vec::new(),
        })
    }
}

struct NoRootTypeHandler {
    opts: Opts,
    type_defs: Vec<TypeDef>,
}

impl Handler for NoRootTypeHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };
        self.type_defs.push(TypeDef { name, span });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();

        for kind in &self.opts.forbidden {
            let type_name = kind.type_name(ctx.schema);
            if let Some(def) = self.type_defs.iter().find(|d| d.name == type_name) {
                let message = format!("Root type \"{}\" is forbidden", def.name);
                ctx.report(DiagnosticBuilder::new(
                    rule_id,
                    path.clone(),
                    def.span,
                    message,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_032() {
        let rule = NoRootType;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-root-type");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn option_deserializes_with_defaults() {
        let opts: Opts =
            serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert_eq!(opts.forbidden.len(), 3);
    }

    #[test]
    fn option_deserializes_partial_forbidden() {
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "forbidden": ["Query"]
        }))
        .unwrap_or_default();
        assert_eq!(opts.forbidden.len(), 1);
    }
}
