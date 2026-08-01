//! `require-type-pattern-with-oneof` (spec-051).
//!
//! The upstream rule applies to object types (not input objects) annotated
//! with `@oneOf`. It requires the conventional `error` and `ok` output
//! fields, preserving the local definition's fields rather than consulting a
//! merged schema that could include fields from an extension.

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "require-type-pattern-with-oneof",
    category = "schema",
    requires_schema = true,
    kinds = "OBJECT_TYPE_DEFINITION|DIRECTIVE|NAME|FIELD_DEFINITION"
)]
pub struct RequireTypePatternWithOneof;

impl RequireTypePatternWithOneof {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RequireTypePatternWithOneofHandler {
            definitions: Vec::new(),
        })
    }
}

struct Definition {
    span: Span,
    name: String,
    name_span: Span,
    one_of: bool,
    fields: Vec<String>,
}

struct RequireTypePatternWithOneofHandler {
    definitions: Vec<Definition>,
}

impl Handler for RequireTypePatternWithOneofHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::OBJECT_TYPE_DEFINITION => {
                let (Some(name), Some(span)) = (node.name.clone(), node.span) else {
                    return;
                };
                self.definitions.push(Definition {
                    span,
                    name,
                    name_span: span,
                    one_of: false,
                    fields: Vec::new(),
                });
            }
            SyntaxKind::NAME => self.record_name(node),
            SyntaxKind::DIRECTIVE if node.name.as_deref() == Some("oneOf") => {
                if let Some(definition_span) = find_object_definition(node).and_then(|n| n.span) {
                    if let Some(definition) = self
                        .definitions
                        .iter_mut()
                        .find(|definition| definition.span == definition_span)
                    {
                        definition.one_of = true;
                    }
                }
            }
            SyntaxKind::FIELD_DEFINITION => {
                let (Some(name), Some(definition_span)) = (
                    node.name.clone(),
                    find_object_definition(node).and_then(|n| n.span),
                ) else {
                    return;
                };
                if let Some(definition) = self
                    .definitions
                    .iter_mut()
                    .find(|definition| definition.span == definition_span)
                {
                    definition.fields.push(name);
                }
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        // The schema requirement matches the upstream rule metadata. The
        // local CST remains authoritative for fields so extensions cannot
        // satisfy a definition that is missing a required field.
        if ctx.schema.is_none() {
            return;
        }

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();
        for definition in &self.definitions {
            if !definition.one_of {
                continue;
            }
            for field_name in ["error", "ok"] {
                if definition.fields.iter().any(|field| field == field_name) {
                    continue;
                }
                ctx.report(DiagnosticBuilder::new(
                    rule_id,
                    path.clone(),
                    definition.name_span,
                    format!(
                        "type \"{}\" is defined as output with \"@oneOf\" and must be defined with \"{}\" field",
                        definition.name, field_name
                    ),
                ));
            }
        }
    }
}

impl RequireTypePatternWithOneofHandler {
    fn record_name(&mut self, node: &Node<'_>) {
        let Some(parent) = node.parent else { return };
        if parent.kind != SyntaxKind::OBJECT_TYPE_DEFINITION {
            return;
        }
        let (Some(definition_span), Some(name_span)) = (parent.span, node.span) else {
            return;
        };
        if let Some(definition) = self
            .definitions
            .iter_mut()
            .find(|definition| definition.span == definition_span)
        {
            definition.name_span = name_span;
        }
    }
}

fn find_object_definition<'a>(start: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = start.parent;
    while let Some(node) = current {
        if node.kind == SyntaxKind::OBJECT_TYPE_DEFINITION {
            return Some(node);
        }
        current = node.parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_051() {
        let rule = RequireTypePatternWithOneof;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-type-pattern-with-oneof");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
