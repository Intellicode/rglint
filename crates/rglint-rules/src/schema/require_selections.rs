//! `require-selections` (spec-042).
//!
//! Keep the implementation aligned with graphql-eslint's rule: the option is
//! `fieldName` (a string or list, defaulting to `id`) and `requireAllFields`
//! controls whether one or every available field must be selected.

use std::collections::HashSet;

use apollo_compiler::executable::{Selection, SelectionSet};
use apollo_compiler::schema::ExtendedType;
use rglint_core::{DiagnosticBuilder, Fix, Handler, RuleContext, Span};
use rglint_derive::Rule;
use serde::Deserialize;

const DEFAULT_FIELD_NAME: &str = "id";
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FieldNames {
    One(String),
    Many(Vec<String>),
}

impl FieldNames {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(name) => vec![name],
            Self::Many(names) => names,
        }
    }
}

fn default_field_names() -> Vec<String> {
    vec![DEFAULT_FIELD_NAME.to_owned()]
}

fn deserialize_field_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    FieldNames::deserialize(deserializer).map(FieldNames::into_vec)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(
        default = "default_field_names",
        deserialize_with = "deserialize_field_names"
    )]
    field_name: Vec<String>,
    #[serde(default)]
    require_all_fields: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            field_name: default_field_names(),
            require_all_fields: false,
        }
    }
}

#[derive(Rule)]
#[rule(
    id = "require-selections",
    category = "operations",
    requires_schema = true,
    requires_siblings = true,
    has_suggestions = true,
    option_schema = r#"{
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "fieldName": {
          "oneOf": [
            {"type": "string"},
            {"type": "array", "items": {"type": "string"}}
          ],
          "default": "id"
        },
        "requireAllFields": {"type": "boolean"}
      }
    }"#
)]
pub struct RequireSelections;

impl RequireSelections {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RequireSelectionsHandler {
            opts: ctx.option().unwrap_or_default(),
        })
    }
}

struct RequireSelectionsHandler {
    opts: Opts,
}

#[derive(Clone)]
struct Anchor {
    start: usize,
    label: String,
    in_fragment: bool,
}

impl Handler for RequireSelectionsHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        let (Some(schema), Some(siblings)) = (ctx.schema, ctx.siblings) else {
            return;
        };
        let source = ctx.source_code().source().to_owned();
        let source_path = ctx.source_code().path().to_path_buf();

        for operation in siblings.operations() {
            if operation.source.path() != source_path {
                continue;
            }
            walk_selection_set(
                &operation.node.selection_set,
                None,
                false,
                schema,
                siblings,
                &self.opts,
                &source,
                ctx,
            );
        }

        for fragment in siblings.fragments_all() {
            if fragment.source.path() != source_path {
                continue;
            }
            let Some(location) = fragment.node.location() else {
                continue;
            };
            walk_selection_set(
                &fragment.node.selection_set,
                Some(Anchor {
                    start: location.offset(),
                    label: fragment.name.clone(),
                    in_fragment: true,
                }),
                true,
                schema,
                siblings,
                &self.opts,
                &source,
                ctx,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_selection_set(
    selection_set: &SelectionSet,
    anchor: Option<Anchor>,
    check_current: bool,
    schema: &apollo_compiler::Schema,
    siblings: &rglint_core::Siblings,
    opts: &Opts,
    source: &str,
    ctx: &mut RuleContext,
) {
    if check_current {
        if let Some(anchor) = anchor.as_ref() {
            report_missing(selection_set, anchor, schema, siblings, opts, source, ctx);
        }
    }

    let in_fragment = anchor.as_ref().is_some_and(|a| a.in_fragment);
    for selection in &selection_set.selections {
        match selection {
            Selection::Field(field) if !field.selection_set.selections.is_empty() => {
                let Some(location) = field.location() else {
                    continue;
                };
                let label = field
                    .alias
                    .as_ref()
                    .unwrap_or(&field.name)
                    .as_str()
                    .to_owned();
                walk_selection_set(
                    &field.selection_set,
                    Some(Anchor {
                        start: location.offset(),
                        label,
                        in_fragment,
                    }),
                    true,
                    schema,
                    siblings,
                    opts,
                    source,
                    ctx,
                );
            }
            Selection::InlineFragment(inline) => {
                // graphql-eslint skips the inline fragment's own selection
                // set. Its fields are still walked, so nested object fields
                // are checked normally.
                walk_selection_set(
                    &inline.selection_set,
                    None,
                    false,
                    schema,
                    siblings,
                    opts,
                    source,
                    ctx,
                );
            }
            Selection::Field(_) | Selection::FragmentSpread(_) => {}
        }
    }
}

fn report_missing(
    selection_set: &SelectionSet,
    anchor: &Anchor,
    schema: &apollo_compiler::Schema,
    siblings: &rglint_core::Siblings,
    opts: &Opts,
    source: &str,
    ctx: &mut RuleContext,
) {
    let available = available_fields(selection_set, schema, siblings, opts);
    if available.is_empty() {
        return;
    }

    let mut used_fragments = Vec::new();
    let selected = selected_fields(
        selection_set,
        siblings,
        &mut HashSet::new(),
        &mut used_fragments,
    );
    let missing: Vec<&str> = available
        .iter()
        .filter(|name| !selected.contains(name.as_str()))
        .map(String::as_str)
        .collect();
    if missing.is_empty() {
        return;
    }
    if !opts.require_all_fields && missing.len() < available.len() {
        return;
    }

    let report_names: Vec<&str> = if opts.require_all_fields {
        missing
    } else {
        // With the default behavior, any one available field satisfies the
        // rule, so report the whole configured alternative list only when all
        // alternatives are absent.
        available.iter().map(String::as_str).collect()
    };
    let report_groups: Vec<Vec<&str>> = if opts.require_all_fields {
        report_names.iter().map(|name| vec![*name]).collect()
    } else {
        vec![report_names]
    };

    for report_names in report_groups {
        let names = report_names
            .iter()
            .map(|name| format!("`{}.{name}`", anchor.label))
            .collect::<Vec<_>>();
        let plural_suffix = if names.len() > 1 { "s" } else { "" };
        let field_name = english_join(&names);
        let addition = if used_fragments.is_empty() {
            String::new()
        } else {
            let fragments = used_fragments
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>();
            format!(
                " or add to used fragment{} {}",
                if fragments.len() > 1 { "s" } else { "" },
                english_join(&fragments)
            )
        };
        let message = format!(
            "Field{plural_suffix} {field_name} must be selected when it's available on a type.\nInclude it in your selection set{addition}."
        );

        let report_offset = selection_set_start(source, anchor.start).unwrap_or(anchor.start);
        let mut builder = DiagnosticBuilder::new(
            ctx.rule_id(),
            ctx.source_code().path().to_path_buf(),
            Span::new(report_offset, 0),
            message,
        );

        if !anchor.in_fragment {
            for name in report_names {
                if let Some(offset) = first_selection_offset(selection_set) {
                    builder = builder.suggestion(
                        format!("Add `{name}` selection"),
                        Fix::Insert {
                            offset,
                            text: format!("{name} "),
                        },
                    );
                }
            }
        }

        ctx.report(builder);
    }
}

fn available_fields(
    selection_set: &SelectionSet,
    schema: &apollo_compiler::Schema,
    siblings: &rglint_core::Siblings,
    opts: &Opts,
) -> Vec<String> {
    let type_name = selection_set.ty.as_str();
    match schema.types.get(type_name) {
        Some(ExtendedType::Object(object)) => fields_in_type(object.fields.keys(), opts),
        Some(ExtendedType::Interface(interface)) => fields_in_type(interface.fields.keys(), opts),
        Some(ExtendedType::Union(union)) => {
            let mut available = Vec::new();
            for selection in &selection_set.selections {
                match selection {
                    Selection::InlineFragment(inline) => {
                        if let Some(condition) = inline.type_condition.as_ref() {
                            add_type_fields(
                                condition.as_str(),
                                union,
                                schema,
                                opts,
                                &mut available,
                            );
                        }
                    }
                    Selection::FragmentSpread(spread) => {
                        if let Some(fragment) =
                            siblings.get_fragment_by_name(spread.fragment_name.as_str())
                        {
                            add_type_fields(
                                fragment.node.selection_set.ty.as_str(),
                                union,
                                schema,
                                opts,
                                &mut available,
                            );
                        }
                    }
                    Selection::Field(_) => {}
                }
            }
            available
        }
        _ => Vec::new(),
    }
}

fn fields_in_type<'a, I>(fields: I, opts: &Opts) -> Vec<String>
where
    I: Iterator<Item = &'a apollo_compiler::Name> + Clone,
{
    opts.field_name
        .iter()
        .filter(|name| fields.clone().any(|field| field.as_str() == name.as_str()))
        .cloned()
        .collect()
}

fn add_type_fields(
    type_name: &str,
    union: &apollo_compiler::Node<apollo_compiler::schema::UnionType>,
    schema: &apollo_compiler::Schema,
    opts: &Opts,
    available: &mut Vec<String>,
) {
    if type_name == union.name.as_str()
        || union
            .members
            .iter()
            .any(|member| member.as_str() == type_name)
    {
        let fields = match schema.types.get(type_name) {
            Some(ExtendedType::Object(object)) => fields_in_type(object.fields.keys(), opts),
            Some(ExtendedType::Interface(interface)) => {
                fields_in_type(interface.fields.keys(), opts)
            }
            _ => Vec::new(),
        };
        for field in fields {
            if !available.contains(&field) {
                available.push(field);
            }
        }
    }
}

fn selected_fields(
    selection_set: &SelectionSet,
    siblings: &rglint_core::Siblings,
    visited: &mut HashSet<String>,
    used_fragments: &mut Vec<String>,
) -> HashSet<String> {
    let mut selected = HashSet::new();
    for selection in &selection_set.selections {
        match selection {
            Selection::Field(field) => {
                selected.insert(field.name.as_str().to_owned());
                if let Some(alias) = field.alias.as_ref() {
                    selected.insert(alias.as_str().to_owned());
                }
            }
            Selection::InlineFragment(inline) => {
                selected.extend(selected_fields(
                    &inline.selection_set,
                    siblings,
                    visited,
                    used_fragments,
                ));
            }
            Selection::FragmentSpread(spread) => {
                let name = spread.fragment_name.as_str();
                if !visited.insert(name.to_owned()) {
                    continue;
                }
                used_fragments.push(name.to_owned());
                if let Some(fragment) = siblings.get_fragment_by_name(name) {
                    selected.extend(selected_fields(
                        &fragment.node.selection_set,
                        siblings,
                        visited,
                        used_fragments,
                    ));
                }
            }
        }
    }
    selected
}

fn english_join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [left, right] => format!("{left} or {right}"),
        _ => {
            let last = items.last().expect("non-empty");
            format!("{}, or {last}", items[..items.len() - 1].join(", "))
        }
    }
}

fn selection_set_start(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = start.min(bytes.len());
    while index < bytes.len() {
        match bytes[index] {
            b'#' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'"' => {
                if bytes.get(index..index + 3) == Some(b"\"\"\"") {
                    index += 3;
                    while index + 2 < bytes.len() && bytes.get(index..index + 3) != Some(b"\"\"\"")
                    {
                        index += 1;
                    }
                    index = (index + 3).min(bytes.len());
                } else {
                    index += 1;
                    while index < bytes.len() {
                        let escaped = index > 0 && bytes[index - 1] == b'\\';
                        if bytes[index] == b'"' && !escaped {
                            index += 1;
                            break;
                        }
                        index += 1;
                    }
                }
            }
            b'{' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn first_selection_offset(selection_set: &SelectionSet) -> Option<usize> {
    selection_set
        .selections
        .first()
        .and_then(|selection| match selection {
            Selection::Field(field) => field.location().map(|location| location.offset()),
            Selection::FragmentSpread(spread) => {
                spread.location().map(|location| location.offset())
            }
            Selection::InlineFragment(inline) => first_selection_offset(&inline.selection_set),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_042() {
        let meta = RequireSelections.meta();
        assert_eq!(meta.id, "require-selections");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(meta.requires_siblings);
        assert!(meta.has_suggestions);
        assert!(meta.option_schema().is_some());
    }

    #[test]
    fn joins_words_like_intl_list_format() {
        assert_eq!(english_join(&["a".to_owned(), "b".to_owned()]), "a or b");
        assert_eq!(
            english_join(&["a".to_owned(), "b".to_owned(), "c".to_owned()]),
            "a, b, or c"
        );
    }

    #[test]
    fn nested_field_selections_do_not_satisfy_the_parent_type() {
        let schema = apollo_compiler::Schema::parse(
            "type Query { user: User } type User { id: ID posts: Post } type Post { id: ID }",
            "schema.graphql",
        )
        .expect("schema parses");
        let documents = rglint_core::DocumentLoader::new()
            .load(
                &rglint_core::DocumentSpec::Inline("{ user { posts { id } } }".to_owned()),
                std::path::Path::new("query.graphql"),
                Some(&schema),
            )
            .expect("document parses");
        let siblings = rglint_core::Siblings::from_documents(&documents);
        let user_selection_set = match &siblings.operations()[0].node.selection_set.selections[0] {
            Selection::Field(field) => &field.selection_set,
            _ => panic!("expected user field"),
        };
        let selected = selected_fields(
            user_selection_set,
            &siblings,
            &mut HashSet::new(),
            &mut Vec::new(),
        );
        assert!(selected.contains("posts"));
        assert!(!selected.contains("id"));
    }
}
