use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "no-typename-prefix",
    category = "schema",
    kinds = "FIELD_DEFINITION"
)]
pub struct NoTypenamePrefix;

impl NoTypenamePrefix {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoTypenamePrefixHandler {
            diagnostics: Vec::new(),
        })
    }
}

fn is_container(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
    )
}

fn find_container<'a>(parent: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = parent;
    loop {
        if is_container(current.kind) {
            return Some(current);
        }
        current = current.parent?;
    }
}

struct NoTypenamePrefixHandler {
    diagnostics: Vec<(String, String, Span)>,
}

impl Handler for NoTypenamePrefixHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let field_name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let parent = match _parent {
            Some(p) => p,
            None => return,
        };
        let container = match find_container(parent) {
            Some(c) => c,
            None => return,
        };
        let type_name = match &container.name {
            Some(n) => n.clone(),
            None => return,
        };
        let field_lower = field_name.to_lowercase();
        let type_lower = type_name.to_lowercase();
        if field_lower.starts_with(&type_lower) {
            let span = node.span.unwrap_or(Span::new(0, 0));
            self.diagnostics
                .push((field_name, type_name, span));
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for (field_name, type_name, span) in self.diagnostics.drain(..) {
            let message =
                format!("Field \"{field_name}\" starts with the name of the parent type \"{type_name}\"");
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                span,
                message,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_031() {
        let rule = NoTypenamePrefix;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-typename-prefix");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
