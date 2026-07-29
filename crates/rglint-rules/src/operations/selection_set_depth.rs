//! `selection-set-depth` (spec-040).

use std::collections::{HashMap, HashSet};

use apollo_compiler::executable::{Selection, SelectionSet};
use apollo_compiler::schema::ExtendedType;
use rglint_core::{DiagnosticBuilder, Handler, RuleContext};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_max_depth")]
    max_depth: usize,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default)]
    depths: HashMap<String, usize>,
}

fn default_max_depth() -> usize {
    7
}

#[derive(Rule)]
#[rule(
    id = "selection-set-depth",
    category = "operations",
    requires_schema = true,
    requires_siblings = true
)]
pub struct SelectionSetDepth;

impl SelectionSetDepth {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(SelectionSetDepthHandler {
            opts: ctx.option().unwrap_or_default(),
        })
    }
}

struct SelectionSetDepthHandler {
    opts: Opts,
}

impl Handler for SelectionSetDepthHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        let (Some(schema), Some(siblings)) = (ctx.schema, ctx.siblings) else {
            return;
        };
        let source_path = ctx.source_code().path().to_path_buf();

        for operation in siblings.operations() {
            if operation.source.path() != source_path {
                continue;
            }

            let depth = selection_set_depth(
                &operation.node.selection_set,
                schema,
                siblings,
                &self.opts,
                &mut HashSet::new(),
            );
            if depth <= self.opts.max_depth {
                continue;
            }

            let name = operation.name.as_deref().unwrap_or("");
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                source_path.clone(),
                // graphql-eslint reports this rule against the operation's
                // document-level definition. The fixture parity contract
                // uses column zero, including anonymous operations.
                rglint_core::Span::new(0, 0),
                format!(
                    "'{name}' exceeds maximum operation depth of {}",
                    self.opts.max_depth
                ),
            ));
        }
    }
}

fn selection_set_depth(
    selection_set: &SelectionSet,
    schema: &apollo_compiler::Schema,
    siblings: &rglint_core::Siblings,
    opts: &Opts,
    fragments_in_progress: &mut HashSet<String>,
) -> usize {
    selection_set
        .selections
        .iter()
        .map(|selection| match selection {
            Selection::Field(field) => {
                let type_name = selection_set.ty.as_str();
                let qualified_name = format!("{type_name}.{}", field.name.as_str());

                if opts
                    .ignore
                    .iter()
                    .any(|ignored| ignored == field.name.as_str() || ignored == &qualified_name)
                {
                    return 0;
                }
                if let Some(depth) = opts.depths.get(&qualified_name) {
                    return *depth;
                }

                if field.selection_set.selections.is_empty() || is_scalar_result(field, schema) {
                    0
                } else {
                    1 + selection_set_depth(
                        &field.selection_set,
                        schema,
                        siblings,
                        opts,
                        fragments_in_progress,
                    )
                }
            }
            Selection::InlineFragment(inline) => selection_set_depth(
                &inline.selection_set,
                schema,
                siblings,
                opts,
                fragments_in_progress,
            ),
            Selection::FragmentSpread(spread) => {
                let name = spread.fragment_name.as_str();
                if !fragments_in_progress.insert(name.to_owned()) {
                    return 0;
                }
                let depth = siblings
                    .get_fragment_by_name(name)
                    .map(|fragment| {
                        selection_set_depth(
                            &fragment.node.selection_set,
                            schema,
                            siblings,
                            opts,
                            fragments_in_progress,
                        )
                    })
                    .unwrap_or(0);
                fragments_in_progress.remove(name);
                depth
            }
        })
        .max()
        .unwrap_or(0)
}

fn is_scalar_result(
    field: &apollo_compiler::Node<apollo_compiler::executable::Field>,
    schema: &apollo_compiler::Schema,
) -> bool {
    let type_name = base_type_name(&field.definition.ty);
    matches!(
        schema.types.get(type_name),
        Some(ExtendedType::Scalar(_) | ExtendedType::Enum(_))
    ) || matches!(type_name, "String" | "Int" | "Float" | "Boolean" | "ID")
}

fn base_type_name(ty: &apollo_compiler::ast::Type) -> &str {
    match ty {
        apollo_compiler::ast::Type::Named(name)
        | apollo_compiler::ast::Type::NonNullNamed(name) => name.as_str(),
        apollo_compiler::ast::Type::List(inner)
        | apollo_compiler::ast::Type::NonNullList(inner) => base_type_name(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_040() {
        let rule = SelectionSetDepth;
        let meta = rule.meta();
        assert_eq!(meta.id, "selection-set-depth");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(meta.requires_siblings);
    }

    #[test]
    fn cycle_guard_is_scoped_to_a_fragment_walk() {
        let schema = apollo_compiler::Schema::parse(
            "type Query { viewer: User } type User { friend: User, id: ID }",
            "schema.graphql",
        )
        .expect("schema parses");
        let documents = rglint_core::DocumentLoader::new()
            .load(
                &rglint_core::DocumentSpec::Inline(
                    "query Q { viewer { ...A } } fragment A on User { friend { ...A } id }"
                        .to_owned(),
                ),
                std::path::Path::new("query.graphql"),
                Some(&schema),
            )
            .expect("document parses");
        let siblings = rglint_core::Siblings::from_documents(&documents);
        let depth = selection_set_depth(
            &siblings.operations()[0].node.selection_set,
            &schema,
            &siblings,
            &Opts {
                max_depth: 99,
                ..Opts::default()
            },
            &mut HashSet::new(),
        );
        assert_eq!(depth, 2);
    }
}
