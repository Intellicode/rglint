//! Compile a parsed [`SelectorNode`] tree into a [`Matcher`] closure
//! (spec-010 / PLAN §4.3).
//!
//! Each variant of [`SelectorNode`] becomes a small `Box<dyn Fn>` over a
//! [`Node`]; combinators (`Child` / `Descendant`) walk the `parent` chain
//! stored on the [`Node`] struct. The result is a single owned
//! `Send + Sync + 'static` callable the engine (spec-011) invokes once per
//! visited node — no per-node allocation, no AST node kinds the matcher
//! cares about beyond [`Node::kind`] and the four attribute fields.
//!
//! ## `Kind` name resolution
//!
//! Selectors spell kinds in graphql-eslint's camelCase form
//! (`ObjectTypeDefinition`); `apollo_parser`'s [`SyntaxKind`] enum uses
//! SCREAMING_SNAKE_CASE. [`kind_from_camel`] is the small static table that
//! bridges the two, so a `Kind(name)` is resolved to a constant at compile
//! time and the matcher ends up as a single `kind ==` comparison.

use apollo_parser::SyntaxKind;

use crate::node::Node;
use crate::selector::ast::{AttrKind, AttrOp, AttrValue, SelectorError, SelectorNode};

/// A compiled selector: a `Send + Sync + 'static` predicate the engine
/// (spec-011) applies to every visited AST node, with the node's optional
/// direct parent. The lifetimes are higher-ranked so the same matcher can be
/// applied to `Node`s of any lifetime over the course of a run.
pub type Matcher = Box<dyn Fn(&Node<'_>, Option<&Node<'_>>) -> bool + Send + Sync>;

/// Compile a selector source string into a [`Matcher`].
///
/// Lexes → parses → walks the AST, returning the first error encountered
/// (with a byte span into the source). Allocations: one `Regex` per
/// `=~ /.../` predicate, one `String` per `= "..."` predicate, and the
/// `Box`es of the recursive tree; the resulting matcher then runs at
/// zero allocation per node.
pub fn compile(src: &str) -> Result<Matcher, SelectorError> {
    let tree = crate::selector::parser::parse(src)?;
    Ok(compile_node(&tree))
}

/// Recursively compile a [`SelectorNode`] into a [`Matcher`].
fn compile_node(node: &SelectorNode) -> Matcher {
    match node {
        SelectorNode::Kind(name) => {
            // `parse` already validated the name; resolution is infallible
            // here, but we fall back to a never-match constant defensively.
            let kind = kind_from_camel(name).unwrap_or(SyntaxKind::TOMBSTONE);
            Box::new(move |n: &Node, _p: Option<&Node>| n.kind == kind)
        }
        SelectorNode::Attribute { target, op, value } => compile_attribute(*target, *op, value),
        SelectorNode::Matches(inner) => {
            let compiled: Vec<Matcher> = inner.iter().map(compile_node).collect();
            Box::new(move |n: &Node, p: Option<&Node>| compiled.iter().any(|m| m(n, p)))
        }
        SelectorNode::Not(inner) => {
            let compiled: Vec<Matcher> = inner.iter().map(compile_node).collect();
            Box::new(move |n: &Node, p: Option<&Node>| !compiled.iter().any(|m| m(n, p)))
        }
        SelectorNode::Child(a, b) => {
            let ma = compile_node(a);
            let mb = compile_node(b);
            Box::new(move |n: &Node, p: Option<&Node>| {
                // B must hold on the current node; A must hold on the
                // direct parent.
                if !mb(n, p) {
                    return false;
                }
                match p {
                    Some(parent) => ma(parent, parent.parent),
                    None => false,
                }
            })
        }
        SelectorNode::Descendant(a, b) => {
            let ma = compile_node(a);
            let mb = compile_node(b);
            Box::new(move |n: &Node, p: Option<&Node>| {
                if !mb(n, p) {
                    return false;
                }
                // Walk the ancestor chain (carried by the Node view) and
                // succeed as soon as some ancestor matches A.
                let mut cur = p;
                while let Some(parent) = cur {
                    if ma(parent, parent.parent) {
                        return true;
                    }
                    cur = parent.parent;
                }
                false
            })
        }
    }
}

/// Compile an `[target op value]` predicate. `Kind` resolves the
/// camelCase value back through [`kind_from_camel`] at compile time so a
/// runtime check is again a `kind ==` comparison (errors are already raised
/// by the parser; an unresolvable value just never matches).
fn compile_attribute(target: AttrKind, op: AttrOp, value: &AttrValue) -> Matcher {
    match (target, op, value) {
        (AttrKind::NameValue, AttrOp::Eq, AttrValue::Str(s)) => {
            let s = s.clone();
            Box::new(move |n: &Node, _| n.name.as_deref().is_some_and(|name| name == s.as_str()))
        }
        (AttrKind::NameValue, AttrOp::RegexMatch, AttrValue::Regex(r)) => {
            let r = r.clone();
            Box::new(move |n: &Node, _| n.name.as_deref().is_some_and(|name| r.is_match(name)))
        }
        (AttrKind::DescriptionValue, AttrOp::Eq, AttrValue::Str(s)) => {
            let s = s.clone();
            Box::new(move |n: &Node, _| n.description.is_some_and(|d| d == s.as_str()))
        }
        (AttrKind::DescriptionValue, AttrOp::RegexMatch, AttrValue::Regex(r)) => {
            let r = r.clone();
            Box::new(move |n: &Node, _| n.description.is_some_and(|d| r.is_match(d)))
        }
        (AttrKind::ValueRaw, AttrOp::Eq, AttrValue::Str(s)) => {
            let s = s.clone();
            Box::new(move |n: &Node, _| n.value_raw.is_some_and(|v| v == s.as_str()))
        }
        (AttrKind::ValueRaw, AttrOp::RegexMatch, AttrValue::Regex(r)) => {
            let r = r.clone();
            Box::new(move |n: &Node, _| n.value_raw.is_some_and(|v| r.is_match(v)))
        }
        (AttrKind::Kind, AttrOp::Eq, AttrValue::Str(s)) => {
            let kind = kind_from_camel(s).unwrap_or(SyntaxKind::TOMBSTONE);
            Box::new(move |n: &Node, _| n.kind == kind)
        }
        (AttrKind::Kind, AttrOp::RegexMatch, AttrValue::Regex(r)) => {
            // Regex-match against a Kind: match the SCREAMING_SNAKE_CASE
            // spelling (`format!("{:?}", kind)`) against the pattern. Less
            // common than `[kind=...]` but supported for parity.
            let r = r.clone();
            Box::new(move |n: &Node, _| r.is_match(&format!("{:?}", n.kind)))
        }
        // Mismatched op/value combinations (e.g. `=~ "string"`) are
        // rejected by the parser, but defensively never match here.
        _ => Box::new(|_: &Node, _| false),
    }
}

/// Resolve a graphql-eslint camelCase kind name to its
/// [`apollo_parser::SyntaxKind`] variant.
///
/// This is a static (manual) table covering every AST-level kind a selector
/// is expected to reference: object / interface / union / enum / scalar /
/// input / directive definitions and their extensions, field / argument /
/// enum-value definitions, and the executable-document node kinds (field,
/// argument, fragment, etc.). Token-level kinds (punctuation, keywords,
/// literals) are included where they overlap with graphql-eslint AST kinds
/// (`NAME`, `STRING_VALUE`, `INT_VALUE`, …) since rules occasionally select
/// on them.
///
/// Returns `None` for unknown names so the parser can raise a
/// [`SelectorError::UnknownKind`] with a span.
pub(crate) fn kind_from_camel(name: &str) -> Option<SyntaxKind> {
    use SyntaxKind::*;
    Some(match name {
        // --- Type-system definitions / extensions -------------------------
        "SchemaDefinition" => SCHEMA_DEFINITION,
        "ScalarTypeDefinition" => SCALAR_TYPE_DEFINITION,
        "ObjectTypeDefinition" => OBJECT_TYPE_DEFINITION,
        "InterfaceTypeDefinition" => INTERFACE_TYPE_DEFINITION,
        "UnionTypeDefinition" => UNION_TYPE_DEFINITION,
        "EnumTypeDefinition" => ENUM_TYPE_DEFINITION,
        "InputObjectTypeDefinition" => INPUT_OBJECT_TYPE_DEFINITION,
        "DirectiveDefinition" => DIRECTIVE_DEFINITION,
        "TypeDefinition" => TYPE_DEFINITION,
        "Definition" => DEFINITION,
        "ExecutableDefinition" => EXECUTABLE_DEFINITION,
        "TypeSystemDefinition" => TYPE_SYSTEM_DEFINITION,

        "SchemaExtension" => SCHEMA_EXTENSION,
        "ScalarTypeExtension" => SCALAR_TYPE_EXTENSION,
        "ObjectTypeExtension" => OBJECT_TYPE_EXTENSION,
        "InterfaceTypeExtension" => INTERFACE_TYPE_EXTENSION,
        "UnionTypeExtension" => UNION_TYPE_EXTENSION,
        "EnumTypeExtension" => ENUM_TYPE_EXTENSION,
        "InputObjectTypeExtension" => INPUT_OBJECT_TYPE_EXTENSION,
        "TypeExtension" => TYPE_EXTENSION,
        "TypeSystemExtension" => TYPE_SYSTEM_EXTENSION,

        "RootOperationTypeDefinition" => ROOT_OPERATION_TYPE_DEFINITION,
        "ImplementsInterfaces" => IMPLEMENTS_INTERFACES,
        "FieldsDefinition" => FIELDS_DEFINITION,
        "FieldDefinition" => FIELD_DEFINITION,
        "ArgumentsDefinition" => ARGUMENTS_DEFINITION,
        "UnionMemberTypes" => UNION_MEMBER_TYPES,
        "EnumValuesDefinition" => ENUM_VALUES_DEFINITION,
        "EnumValueDefinition" => ENUM_VALUE_DEFINITION,
        "InputFieldsDefinition" => INPUT_FIELDS_DEFINITION,
        "InputValueDefinition" => INPUT_VALUE_DEFINITION,
        "DirectiveLocations" => DIRECTIVE_LOCATIONS,
        "DirectiveLocation" => DIRECTIVE_LOCATION,
        "ExecutableDirectiveLocation" => EXECUTABLE_DIRECTIVE_LOCATION,
        "TypeSystemDirectiveLocation" => TYPE_SYSTEM_DIRECTIVE_LOCATION,
        "Description" => DESCRIPTION,

        // --- Executable document ----------------------------------------
        "Document" => DOCUMENT,
        "OperationDefinition" => OPERATION_DEFINITION,
        "OperationType" => OPERATION_TYPE,
        "FragmentDefinition" => FRAGMENT_DEFINITION,
        "FragmentName" => FRAGMENT_NAME,
        "TypeCondition" => TYPE_CONDITION,
        "VariableDefinitions" => VARIABLE_DEFINITIONS,
        "VariableDefinition" => VARIABLE_DEFINITION,
        "Variable" => VARIABLE,
        "DefaultValue" => DEFAULT_VALUE,
        "SelectionSet" => SELECTION_SET,
        "Selection" => SELECTION,
        "Field" => FIELD,
        "FragmentSpread" => FRAGMENT_SPREAD,
        "InlineFragment" => INLINE_FRAGMENT,
        "Alias" => ALIAS,
        "Arguments" => ARGUMENTS,
        "Argument" => ARGUMENT,
        "Directives" => DIRECTIVES,
        "Directive" => DIRECTIVE,

        "Type" => TYPE,
        "NamedType" => NAMED_TYPE,
        "ListType" => LIST_TYPE,
        "NonNullType" => NON_NULL_TYPE,

        // --- Value literal kinds (overlap with graphql-eslint AST) ------
        "Value" => VALUE,
        "Name" => NAME,
        "StringValue" => STRING_VALUE,
        "IntValue" => INT_VALUE,
        "FloatValue" => FLOAT_VALUE,
        "BooleanValue" => BOOLEAN_VALUE,
        "NullValue" => NULL_VALUE,
        "EnumValue" => ENUM_VALUE,
        "ListValue" => LIST_VALUE,
        "ObjectValue" => OBJECT_VALUE,
        "ObjectField" => OBJECT_FIELD,
        _ => return None,
    })
}
