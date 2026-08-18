//! `no-unreachable-types` (spec-036).

use std::collections::{HashSet, VecDeque};

use apollo_compiler::schema::ExtendedType;
use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    ignore_type: Vec<String>,
}

#[derive(Rule)]
#[rule(
    id = "no-unreachable-types",
    category = "schema",
    requires_schema = true,
    kinds = "SCALAR_TYPE_DEFINITION|OBJECT_TYPE_DEFINITION|INTERFACE_TYPE_DEFINITION|UNION_TYPE_DEFINITION|ENUM_TYPE_DEFINITION|INPUT_OBJECT_TYPE_DEFINITION|DIRECTIVE_DEFINITION|SCALAR_TYPE_EXTENSION|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_EXTENSION|UNION_TYPE_EXTENSION|ENUM_TYPE_EXTENSION|INPUT_OBJECT_TYPE_EXTENSION"
)]
pub struct NoUnreachableTypes;

impl NoUnreachableTypes {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoUnreachableTypesHandler { defs: Vec::new() })
    }
}

struct Definition {
    name: String,
    kind: DefKind,
}

enum DefKind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
    Directive,
}

impl DefKind {
    fn message_prefix(&self) -> &'static str {
        match self {
            DefKind::Scalar => "Scalar",
            DefKind::Object => "Object",
            DefKind::Interface => "Interface",
            DefKind::Union => "Union",
            DefKind::Enum => "Enum",
            DefKind::InputObject => "Input object",
            DefKind::Directive => "Directive",
        }
    }

    fn from_kind(kind: SyntaxKind) -> Option<Self> {
        use SyntaxKind::*;
        match kind {
            SCALAR_TYPE_DEFINITION | SCALAR_TYPE_EXTENSION => Some(DefKind::Scalar),
            OBJECT_TYPE_DEFINITION | OBJECT_TYPE_EXTENSION => Some(DefKind::Object),
            INTERFACE_TYPE_DEFINITION | INTERFACE_TYPE_EXTENSION => Some(DefKind::Interface),
            UNION_TYPE_DEFINITION | UNION_TYPE_EXTENSION => Some(DefKind::Union),
            ENUM_TYPE_DEFINITION | ENUM_TYPE_EXTENSION => Some(DefKind::Enum),
            INPUT_OBJECT_TYPE_DEFINITION | INPUT_OBJECT_TYPE_EXTENSION => {
                Some(DefKind::InputObject)
            }
            DIRECTIVE_DEFINITION => Some(DefKind::Directive),
            _ => None,
        }
    }
}

struct NoUnreachableTypesHandler {
    defs: Vec<Definition>,
}

impl Handler for NoUnreachableTypesHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let kind = match DefKind::from_kind(node.kind) {
            Some(k) => k,
            None => return,
        };
        self.defs.push(Definition { name, kind });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema {
            Some(s) => s,
            None => return,
        };

        let opts: Opts = ctx.option().unwrap_or_default();
        let ignore_type: HashSet<&str> = opts.ignore_type.iter().map(|s| s.as_str()).collect();

        let reachable = compute_reachable_types(schema);
        let has_reachable = has_user_reachable_types(&reachable, schema);

        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();

        for def in &self.defs {
            match def.kind {
                DefKind::Directive => {
                    // Only report directives when nothing is reachable
                    if !has_reachable {
                        ctx.report(DiagnosticBuilder::new(
                            rule_id,
                            path.clone(),
                            Span::new(0, 0),
                            format!("Directive `{}` is unreachable.", def.name),
                        ));
                    }
                }
                _ => {
                    if def.name.starts_with("__") {
                        continue;
                    }
                    if ignore_type.contains(def.name.as_str()) {
                        continue;
                    }
                    if !reachable.contains(def.name.as_str()) {
                        ctx.report(DiagnosticBuilder::new(
                            rule_id,
                            path.clone(),
                            Span::new(0, 0),
                            format!(
                                "{} type `{}` is unreachable.",
                                def.kind.message_prefix(),
                                def.name
                            ),
                        ));
                    }
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

fn resolve_base_type_name(ty: &apollo_compiler::ast::Type) -> Option<&str> {
    match ty {
        apollo_compiler::ast::Type::Named(name)
        | apollo_compiler::ast::Type::NonNullNamed(name) => Some(name.as_str()),
        apollo_compiler::ast::Type::List(inner)
        | apollo_compiler::ast::Type::NonNullList(inner) => resolve_base_type_name(inner),
    }
}

fn compute_reachable_types(schema: &apollo_compiler::Schema) -> HashSet<String> {
    let mut reachable: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    for root in root_type_names(schema) {
        if reachable.insert(root.clone()) {
            queue.push_back(root);
        }
    }

    for builtin in &["String", "Int", "Float", "Boolean", "ID"] {
        reachable.insert(builtin.to_string());
    }

    for (_dir_name, dir_def) in &schema.directive_definitions {
        for arg in &dir_def.arguments {
            if let Some(base) = resolve_base_type_name(&arg.ty) {
                if reachable.insert(base.to_string()) {
                    queue.push_back(base.to_string());
                }
            }
        }
    }

    while let Some(type_name) = queue.pop_front() {
        let ext_type = match schema.types.get(type_name.as_str()) {
            Some(t) => t,
            None => continue,
        };

        match ext_type {
            ExtendedType::Object(obj) => {
                for iface in &obj.implements_interfaces {
                    let name = iface.to_string();
                    if reachable.insert(name.clone()) {
                        queue.push_back(name);
                    }
                }
                for field in obj.fields.values() {
                    if let Some(base) = resolve_base_type_name(&field.ty) {
                        if reachable.insert(base.to_string()) {
                            queue.push_back(base.to_string());
                        }
                    }
                    for arg in &field.arguments {
                        if let Some(base) = resolve_base_type_name(&arg.ty) {
                            if reachable.insert(base.to_string()) {
                                queue.push_back(base.to_string());
                            }
                        }
                    }
                }
            }
            ExtendedType::Interface(iface) => {
                for iface_name in &iface.implements_interfaces {
                    let name = iface_name.to_string();
                    if reachable.insert(name.clone()) {
                        queue.push_back(name);
                    }
                }
                for field in iface.fields.values() {
                    if let Some(base) = resolve_base_type_name(&field.ty) {
                        if reachable.insert(base.to_string()) {
                            queue.push_back(base.to_string());
                        }
                    }
                }
                // Find types and interfaces that implement this interface
                for (_name, other_type) in &schema.types {
                    let implements = match other_type {
                        ExtendedType::Object(obj) => Some(&obj.implements_interfaces),
                        ExtendedType::Interface(iface2) => Some(&iface2.implements_interfaces),
                        _ => None,
                    };
                    if let Some(impls) = implements {
                        if impls.iter().any(|i| i.as_str() == type_name) {
                            let n = match other_type {
                                ExtendedType::Object(obj) => obj.name.to_string(),
                                ExtendedType::Interface(iface2) => iface2.name.to_string(),
                                _ => unreachable!(),
                            };
                            if reachable.insert(n.clone()) {
                                queue.push_back(n);
                            }
                        }
                    }
                }
            }
            ExtendedType::Union(union) => {
                for member in &union.members {
                    let name = member.to_string();
                    if reachable.insert(name.clone()) {
                        queue.push_back(name);
                    }
                }
            }
            ExtendedType::Scalar(_) | ExtendedType::Enum(_) | ExtendedType::InputObject(_) => {}
        }
    }

    reachable
}

/// Returns true if there are any user-defined (non-built-in, non-introspection) reachable types.
fn has_user_reachable_types(reachable: &HashSet<String>, schema: &apollo_compiler::Schema) -> bool {
    for type_name in reachable {
        if type_name.starts_with("__") {
            continue;
        }
        match type_name.as_str() {
            "String" | "Int" | "Float" | "Boolean" | "ID" => continue,
            _ => {}
        }
        if schema.types.contains_key(type_name.as_str()) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_036() {
        let rule = NoUnreachableTypes;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-unreachable-types");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
