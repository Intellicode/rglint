//! `no-unused-fields` (spec-035).

use std::collections::{HashMap, HashSet};

use apollo_compiler::executable::{Selection, SelectionSet};
use apollo_compiler::schema::ExtendedType;
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    ignore_type: Vec<String>,
    #[serde(default)]
    ignore_field: Vec<String>,
}

#[derive(Rule)]
#[rule(
    id = "no-unused-fields",
    category = "schema",
    requires_schema = true,
    requires_siblings = true,
    kinds = "FIELD_DEFINITION"
)]
pub struct NoUnusedFields;

impl NoUnusedFields {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoUnusedFieldsHandler {
            is_schema_source: false,
        })
    }
}

struct NoUnusedFieldsHandler {
    is_schema_source: bool,
}

impl Handler for NoUnusedFieldsHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        if node.kind == SyntaxKind::FIELD_DEFINITION {
            self.is_schema_source = true;
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        if !self.is_schema_source {
            return;
        }

        let Some(schema) = ctx.schema else {
            return;
        };
        let Some(siblings) = ctx.siblings else {
            return;
        };

        if !siblings.is_available() {
            return;
        }

        let opts: Opts = ctx.option().unwrap_or_default();

        let ignore_type: HashSet<&str> = opts.ignore_type.iter().map(|s| s.as_str()).collect();
        let ignore_field: HashSet<&str> = opts.ignore_field.iter().map(|s| s.as_str()).collect();

        let mut used: HashSet<(String, String)> = HashSet::new();

        let frag_map: HashMap<&str, &rglint_core::FragmentDef> =
            siblings.fragments().map(|(n, d)| (n.as_str(), d)).collect();

        for op_def in siblings.operations() {
            collect_used_fields(
                &op_def.node.selection_set,
                schema,
                &mut used,
                &frag_map,
                &mut HashSet::new(),
            );
        }

        for frag_def in siblings.fragments_all() {
            if used
                .iter()
                .any(|(t, f)| t == frag_def.node.selection_set.ty.as_str() && f == &frag_def.name)
            {
                continue;
            }
            collect_used_fields(
                &frag_def.node.selection_set,
                schema,
                &mut used,
                &frag_map,
                &mut HashSet::new(),
            );
        }

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for (type_name, ext_type) in &schema.types {
            if type_name.as_str().starts_with("__") {
                continue;
            }
            if ignore_type.contains(type_name.as_str()) {
                continue;
            }
            let fields = match ext_type {
                ExtendedType::Object(obj) => {
                    Some(obj.fields.keys().map(|n| n.to_string()).collect::<Vec<_>>())
                }
                ExtendedType::Interface(iface) => Some(
                    iface
                        .fields
                        .keys()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            };
            let Some(fields) = fields else {
                continue;
            };
            for field_name in &fields {
                let qualified = format!("{}.{}", type_name.as_str(), field_name);
                if ignore_field.contains(qualified.as_str()) {
                    continue;
                }
                if ignore_field.contains(field_name.as_str()) {
                    continue;
                }
                let key = (type_name.to_string(), field_name.clone());
                if used.contains(&key) {
                    continue;
                }
                ctx.report(DiagnosticBuilder::new(
                    rule_id,
                    path.clone(),
                    Span::new(0, 0),
                    format!("Field \"{field_name}\" is unused"),
                ));
            }
        }
    }
}

fn collect_used_fields(
    sel_set: &SelectionSet,
    schema: &apollo_compiler::Schema,
    used: &mut HashSet<(String, String)>,
    fragments: &HashMap<&str, &rglint_core::FragmentDef>,
    visited: &mut HashSet<String>,
) {
    let type_name = sel_set.ty.as_str().to_string();

    for selection in &sel_set.selections {
        match selection {
            Selection::Field(field) => {
                let field_name = field.name.as_str().to_string();
                used.insert((type_name.clone(), field_name.clone()));

                if let Some(obj) = schema.get_object(&type_name) {
                    for iface_name in &obj.implements_interfaces {
                        used.insert((iface_name.to_string(), field_name.clone()));
                    }
                }

                collect_used_fields(&field.selection_set, schema, used, fragments, visited);
            }
            Selection::FragmentSpread(spread) => {
                let frag_name = spread.fragment_name.as_str();
                if visited.contains(frag_name) {
                    continue;
                }
                visited.insert(frag_name.to_string());
                if let Some(frag_def) = fragments.get(frag_name) {
                    collect_used_fields(
                        &frag_def.node.selection_set,
                        schema,
                        used,
                        fragments,
                        visited,
                    );
                }
            }
            Selection::InlineFragment(inline) => {
                collect_used_fields(&inline.selection_set, schema, used, fragments, visited);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_035() {
        let rule = NoUnusedFields;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-unused-fields");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(meta.requires_siblings);
    }
}
