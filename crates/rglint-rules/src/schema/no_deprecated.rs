//! `no-deprecated` (spec-034).

use std::ops::Deref;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "no-deprecated",
    category = "operations",
    requires_schema = true,
    kinds = "FIELD|ARGUMENT|ENUM_VALUE|OBJECT_FIELD"
)]
pub struct NoDeprecated;

impl NoDeprecated {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoDeprecatedHandler {
            candidates: Vec::new(),
        })
    }
}

struct NoDeprecatedHandler {
    candidates: Vec<Candidate>,
}

#[derive(Debug)]
struct Candidate {
    kind: CandidateKind,
    name: String,
    span: Span,
    field_chain: Vec<String>,
    arg_name: Option<String>,
}

#[derive(Debug)]
enum CandidateKind {
    Field,
    Argument,
    EnumValue,
    ObjectField,
}

impl Handler for NoDeprecatedHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };

        let kind = match node.kind {
            SyntaxKind::FIELD => CandidateKind::Field,
            SyntaxKind::ARGUMENT => CandidateKind::Argument,
            SyntaxKind::ENUM_VALUE => CandidateKind::EnumValue,
            SyntaxKind::OBJECT_FIELD => CandidateKind::ObjectField,
            _ => return,
        };

        let mut field_chain: Vec<String> = Vec::new();
        let mut arg_name: Option<String> = None;
        let mut current = node.parent;

        while let Some(p) = current {
            match p.kind {
                SyntaxKind::FIELD => {
                    if let Some(field_name) = &p.name {
                        field_chain.push(field_name.clone());
                    }
                }
                SyntaxKind::ARGUMENT if arg_name.is_none() => {
                    arg_name = p.name.clone();
                }
                _ => {}
            }
            current = p.parent;
        }

        field_chain.reverse();

        self.candidates.push(Candidate {
            kind,
            name,
            span,
            field_chain,
            arg_name,
        });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(s) => s,
            None => return,
        };
        let path = ctx.file.path();
        let rule_id = ctx.rule_id();

        let root_types = root_type_names(schema);

        for c in &self.candidates {
            match c.kind {
                CandidateKind::Field => {
                    check_field(schema, c, &root_types, rule_id, path, ctx);
                }
                CandidateKind::Argument => {
                    check_argument(schema, c, &root_types, rule_id, path, ctx);
                }
                CandidateKind::EnumValue => {
                    check_enum_value(schema, c, &root_types, rule_id, path, ctx);
                }
                CandidateKind::ObjectField => {
                    check_input_field(schema, c, &root_types, rule_id, path, ctx);
                }
            }
        }
    }
}

fn root_type_names(schema: &apollo_compiler::Schema) -> Vec<String> {
    let def = &schema.schema_definition;
    vec![
        def.query
            .as_ref()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Query".into()),
        def.mutation
            .as_ref()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Mutation".into()),
        def.subscription
            .as_ref()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Subscription".into()),
    ]
}

fn find_root_obj_field<'a>(
    schema: &'a apollo_compiler::Schema,
    field_name: &str,
    root_types: &[String],
) -> Option<&'a apollo_compiler::ast::FieldDefinition> {
    for root in root_types {
        if let Some(obj) = schema.get_object(root.as_str()) {
            if let Some(field) = obj.fields.get(field_name) {
                return Some(field.deref().deref());
            }
        }
        if let Some(iface) = schema.get_interface(root.as_str()) {
            if let Some(field) = iface.fields.get(field_name) {
                return Some(field.deref().deref());
            }
        }
    }
    None
}

fn is_field_deprecated(directives: &apollo_compiler::ast::DirectiveList) -> bool {
    directives.get("deprecated").is_some()
}

fn deprecation_reason_from(
    directives: &apollo_compiler::ast::DirectiveList,
    schema: &apollo_compiler::Schema,
) -> String {
    directives
        .get("deprecated")
        .and_then(|dir| dir.argument_by_name("reason", schema).ok())
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| "No longer supported".to_owned())
}

fn message_for(kind: &str, name: &str, reason: &str) -> String {
    if reason.is_empty() {
        format!("{kind} \"{name}\" is marked as deprecated in your GraphQL schema")
    } else {
        format!("{kind} \"{name}\" is marked as deprecated in your GraphQL schema (reason: {reason})")
    }
}

fn resolve_base_type_name(ty: &apollo_compiler::ast::Type) -> Option<&str> {
    match ty {
        apollo_compiler::ast::Type::Named(name) | apollo_compiler::ast::Type::NonNullNamed(name) => {
            Some(name.as_str())
        }
        apollo_compiler::ast::Type::List(inner) | apollo_compiler::ast::Type::NonNullList(inner) => {
            resolve_base_type_name(inner)
        }
    }
}

fn check_field(
    schema: &apollo_compiler::Schema,
    c: &Candidate,
    root_types: &[String],
    rule_id: &str,
    path: &std::path::Path,
    ctx: &mut RuleContext,
) {
    let field_def = match find_root_obj_field(schema, &c.name, root_types) {
        Some(fd) => fd,
        None => return,
    };
    if !is_field_deprecated(&field_def.directives) {
        return;
    }
    let reason = deprecation_reason_from(&field_def.directives, schema);
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        c.span,
        message_for("Field", &c.name, &reason),
    ));
}

fn check_argument(
    schema: &apollo_compiler::Schema,
    c: &Candidate,
    root_types: &[String],
    rule_id: &str,
    path: &std::path::Path,
    ctx: &mut RuleContext,
) {
    let parent_field_name = match c.field_chain.first() {
        Some(n) => n,
        None => return,
    };
    let field_def = match find_root_obj_field(schema, parent_field_name, root_types) {
        Some(fd) => fd,
        None => return,
    };
    let arg_def = match field_def.argument_by_name(&c.name) {
        Some(a) => a,
        None => return,
    };
    if arg_def.directives.get("deprecated").is_none() {
        return;
    }
    let reason = deprecation_reason_from(&arg_def.directives, schema);
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        c.span,
        message_for("Argument", &c.name, &reason),
    ));
}

fn check_enum_value(
    schema: &apollo_compiler::Schema,
    c: &Candidate,
    root_types: &[String],
    rule_id: &str,
    path: &std::path::Path,
    ctx: &mut RuleContext,
) {
    let parent_field_name = match c.field_chain.first() {
        Some(n) => n,
        None => return,
    };
    let arg_name = match &c.arg_name {
        Some(n) => n,
        None => return,
    };
    let field_def = match find_root_obj_field(schema, parent_field_name, root_types) {
        Some(fd) => fd,
        None => return,
    };
    let arg_def = match field_def.argument_by_name(arg_name) {
        Some(a) => a,
        None => return,
    };
    let type_name = match resolve_base_type_name(&arg_def.ty) {
        Some(t) => t,
        None => return,
    };
    let enum_type = match schema.get_enum(type_name) {
        Some(e) => e,
        None => return,
    };
    let value_def = match enum_type.values.get(c.name.as_str()) {
        Some(v) => v,
        None => return,
    };
    if value_def.directives.get("deprecated").is_none() {
        return;
    }
    let reason = deprecation_reason_from(&value_def.directives, schema);
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        c.span,
        message_for("Enum", &c.name, &reason),
    ));
}

fn check_input_field(
    schema: &apollo_compiler::Schema,
    c: &Candidate,
    root_types: &[String],
    rule_id: &str,
    path: &std::path::Path,
    ctx: &mut RuleContext,
) {
    let parent_field_name = match c.field_chain.first() {
        Some(n) => n,
        None => return,
    };
    let arg_name = match &c.arg_name {
        Some(n) => n,
        None => return,
    };
    let field_def = match find_root_obj_field(schema, parent_field_name, root_types) {
        Some(fd) => fd,
        None => return,
    };
    let arg_def = match field_def.argument_by_name(arg_name) {
        Some(a) => a,
        None => return,
    };
    let type_name = match resolve_base_type_name(&arg_def.ty) {
        Some(t) => t,
        None => return,
    };
    let input_obj = match schema.get_input_object(type_name) {
        Some(o) => o,
        None => return,
    };
    let input_field_def = match input_obj.fields.get(c.name.as_str()) {
        Some(f) => f,
        None => return,
    };
    if input_field_def.directives.get("deprecated").is_none() {
        return;
    }
    let reason = deprecation_reason_from(&input_field_def.directives, schema);
    ctx.report(DiagnosticBuilder::new(
        rule_id,
        path.to_path_buf(),
        c.span,
        message_for("Object field", &c.name, &reason),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_034() {
        let rule = NoDeprecated;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-deprecated");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
