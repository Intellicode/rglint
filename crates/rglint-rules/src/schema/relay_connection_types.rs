//! `relay-connection-types` (spec-046).

use apollo_compiler::ast;
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

const MUST_BE_OBJECT_TYPE: &str = "Connection type must be an Object type.";
const MUST_HAVE_CONNECTION_SUFFIX: &str = "Connection type must have `Connection` suffix.";
const MUST_CONTAIN_FIELD_EDGES: &str =
    "Connection type must contain a field `edges` that return a list type.";
const MUST_CONTAIN_FIELD_PAGE_INFO: &str =
    "Connection type must contain a field `pageInfo` that return a non-null `PageInfo` Object type.";
const EDGES_FIELD_MUST_RETURN_LIST_TYPE: &str = "`edges` field must return a list type.";
const PAGE_INFO_FIELD_MUST_RETURN_NON_NULL_TYPE: &str =
    "`pageInfo` field must return a non-null `PageInfo` Object type.";

#[derive(Rule)]
#[rule(
    id = "relay-connection-types",
    category = "schema",
    requires_schema = true,
    kinds = "NAME|FIELD_DEFINITION|NAMED_TYPE|LIST_TYPE|NON_NULL_TYPE"
)]
pub struct RelayConnectionTypes;

impl RelayConnectionTypes {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RelayConnectionTypesHandler {
            definitions: Vec::new(),
            type_checks: Vec::new(),
        })
    }
}

struct RelayConnectionTypesHandler {
    definitions: Vec<DefinitionCandidate>,
    type_checks: Vec<TypeCandidate>,
}

struct DefinitionCandidate {
    name: String,
    name_span: Span,
    definition_span: Span,
    kind: DefinitionKind,
    has_edges: bool,
    has_page_info: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DefinitionKind {
    Object,
    NonObject,
}

struct TypeCandidate {
    object_name: String,
    field_name: String,
    kind: SyntaxKind,
    span: Span,
}

impl Handler for RelayConnectionTypesHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::NAME => self.record_definition_name(node),
            SyntaxKind::FIELD_DEFINITION => self.record_field(node),
            SyntaxKind::NAMED_TYPE | SyntaxKind::LIST_TYPE | SyntaxKind::NON_NULL_TYPE => {
                self.record_type(node)
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        if ctx.schema.is_none() {
            return;
        }

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for definition in &self.definitions {
            if definition.name.ends_with("Connection") {
                if definition.kind == DefinitionKind::NonObject {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        definition.name_span,
                        MUST_BE_OBJECT_TYPE,
                    );
                    continue;
                }

                if !definition.has_edges {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        definition.name_span,
                        MUST_CONTAIN_FIELD_EDGES,
                    );
                }
                if !definition.has_page_info {
                    report(
                        ctx,
                        rule_id,
                        &path,
                        definition.name_span,
                        MUST_CONTAIN_FIELD_PAGE_INFO,
                    );
                }
            } else if definition.kind == DefinitionKind::Object
                && definition.has_edges
                && definition.has_page_info
            {
                report(
                    ctx,
                    rule_id,
                    &path,
                    definition.name_span,
                    MUST_HAVE_CONNECTION_SUFFIX,
                );
            }
        }

        let Some(schema) = ctx.schema else { return };
        for candidate in &self.type_checks {
            let Some(field) = schema_field(schema, &candidate.object_name, &candidate.field_name)
            else {
                continue;
            };

            let valid = match candidate.field_name.as_str() {
                "edges" => match candidate.kind {
                    SyntaxKind::LIST_TYPE => true,
                    SyntaxKind::NON_NULL_TYPE => field.ty.is_list(),
                    SyntaxKind::NAMED_TYPE => false,
                    _ => unreachable!(),
                },
                "pageInfo" => {
                    candidate.kind == SyntaxKind::NON_NULL_TYPE
                        && matches!(&field.ty, ast::Type::NonNullNamed(name) if name == "PageInfo")
                }
                _ => true,
            };

            if valid {
                continue;
            }

            let message = match candidate.field_name.as_str() {
                "edges" => EDGES_FIELD_MUST_RETURN_LIST_TYPE,
                "pageInfo" => PAGE_INFO_FIELD_MUST_RETURN_NON_NULL_TYPE,
                _ => continue,
            };
            report(ctx, rule_id, &path, candidate.span, message);
        }
    }
}

impl RelayConnectionTypesHandler {
    fn record_definition_name(&mut self, node: &Node<'_>) {
        let Some(parent) = node.parent else { return };
        let kind = match parent.kind {
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => {
                DefinitionKind::Object
            }
            SyntaxKind::SCALAR_TYPE_DEFINITION
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
        let Some(name) = parent.name.clone() else {
            return;
        };
        let Some(name_span) = node.span else { return };
        self.definitions.push(DefinitionCandidate {
            name,
            name_span,
            definition_span: parent.span.unwrap_or(name_span),
            kind,
            has_edges: false,
            has_page_info: false,
        });
    }

    fn record_field(&mut self, node: &Node<'_>) {
        let Some(field_name) = node.name.as_deref() else {
            return;
        };
        let Some(object) = containing_object(node) else {
            return;
        };
        let Some(object_name) = object.name.clone() else {
            return;
        };
        let Some(object_span) = object.span else {
            return;
        };

        if let Some(definition) = self
            .definitions
            .iter_mut()
            .find(|definition| definition.definition_span == object_span)
        {
            if definition.name == object_name {
                if field_name == "edges" {
                    definition.has_edges = true;
                } else if field_name == "pageInfo" {
                    definition.has_page_info = true;
                }
            }
        }
    }

    fn record_type(&mut self, node: &Node<'_>) {
        let Some(field) = node
            .parent
            .filter(|parent| parent.kind == SyntaxKind::FIELD_DEFINITION)
        else {
            return;
        };
        let Some(field_name) = field.name.clone() else {
            return;
        };
        if field_name != "edges" && field_name != "pageInfo" {
            return;
        }
        let Some(object) = containing_object(field) else {
            return;
        };
        let Some(object_name) = object.name.clone() else {
            return;
        };
        let Some(span) = node.span else { return };
        self.type_checks.push(TypeCandidate {
            object_name,
            field_name,
            kind: node.kind,
            span,
        });
    }
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
    schema: &'a apollo_compiler::Schema,
    object_name: &str,
    field_name: &str,
) -> Option<&'a ast::FieldDefinition> {
    match schema.types.get(object_name)? {
        apollo_compiler::schema::ExtendedType::Object(object) => object
            .fields
            .get(field_name)
            .map(|field| std::ops::Deref::deref(std::ops::Deref::deref(field))),
        _ => None,
    }
}

fn report(ctx: &mut RuleContext, rule_id: &str, path: &std::path::Path, span: Span, message: &str) {
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
    fn rule_meta_matches_spec_046() {
        let rule = RelayConnectionTypes;
        let meta = rule.meta();
        assert_eq!(meta.id, "relay-connection-types");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn page_info_requires_exact_non_null_named_type() {
        let page_info = apollo_compiler::Name::new("PageInfo").unwrap();
        assert!(matches!(
            ast::Type::NonNullNamed(page_info),
            ast::Type::NonNullNamed(name) if name == "PageInfo"
        ));
        assert!(!matches!(
            ast::Type::NonNullList(Box::new(ast::Type::Named(
                apollo_compiler::Name::new("PageInfo").unwrap()
            ))),
            ast::Type::NonNullNamed(_)
        ));
    }
}
