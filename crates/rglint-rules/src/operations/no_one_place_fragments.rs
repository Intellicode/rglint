//! `no-one-place-fragments` (spec-043).
//!
//! Port of graphql-eslint's rule that flags a fragment whose definition is
//! spread exactly once across the complete sibling document set. The upstream
//! rule counts every spread node, including spreads nested in fragment
//! definitions, and reports the fragment name node with the file containing
//! the sole spread in the message.

use std::collections::HashMap;
use std::path::Path;

use apollo_compiler::executable::{Selection, SelectionSet};
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "no-one-place-fragments",
    category = "operations",
    requires_siblings = true,
    kinds = "FRAGMENT_NAME"
)]
pub struct NoOnePlaceFragments;

impl NoOnePlaceFragments {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoOnePlaceFragmentsHandler {
            definitions: Vec::new(),
        })
    }
}

struct NoOnePlaceFragmentsHandler {
    definitions: Vec<(String, Span)>,
}

impl Handler for NoOnePlaceFragmentsHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        // graphql-eslint visits `FragmentDefinition > Name`, which is the
        // identifier span rather than the whole definition. The CST exposes
        // that identifier as FRAGMENT_NAME for both definitions and spreads.
        if parent.is_some_and(|parent| parent.kind == SyntaxKind::FRAGMENT_DEFINITION) {
            if let (Some(name), Some(span)) = (node.name.clone(), node.span) {
                self.definitions.push((name, span));
            }
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(siblings) = ctx.siblings else {
            return;
        };

        let mut usages: HashMap<String, Vec<String>> = HashMap::new();
        for operation in siblings.operations() {
            let file_name = display_file_name(operation.source.path());
            collect_spreads(&operation.node.selection_set, &file_name, &mut usages);
        }
        for fragment in siblings.fragments_all() {
            let file_name = display_file_name(fragment.source.path());
            collect_spreads(&fragment.node.selection_set, &file_name, &mut usages);
        }

        let source_path = ctx.source_code().path().to_path_buf();
        for (name, span) in &self.definitions {
            let Some(usage) = usages.get(name) else {
                // Unused fragments belong to graphql-eslint's separate
                // no-unused-fragments rule, not this one.
                continue;
            };
            if usage.len() != 1 {
                continue;
            }

            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                source_path.clone(),
                *span,
                format!(
                    "Fragment `{name}` used only once. Inline him in \"{}\".",
                    usage[0]
                ),
            ));
        }
    }
}

fn collect_spreads(
    selection_set: &SelectionSet,
    file_name: &str,
    usages: &mut HashMap<String, Vec<String>>,
) {
    for selection in &selection_set.selections {
        match selection {
            Selection::FragmentSpread(spread) => usages
                .entry(spread.fragment_name.as_str().to_owned())
                .or_default()
                .push(file_name.to_owned()),
            Selection::Field(field) => collect_spreads(&field.selection_set, file_name, usages),
            Selection::InlineFragment(inline) => {
                collect_spreads(&inline.selection_set, file_name, usages)
            }
        }
    }
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_043() {
        let meta = NoOnePlaceFragments.meta();
        assert_eq!(meta.id, "no-one-place-fragments");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(meta.requires_siblings);
        assert!(!meta.has_suggestions);
        assert!(meta.option_schema().is_none());
    }

    #[test]
    fn collect_spreads_counts_nested_and_repeated_occurrences() {
        let schema = apollo_compiler::Schema::parse(
            "type Query { user: User } type User { id: ID, friend: User }",
            "schema.graphql",
        )
        .expect("schema parses");
        let documents = rglint_core::DocumentLoader::new()
            .load(
                &rglint_core::DocumentSpec::Inline(
                    "query Q { user { ...UserFields friend { ...UserFields } } } fragment UserFields on User { id }"
                        .to_owned(),
                ),
                Path::new("01.graphql"),
                Some(&schema),
            )
            .expect("document parses");
        let siblings = rglint_core::Siblings::from_documents(&documents);
        let mut usages = HashMap::new();
        collect_spreads(
            &siblings.operations()[0].node.selection_set,
            "01.graphql",
            &mut usages,
        );
        assert_eq!(usages["UserFields"], ["01.graphql", "01.graphql"]);
    }
}
