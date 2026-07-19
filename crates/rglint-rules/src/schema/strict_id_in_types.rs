//! `strict-id-in-types` (spec-030).

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span};
use rglint_derive::Rule;
use serde::Deserialize;

fn default_id_names() -> Vec<String> {
    vec!["id".to_owned()]
}

fn default_id_types() -> Vec<String> {
    vec!["ID".to_owned()]
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_id_names")]
    accepted_id_names: Vec<String>,
    #[serde(default = "default_id_types")]
    accepted_id_types: Vec<String>,
    #[serde(default)]
    exceptions: Option<Exceptions>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            accepted_id_names: default_id_names(),
            accepted_id_types: default_id_types(),
            exceptions: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Exceptions {
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    suffixes: Vec<String>,
}

#[derive(Rule)]
#[rule(
    id = "strict-id-in-types",
    category = "schema",
    requires_schema = true,
    kinds = "OBJECT_TYPE_DEFINITION|INTERFACE_TYPE_DEFINITION|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_EXTENSION"
)]
pub struct StrictIdInTypes;

impl StrictIdInTypes {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        let root_types = find_root_types(ctx.schema);
        Box::new(StrictIdInTypesHandler {
            opts,
            root_types,
            type_entries: Vec::new(),
        })
    }
}

fn find_root_types(schema: Option<&apollo_compiler::Schema>) -> Vec<String> {
    let schema = match schema {
        Some(s) => s,
        None => return vec!["Query".into(), "Mutation".into(), "Subscription".into()],
    };
    let def = &schema.schema_definition;
    let query = def
        .query
        .as_ref()
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| "Query".into());
    let mutation = def
        .mutation
        .as_ref()
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| "Mutation".into());
    let subscription = def
        .subscription
        .as_ref()
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| "Subscription".into());
    vec![query, mutation, subscription]
}

struct TypeEntry {
    name: String,
    span: Span,
}

struct StrictIdInTypesHandler {
    opts: Opts,
    root_types: Vec<String>,
    type_entries: Vec<TypeEntry>,
}

impl StrictIdInTypesHandler {
    fn should_skip(&self, type_name: &str) -> bool {
        if self.root_types.iter().any(|r| r == type_name) {
            return true;
        }
        if let Some(ref exc) = self.opts.exceptions {
            if exc.types.iter().any(|t| t == type_name) {
                return true;
            }
            if exc.suffixes.iter().any(|s| type_name.ends_with(s.as_str())) {
                return true;
            }
        }
        false
    }
}

impl Handler for StrictIdInTypesHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };
        self.type_entries.push(TypeEntry { name, span });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(s) => s,
            None => return,
        };
        let source = ctx.source_code();
        let path = source.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for entry in &self.type_entries {
            if self.should_skip(&entry.name) {
                continue;
            }

            let valid_field_count = match schema.types.get(entry.name.as_str()) {
                Some(ext_type) => count_valid_fields(ext_type, &self.opts),
                None => 0,
            };

            if valid_field_count != 1 {
                let name_plural = if self.opts.accepted_id_names.len() > 1 {
                    "s"
                } else {
                    ""
                };
                let type_plural = if self.opts.accepted_id_types.len() > 1 {
                    "s"
                } else {
                    ""
                };
                let message = format!(
                    "type \"{}\" must have exactly one non-nullable unique identifier.\nAccepted name{}: {}.\nAccepted type{}: {}.",
                    entry.name,
                    name_plural,
                    english_join_words(&self.opts.accepted_id_names),
                    type_plural,
                    english_join_words(&self.opts.accepted_id_types),
                );
                ctx.report(DiagnosticBuilder::new(
                    rule_id,
                    path.clone(),
                    entry.span,
                    message,
                ));
            }
        }
    }
}

fn count_valid_fields(
    ext_type: &apollo_compiler::schema::ExtendedType,
    opts: &Opts,
) -> usize {
    use apollo_compiler::ast::Type;
    use apollo_compiler::schema::ExtendedType;
    use std::ops::Deref;

    let type_fields: Vec<&apollo_compiler::ast::FieldDefinition> = match ext_type {
        ExtendedType::Object(obj) => obj
            .fields
            .values()
            .map(|c| {
                let node: &apollo_compiler::Node<_> = c.deref();
                node.deref()
            })
            .collect(),
        ExtendedType::Interface(iface) => iface
            .fields
            .values()
            .map(|c| {
                let node: &apollo_compiler::Node<_> = c.deref();
                node.deref()
            })
            .collect(),
        _ => return 0,
    };

    type_fields
        .iter()
        .filter(|field| {
            let field_name = field.name.as_str();
            if !opts.accepted_id_names.iter().any(|n| n == field_name) {
                return false;
            }
            match &field.ty {
                Type::NonNullNamed(named_type) => {
                    let type_name = named_type.as_str();
                    opts.accepted_id_types.iter().any(|t| t == type_name)
                }
                _ => false,
            }
        })
        .count()
}

fn english_join_words(words: &[String]) -> String {
    match words.len() {
        0 => String::new(),
        1 => words[0].clone(),
        2 => format!("{} or {}", words[0], words[1]),
        _ => {
            let mut result = String::new();
            for (i, word) in words.iter().enumerate() {
                if i == words.len() - 1 {
                    result.push_str(&format!("or {word}"));
                } else {
                    result.push_str(&format!("{word}, "));
                }
            }
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_030() {
        let rule = StrictIdInTypes;
        let meta = rule.meta();
        assert_eq!(meta.id, "strict-id-in-types");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }

    #[test]
    fn option_deserializes_with_defaults() {
        let opts: Opts =
            serde_json::from_value(serde_json::json!({})).unwrap_or_default();
        assert_eq!(opts.accepted_id_names, vec!["id"]);
        assert_eq!(opts.accepted_id_types, vec!["ID"]);
        assert!(opts.exceptions.is_none());
    }

    #[test]
    fn option_deserializes_full_config() {
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "acceptedIdNames": ["id", "_id"],
            "acceptedIdTypes": ["ID", "String"],
            "exceptions": {
                "types": ["Error"],
                "suffixes": ["Result"]
            }
        }))
        .unwrap_or_default();
        assert_eq!(opts.accepted_id_names, vec!["id", "_id"]);
        assert_eq!(opts.accepted_id_types, vec!["ID", "String"]);
        let exc = opts.exceptions.unwrap();
        assert_eq!(exc.types, vec!["Error"]);
        assert_eq!(exc.suffixes, vec!["Result"]);
    }

    #[test]
    fn english_join_single_word() {
        assert_eq!(english_join_words(&["id".into()]), "id");
    }

    #[test]
    fn english_join_two_words() {
        assert_eq!(
            english_join_words(&["id".into(), "_id".into()]),
            "id or _id"
        );
    }

    #[test]
    fn english_join_three_words() {
        assert_eq!(
            english_join_words(&["id".into(), "_id".into(), "uid".into()]),
            "id, _id, or uid"
        );
    }

    #[test]
    fn schema_builder_accepts_type_definitions() {
        // Verify that the schema builder does NOT produce "executable document"
        // errors when given SDL type definitions.
        for source in [
            "type A { id: ID! }",
            "type A { _id: String! }",
            "type A { _id: String! } type A1 { id: ID! }",
            "type B { id: String! } type B1 { id: [String] }",
            "type B { id: ID! } type BError { message: String! }",
        ] {
            let builder = apollo_compiler::Schema::builder()
                .parse(source, "<inline>");
            let result = builder.build();
            assert!(result.is_ok(), "schema build should succeed for: {source}");
        }
    }
}
