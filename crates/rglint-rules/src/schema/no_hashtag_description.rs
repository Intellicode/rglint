use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span};
use rglint_derive::Rule;

use crate::shared::comment_scanner;

#[derive(Rule)]
#[rule(
    id = "no-hashtag-description",
    category = "schema",
    kinds = "OBJECT_TYPE_DEFINITION|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_DEFINITION|INTERFACE_TYPE_EXTENSION|INPUT_OBJECT_TYPE_DEFINITION|INPUT_OBJECT_TYPE_EXTENSION|ENUM_TYPE_DEFINITION|ENUM_TYPE_EXTENSION|SCALAR_TYPE_DEFINITION|SCALAR_TYPE_EXTENSION|UNION_TYPE_DEFINITION|UNION_TYPE_EXTENSION|FIELD_DEFINITION|INPUT_VALUE_DEFINITION|ENUM_VALUE_DEFINITION|DIRECTIVE_DEFINITION|SCHEMA_DEFINITION|SCHEMA_EXTENSION"
)]
pub struct NoHashtagDescription;

impl NoHashtagDescription {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoHashtagDescriptionHandler {
            spans: Vec::new(),
        })
    }
}

struct NoHashtagDescriptionHandler {
    spans: Vec<Span>,
}

impl Handler for NoHashtagDescriptionHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        if let Some(span) = node.span {
            self.spans.push(span);
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let source = ctx.source_code();
        let path = source.path().to_path_buf();
        let results: Vec<Span> = self
            .spans
            .iter()
            .filter_map(|&node_span| {
                let comments = comment_scanner::preceding_comments(source, node_span);
                comments.first().map(|c| c.span)
            })
            .collect();
        for comment_span in results {
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                path.clone(),
                comment_span,
                "Use \"\"\"description\"\"\" instead of #description".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_024() {
        let rule = NoHashtagDescription;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-hashtag-description");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
