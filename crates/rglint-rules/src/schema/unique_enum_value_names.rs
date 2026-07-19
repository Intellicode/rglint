//! `unique-enum-value-names` (spec-029).

use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "unique-enum-value-names",
    category = "schema",
    kinds = "ENUM_VALUE"
)]
pub struct UniqueEnumValueNames;

impl UniqueEnumValueNames {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(UniqueEnumValueNamesHandler {
            seen: HashMap::new(),
            duplicates: Vec::new(),
        })
    }
}

fn is_enum_container(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION
    )
}

struct UniqueEnumValueNamesHandler {
    seen: HashMap<usize, HashMap<String, Span>>,
    duplicates: Vec<(String, Span)>,
}

impl UniqueEnumValueNamesHandler {
    fn find_enum_container<'a>(parent: &'a Node<'a>) -> Option<&'a Node<'a>> {
        let mut current = parent;
        loop {
            if is_enum_container(current.kind) {
                return Some(current);
            }
            current = current.parent?;
        }
    }
}

impl Handler for UniqueEnumValueNamesHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let value_name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };

        let parent = match _parent {
            Some(p) => p,
            None => return,
        };

        let container = match Self::find_enum_container(parent) {
            Some(c) => c,
            None => return,
        };

        let container_key = container.span.unwrap_or(Span::new(0, 0)).offset;
        let span = node.span.unwrap_or(Span::new(0, 0));

        let lower_name = value_name.to_lowercase();
        let fields = self.seen.entry(container_key).or_default();
        if let std::collections::hash_map::Entry::Vacant(e) = fields.entry(lower_name) {
            e.insert(Span::new(span.offset, 0));
            return;
        }

        self.duplicates.push((value_name, Span::new(span.offset, 0)));
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for (name, span) in self.duplicates.drain(..) {
            let message = format!(
                "Unexpected case-insensitive enum values duplicates for {name}"
            );
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
    fn rule_meta_matches_spec_029() {
        let rule = UniqueEnumValueNames;
        let meta = rule.meta();
        assert_eq!(meta.id, "unique-enum-value-names");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn rule_interested_in_enum_value() {
        let entry = rglint_core::ALL_RULES
            .iter()
            .find(|e| e.meta.id == "unique-enum-value-names")
            .expect("unique-enum-value-names must be registered via #[derive(Rule)]");
        assert_eq!(entry.interested_kinds, &[SyntaxKind::ENUM_VALUE]);
    }
}
