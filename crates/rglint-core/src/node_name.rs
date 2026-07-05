//! `node_name(node) -> Option<String>` — the single most widely depended-upon
//! shared helper in `graphql-eslint` (its `getNodeName`). Rules, the selector
//! engine, and reporters alike ask a node for its name; this module gives them
//! one stable place to do so.
//!
//! ## Status
//!
//! Spec-012. `Node` (spec-008 / spec-010) already carries a precomputed
//! `name: Option<&'a str>` borrowed from the originating CST, populated by the
//! engine walk (spec-011) for every node kind that *can* bear a name. This
//! helper is therefore a thin projection today: it widens the `&str` to an
//! owned `String` and returns `None` for nameless nodes (anonymous operations,
//! `SelectionSet`, `Argument`, …) — exactly the contract:
//!
//! > `node_name` never panics; returns `None` for nodes without a name token.
//!
//! Keeping the projection here (rather than inline at call sites) preserves the
//! stable signature the rest of the codebase is written against: when a later
//! spec pushes the real `apollo_compiler` typed AST into `Node`, only this
//! module needs to learn how to read a name off each kind — every caller keeps
//! calling `node_name(&node)`.

use crate::node::Node;

/// Return the name string of `node`, or `None` for nameless nodes.
///
/// Covers every named definition / executable node kind that
/// `graphql-eslint`'s `getNodeName` does: type-system definitions
/// (`ObjectTypeDefinition`, `InterfaceTypeDefinition`, `UnionTypeDefinition`,
/// `EnumTypeDefinition`, `ScalarTypeDefinition`,
/// `InputObjectTypeDefinition`, `DirectiveDefinition`, `SchemaDefinition`),
/// their members (`FieldDefinition`, `InputValueDefinition`,
/// `EnumValueDefinition`, `Argument`), and executable definitions
/// (`OperationDefinition`, `FragmentDefinition`, `Field`, `FragmentSpread`,
/// `NamedType`, …). Anonymous operations and other nameless nodes yield `None`.
///
/// Owns the returned string so callers don't have to juggle lifetimes from a
/// borrowed CST node; the common case is a short identifier, so the
/// allocation is negligible. Never panics.
pub fn node_name(node: &Node<'_>) -> Option<String> {
    node.name.map(|s| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use apollo_parser::SyntaxKind;

    /// Build a node of `kind` carrying `name` (or no name when `name` is `None`).
    fn named<'a>(kind: SyntaxKind, name: Option<&'a str>) -> Node<'a> {
        let mut n = Node::new(kind);
        if let Some(s) = name {
            n = n.with_name(s);
        }
        n
    }

    #[test]
    fn returns_some_for_named_definition_kinds() {
        // One fixture per named node kind, mirroring graphql-eslint's
        // `getNodeName` coverage. The engine (spec-011) populates `Node::name`
        // for exactly these kinds; the helper just unwraps the projection.
        let cases: &[(SyntaxKind, &str)] = &[
            (SyntaxKind::OBJECT_TYPE_DEFINITION, "Query"),
            (SyntaxKind::INTERFACE_TYPE_DEFINITION, "Node"),
            (SyntaxKind::UNION_TYPE_DEFINITION, "SearchResult"),
            (SyntaxKind::ENUM_TYPE_DEFINITION, "Color"),
            (SyntaxKind::SCALAR_TYPE_DEFINITION, "DateTime"),
            (SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION, "UserInput"),
            (SyntaxKind::DIRECTIVE_DEFINITION, "deprecated"),
            (SyntaxKind::FIELD_DEFINITION, "id"),
            (SyntaxKind::INPUT_VALUE_DEFINITION, "name"),
            (SyntaxKind::ENUM_VALUE_DEFINITION, "RED"),
            (SyntaxKind::ARGUMENT, "where"),
            (SyntaxKind::OPERATION_DEFINITION, "GetUser"),
            (SyntaxKind::FRAGMENT_DEFINITION, "UserFields"),
            (SyntaxKind::FIELD, "id"),
            (SyntaxKind::FRAGMENT_SPREAD, "UserFields"),
            (SyntaxKind::NAMED_TYPE, "User"),
        ];
        for (kind, name) in cases {
            let node = named(*kind, Some(name));
            assert_eq!(
                node_name(&node).as_deref(),
                Some(*name),
                "node_name should return the name for {kind:?}"
            );
        }
    }

    #[test]
    fn returns_none_for_anonymous_operation() {
        // An OperationDefinition with no name token yields `None` — the
        // primary anonymous case `no-anonymous-operations` (spec-016) keys on.
        let node = named(SyntaxKind::OPERATION_DEFINITION, None);
        assert_eq!(node_name(&node), None);
    }

    #[test]
    fn returns_none_for_inherently_nameless_nodes() {
        // SelectionSet / Arguments / Document etc. have no name token at all.
        for kind in [
            SyntaxKind::SELECTION_SET,
            SyntaxKind::ARGUMENTS,
            SyntaxKind::DOCUMENT,
            SyntaxKind::DIRECTIVES,
        ] {
            let node = Node::new(kind);
            assert_eq!(node_name(&node), None, "{kind:?} has no name");
        }
    }

    #[test]
    fn never_panics_on_unpopulated_node() {
        // The placeholder `Node::new` (no name set) must be safe to ask.
        let node = Node::new(SyntaxKind::NAME);
        assert_eq!(node_name(&node), None);
    }

    #[test]
    fn returns_owned_string() {
        // The helper owns its output so callers don't need to thread the
        // CST lifetime through. Asserting `String` keeps the crate honest if
        // someone tries to "optimize" this to return `&'a str`.
        let node = named(SyntaxKind::OBJECT_TYPE_DEFINITION, Some("Query"));
        let name: Option<String> = node_name(&node);
        assert_eq!(name.as_deref(), Some("Query"));
    }
}
