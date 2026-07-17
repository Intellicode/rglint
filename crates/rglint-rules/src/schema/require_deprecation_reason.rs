use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "require-deprecation-reason",
    category = "schema",
    requires_schema = true,
    kinds = "DIRECTIVE|ARGUMENT"
)]
pub struct RequireDeprecationReason;

impl RequireDeprecationReason {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RequireDeprecationReasonHandler {
            directives: HashMap::new(),
            reasons: HashMap::new(),
        })
    }
}

struct RequireDeprecationReasonHandler {
    /// directive span offset → Span for all @deprecated directives
    directives: HashMap<usize, Span>,
    /// directive span offset → argument span for the `reason` argument
    reasons: HashMap<usize, Span>,
}

fn find_deprecated_directive<'a>(parent: Option<&'a Node<'a>>) -> Option<&'a Node<'a>> {
    let mut current = parent;
    loop {
        match current {
            Some(n) if n.kind == SyntaxKind::DIRECTIVE => return Some(n),
            Some(n) => current = n.parent,
            None => return None,
        }
    }
}

fn is_empty_or_whitespace_string(text: &str) -> bool {
    if let Some(inner) = text.strip_prefix("\"\"\"") {
        if let Some(end) = inner.find("\"\"\"") {
            inner[..end].trim().is_empty()
        } else {
            false
        }
    } else if let Some(inner) = text.strip_prefix('"') {
        if let Some(end) = inner.find('"') {
            inner[..end].trim().is_empty()
        } else {
            false
        }
    } else {
        false
    }
}

impl Handler for RequireDeprecationReasonHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::DIRECTIVE => {
                if node.name.as_deref() == Some("deprecated") {
                    if let Some(span) = node.span {
                        self.directives.insert(span.offset, span);
                    }
                }
            }
            SyntaxKind::ARGUMENT if node.name.as_deref() == Some("reason") => {
                if let Some(dir) = find_deprecated_directive(parent) {
                    if dir.name.as_deref() == Some("deprecated") {
                        if let (Some(dir_span), Some(arg_span)) = (dir.span, node.span) {
                            self.reasons.insert(dir_span.offset, arg_span);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let source = ctx.source_code().source().to_owned();
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();
        for (&dir_offset, &dir_span) in &self.directives {
            let needs_report = match self.reasons.get(&dir_offset) {
                Some(&arg_span) => {
                    let arg_text = &source[arg_span.offset..arg_span.end()];
                    if let Some(pos) = arg_text.find(':') {
                        let value_text = &arg_text[pos + 1..].trim_start();
                        is_empty_or_whitespace_string(value_text)
                    } else {
                        false
                    }
                }
                None => true,
            };
            if needs_report {
                ctx.report(DiagnosticBuilder::new(
                    rule_id,
                    path.clone(),
                    dir_span,
                    "@deprecated should have a reason".to_string(),
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
    fn rule_meta_matches_spec_026() {
        let rule = RequireDeprecationReason;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-deprecation-reason");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
