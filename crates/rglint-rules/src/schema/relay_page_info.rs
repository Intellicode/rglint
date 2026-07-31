//! `relay-page-info` (spec-048).

use std::ops::Deref;

use apollo_compiler::{ast, schema, Schema};
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

const PAGE_INFO: &str = "PageInfo";
const BOOLEAN_RETURN: &str = "non-null Boolean";
const CURSOR_RETURN: &str = "either String or Scalar, which can be null if there are no results";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default = "default_page_info_name")]
    page_info_name: String,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            page_info_name: default_page_info_name(),
        }
    }
}

fn default_page_info_name() -> String {
    PAGE_INFO.to_owned()
}

#[derive(Rule)]
#[rule(
    id = "relay-page-info",
    category = "schema",
    requires_schema = true,
    option_schema = r#"{
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "pageInfoName": {"type": "string", "default": "PageInfo"}
      }
    }"#,
    default_options = r#"{
      "pageInfoName": "PageInfo"
    }"#,
    kinds = "NAME|FIELD_DEFINITION"
)]
pub struct RelayPageInfo;

impl RelayPageInfo {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RelayPageInfoHandler {
            opts: ctx.option().unwrap_or_default(),
            definitions: Vec::new(),
            fields: Vec::new(),
            saw_schema_definition: false,
        })
    }
}

struct RelayPageInfoHandler {
    opts: Opts,
    definitions: Vec<DefinitionCandidate>,
    fields: Vec<FieldCandidate>,
    saw_schema_definition: bool,
}

struct DefinitionCandidate {
    name: String,
    name_span: Span,
    definition_span: Span,
    kind: DefinitionKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DefinitionKind {
    Object,
    NonObject,
}

struct FieldCandidate {
    object_span: Span,
    name: String,
    span: Span,
}

impl Handler for RelayPageInfoHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::NAME => self.record_definition(node),
            SyntaxKind::FIELD_DEFINITION => self.record_field(node),
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(schema) = ctx.schema else { return };
        let page_info_name = self.opts.page_info_name.as_str();
        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for definition in &self.definitions {
            if definition.name != page_info_name {
                continue;
            }

            if definition.kind == DefinitionKind::NonObject {
                report(
                    ctx,
                    rule_id,
                    &path,
                    definition.name_span,
                    format!("`{page_info_name}` must be an Object type."),
                );
            }
        }

        if self.saw_schema_definition && !schema.types.contains_key(page_info_name) {
            report(
                ctx,
                rule_id,
                &path,
                Span::new(0, 0),
                format!("The server must provide a `{page_info_name}` object."),
            );
        }

        for definition in &self.definitions {
            if definition.kind != DefinitionKind::Object || definition.name != page_info_name {
                continue;
            }

            let fields = self
                .fields
                .iter()
                .filter(|field| field.object_span == definition.definition_span);

            let mut field_map = std::collections::HashMap::new();
            for field in fields {
                field_map.insert(field.name.as_str(), field);
            }

            for (field_name, expected) in [
                ("hasPreviousPage", ExpectedType::Boolean),
                ("hasNextPage", ExpectedType::Boolean),
                ("startCursor", ExpectedType::Cursor),
                ("endCursor", ExpectedType::Cursor),
            ] {
                let Some(field) = field_map.get(field_name) else {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        definition.name_span,
                        missing_message(page_info_name, field_name, expected),
                    );
                    continue;
                };

                let valid = schema_field(schema, page_info_name, field_name)
                    .is_some_and(|schema_field| expected.allows(&schema_field.ty, schema));
                if !valid {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        field.span,
                        field_message(field_name, expected),
                    );
                }
            }
        }
    }
}

impl RelayPageInfoHandler {
    fn record_definition(&mut self, node: &Node<'_>) {
        let Some(parent) = node.parent else { return };
        let kind = match parent.kind {
            SyntaxKind::OBJECT_TYPE_DEFINITION => DefinitionKind::Object,
            SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION => DefinitionKind::NonObject,
            _ => return,
        };
        let (Some(name), Some(name_span), Some(definition_span)) =
            (parent.name.clone(), node.span, parent.span)
        else {
            return;
        };

        self.saw_schema_definition = true;
        self.definitions.push(DefinitionCandidate {
            name,
            name_span,
            definition_span,
            kind,
        });
    }

    fn record_field(&mut self, node: &Node<'_>) {
        let Some(object) = containing_object(node) else {
            return;
        };
        if object.kind != SyntaxKind::OBJECT_TYPE_DEFINITION {
            return;
        }
        let (Some(object_span), Some(name), Some(span)) =
            (object.span, node.name.clone(), node.span)
        else {
            return;
        };
        self.fields.push(FieldCandidate {
            object_span,
            name,
            span,
        });
    }
}

#[derive(Clone, Copy)]
enum ExpectedType {
    Boolean,
    Cursor,
}

impl ExpectedType {
    fn allows(self, ty: &ast::Type, schema: &Schema) -> bool {
        match self {
            Self::Boolean => {
                matches!(ty, ast::Type::NonNullNamed(name) if name == "Boolean")
            }
            Self::Cursor => {
                matches!(ty, ast::Type::Named(name) if name == "String" || is_scalar_type(schema, name.as_str()))
            }
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Boolean => BOOLEAN_RETURN,
            Self::Cursor => CURSOR_RETURN,
        }
    }
}

fn missing_message(page_info_name: &str, field_name: &str, expected: ExpectedType) -> String {
    format!(
        "`{page_info_name}` must contain a field `{field_name}`, that return {}.",
        expected.description()
    )
}

fn field_message(field_name: &str, expected: ExpectedType) -> String {
    format!(
        "Field `{field_name}` must return {}.",
        expected.description()
    )
}

fn containing_object<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent;
    while let Some(parent) = current {
        if matches!(
            parent.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return Some(parent);
        }
        current = parent.parent;
    }
    None
}

fn schema_field<'a>(
    schema: &'a Schema,
    object_name: &str,
    field_name: &str,
) -> Option<&'a ast::FieldDefinition> {
    match schema.types.get(object_name)? {
        schema::ExtendedType::Object(object) => object
            .fields
            .get(field_name)
            .map(|field| field.deref().deref()),
        _ => None,
    }
}

fn is_scalar_type(schema: &Schema, type_name: &str) -> bool {
    matches!(type_name, "String" | "Int" | "Float" | "Boolean" | "ID")
        || matches!(
            schema.types.get(type_name),
            Some(schema::ExtendedType::Scalar(_))
        )
}

fn report(
    ctx: &mut RuleContext,
    rule_id: &str,
    path: &std::path::Path,
    span: Span,
    message: impl Into<String>,
) {
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        span,
        message,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_048() {
        let rule = RelayPageInfo;
        let meta = rule.meta();
        assert_eq!(meta.id, "relay-page-info");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn options_default_to_page_info() {
        assert_eq!(Opts::default().page_info_name, PAGE_INFO);
        let opts: Opts = serde_json::from_value(serde_json::json!({
            "pageInfoName": "PagingInfo"
        }))
        .unwrap();
        assert_eq!(opts.page_info_name, "PagingInfo");
    }

    #[test]
    fn field_type_rules_match_relay_shape() {
        let boolean = apollo_compiler::Name::new("Boolean").unwrap();
        let string = apollo_compiler::Name::new("String").unwrap();
        let schema =
            Schema::parse_and_validate("scalar Date\ntype Query { x: Int }", "schema.graphqls")
                .unwrap()
                .into_inner();

        assert!(ExpectedType::Boolean.allows(&ast::Type::NonNullNamed(boolean), &schema));
        assert!(!ExpectedType::Boolean.allows(&ast::Type::Named(string.clone()), &schema));
        assert!(ExpectedType::Cursor.allows(&ast::Type::Named(string), &schema));
        let date = apollo_compiler::Name::new("Date").unwrap();
        assert!(ExpectedType::Cursor.allows(&ast::Type::Named(date), &schema));
        assert!(!ExpectedType::Cursor.allows(
            &ast::Type::NonNullNamed(apollo_compiler::Name::new("String").unwrap()),
            &schema
        ));
    }
}
