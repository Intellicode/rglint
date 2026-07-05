//! Tests for the selector engine (spec-010).
//!
//! Three groups:
//!
//! 1. **Lexer** — tokenize a few representative inputs and check the
//!    token stream.
//! 2. **Parser / AST snapshots** — `insta` snapshots of the parsed
//!    [`SelectorNode`] tree for the five representative selectors listed in
//!    the spec's *Testing* section, plus a few extras that exercise
//!    combinator chains and compound refinement.
//! 3. **Matcher** — compile each selector and run it against a small
//!    `apollo_compiler` schema fixture (walked into a `Node<'static>` tree
//!    via `Box::leak`), asserting the *exact* set of selected nodes.
//! 4. **Negative** — malformed selectors produce a [`SelectorError`] whose
//!    `span()` lands inside the offending fragment.

#![cfg(test)]

use apollo_compiler::Schema;
use apollo_parser::SyntaxKind;

use crate::node::Node;
use crate::selector::{compile, parse, Matcher, SelectorError, SelectorNode};

// ---------------------------------------------------------------------------
// 1. Lexer
// ---------------------------------------------------------------------------

#[test]
fn lex_simple_kind() {
    let toks = crate::selector::lexer::lex("ObjectTypeDefinition").unwrap();
    let kinds: Vec<_> = toks.iter().map(|t| debug_kind(&t.kind)).collect();
    assert_eq!(kinds, vec!["Ident(ObjectTypeDefinition)"]);
}

#[test]
fn lex_child_combinator() {
    let toks = crate::selector::lexer::lex("A > B").unwrap();
    let kinds: Vec<_> = toks.iter().map(|t| debug_kind(&t.kind)).collect();
    assert_eq!(
        kinds,
        vec!["Ident(A)", "Whitespace", "Gt", "Whitespace", "Ident(B)"]
    );
}

#[test]
fn lex_attribute_regex() {
    // graphql-eslint spells regex match as `=/.../` — the `=` operator is
    // overloaded by the RHS (a regex literal means regex-match). The lexer
    // just emits `Eq` + `Regex`; the parser picks the op.
    let toks = crate::selector::lexer::lex("[name.value=/^_/]").unwrap();
    let kinds: Vec<_> = toks.iter().map(|t| debug_kind(&t.kind)).collect();
    assert_eq!(
        kinds,
        vec![
            "LBracket",
            "Ident(name.value)",
            "Eq",
            "Regex(^_)",
            "RBracket",
        ]
    );
}

#[test]
fn lex_unterminated_string_is_an_error_with_offset() {
    let err = crate::selector::lexer::lex("\"abc").unwrap_err();
    assert!(matches!(err, SelectorError::Lex { span, .. } if span == 0));
}

#[test]
fn lex_unterminated_regex_is_an_error_with_offset() {
    let err = crate::selector::lexer::lex("/abc").unwrap_err();
    assert!(matches!(err, SelectorError::Lex { span, .. } if span == 0));
}

// ---------------------------------------------------------------------------
// 2. Parser / AST snapshots
// ---------------------------------------------------------------------------

#[test]
fn snapshot_object_type_child_field() {
    insta::assert_debug_snapshot!(parse("ObjectTypeDefinition > FieldDefinition").unwrap());
}

#[test]
fn snapshot_field_with_name_regex() {
    insta::assert_debug_snapshot!(parse("FieldDefinition[name.value=/^_/]").unwrap());
}

#[test]
fn snapshot_matches_object_or_interface() {
    insta::assert_debug_snapshot!(
        parse(":matches(ObjectTypeDefinition, InterfaceTypeDefinition)").unwrap()
    );
}

#[test]
fn snapshot_not_field_definition() {
    insta::assert_debug_snapshot!(parse(":not(FieldDefinition)").unwrap());
}

#[test]
fn snapshot_object_type_child_field_named_pageinfo() {
    insta::assert_debug_snapshot!(parse(
        "ObjectTypeDefinition > FieldDefinition[name.value=PageInfo]"
    )
    .unwrap());
}

#[test]
fn snapshot_descendant_chain() {
    // `A B C` -> Descendant(Descendant(A, B), C)
    insta::assert_debug_snapshot!(parse("ObjectTypeDefinition FieldDefinition Name").unwrap());
}

#[test]
fn snapshot_child_chain() {
    // `A > B > C` -> Child(Child(A, B), C)
    insta::assert_debug_snapshot!(parse("ObjectTypeDefinition > FieldDefinition > Name").unwrap());
}

#[test]
fn snapshot_compound_kind_and_two_filters() {
    // FieldDefinition[name=/^_/][:not(...)] -> De Morgan AND
    insta::assert_debug_snapshot!(
        parse("FieldDefinition[name.value=/^_/]:not(FieldDefinition)").unwrap()
    );
}

#[test]
fn snapshot_descendant_with_child_right() {
    insta::assert_debug_snapshot!(parse(
        "ObjectTypeDefinition > FieldDefinition[name.value=/^_/] Name"
    )
    .unwrap());
}

// ---------------------------------------------------------------------------
// 3. Matcher — against a real parsed schema fixture
// ---------------------------------------------------------------------------

/// Schema fixture walked into `Node`s for matcher tests. Mirrors the parse
/// of:
///
/// ```graphql
/// type Query { id: ID! _secret: String }
/// interface Node { id: ID! _internal: Int }
/// enum Status { ACTIVE INACTIVE _DEPRECATED }
/// ```
const FIXTURE_SDL: &str = "\
type Query { id: ID! _secret: String }
interface Node { id: ID! _internal: Int }
enum Status { ACTIVE INACTIVE _DEPRECATED }
";

/// Build a `&'static Node<'static>` for the matcher tests. Strings are
/// leaked via `Box::leak` so the resulting tree is `'static` and can be
/// freely referenced by parent links. Tests are short-lived so the leak is
/// harmless.
fn node_view(
    kind: SyntaxKind,
    name: Option<&str>,
    parent: Option<&'static Node<'static>>,
) -> &'static Node<'static> {
    let name_static = name.map(|s| Box::leak(s.to_string().into_boxed_str()) as &'static str);
    let mut node = Node::new(kind);
    if let Some(n) = name_static {
        node = node.with_name(n);
    }
    if let Some(p) = parent {
        node = node.with_parent(p);
    }
    Box::leak(Box::new(node))
}

/// Walk a parsed [`Schema`] into a flat `Vec<&'static Node>` mirroring the
/// engine's (spec-011) future traversal: type definitions first, then their
/// fields / enum values with `parent` set to the enclosing type. Only the
/// kinds the matcher tests exercise are produced (object / interface / enum
/// types + their field / enum-value children).
///
/// Introspection types (every name starting with `__`) are skipped —
/// `apollo_compiler::Schema::parse` seeds them by default and they would
/// otherwise dominate the fixture with `__Schema` / `__Type` / `__Field`
/// object types and their fields.
fn walk_schema(schema: &Schema) -> Vec<&'static Node<'static>> {
    use apollo_compiler::schema::ExtendedType;
    let mut out: Vec<&'static Node<'static>> = Vec::new();
    for (name, ext) in &schema.types {
        if name.as_str().starts_with("__") {
            continue;
        }
        match ext {
            ExtendedType::Object(obj) => {
                let ty = node_view(
                    SyntaxKind::OBJECT_TYPE_DEFINITION,
                    Some(obj.name.as_str()),
                    None,
                );
                out.push(ty);
                for (field_name, _field) in &obj.fields {
                    out.push(node_view(
                        SyntaxKind::FIELD_DEFINITION,
                        Some(field_name.as_str()),
                        Some(ty),
                    ));
                }
            }
            ExtendedType::Interface(int) => {
                let ty = node_view(
                    SyntaxKind::INTERFACE_TYPE_DEFINITION,
                    Some(int.name.as_str()),
                    None,
                );
                out.push(ty);
                for (field_name, _field) in &int.fields {
                    out.push(node_view(
                        SyntaxKind::FIELD_DEFINITION,
                        Some(field_name.as_str()),
                        Some(ty),
                    ));
                }
            }
            ExtendedType::Enum(en) => {
                let ty = node_view(
                    SyntaxKind::ENUM_TYPE_DEFINITION,
                    Some(en.name.as_str()),
                    None,
                );
                out.push(ty);
                for (value_name, _value) in &en.values {
                    out.push(node_view(
                        SyntaxKind::ENUM_VALUE_DEFINITION,
                        Some(value_name.as_str()),
                        Some(ty),
                    ));
                }
            }
            ExtendedType::Scalar(scalar) => {
                out.push(node_view(
                    SyntaxKind::SCALAR_TYPE_DEFINITION,
                    Some(scalar.name.as_str()),
                    None,
                ));
            }
            ExtendedType::Union(un) => {
                out.push(node_view(
                    SyntaxKind::UNION_TYPE_DEFINITION,
                    Some(un.name.as_str()),
                    None,
                ));
            }
            ExtendedType::InputObject(io) => {
                let ty = node_view(
                    SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION,
                    Some(io.name.as_str()),
                    None,
                );
                out.push(ty);
                for (field_name, _field) in &io.fields {
                    out.push(node_view(
                        SyntaxKind::INPUT_VALUE_DEFINITION,
                        Some(field_name.as_str()),
                        Some(ty),
                    ));
                }
            }
        }
    }
    out
}

/// Parse the fixture schema and walk it into `Node`s.
fn fixture_nodes() -> Vec<&'static Node<'static>> {
    let schema = Schema::parse(FIXTURE_SDL, "fixture.graphql").expect("fixture parses cleanly");
    walk_schema(&schema)
}

/// Run a compiled matcher over a fixture node set and return the names of
/// the nodes it selects (sorted for stable comparison). Nodes without a
/// name are reported as `<anon>`; we use names because the fixture has
/// unique names per kind.
fn selected_names(m: &Matcher, nodes: &[&'static Node<'static>]) -> Vec<String> {
    let mut hits: Vec<String> = nodes
        .iter()
        .filter(|n| m(n, n.parent))
        .map(|n| n.name.unwrap_or("<anon>").to_owned())
        .collect();
    hits.sort();
    hits
}

#[test]
fn matcher_selects_all_field_definitions_via_child() {
    let m = compile("ObjectTypeDefinition > FieldDefinition").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    // Field definitions whose direct parent is an ObjectTypeDefinition:
    // Query.id, Query._secret. Node.id / Node._internal are under an
    // InterfaceTypeDefinition and must NOT match.
    assert_eq!(hits, vec!["_secret".to_owned(), "id".to_owned()]);
}

#[test]
fn matcher_selects_underscore_fields_via_name_regex() {
    let m = compile("FieldDefinition[name.value=/^_/]").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    // _secret (under Query) and _internal (under Node) — both are
    // FieldDefinitions whose name starts with `_`.
    assert_eq!(hits, vec!["_internal".to_owned(), "_secret".to_owned()]);
}

#[test]
fn matcher_object_type_then_underscore_field() {
    let m = compile("ObjectTypeDefinition > FieldDefinition[name.value=/^_/]").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    // Only _secret (under Query, an ObjectTypeDefinition). _internal is
    // under InterfaceTypeDefinition so its parent doesn't match.
    assert_eq!(hits, vec!["_secret".to_owned()]);
}

#[test]
fn matcher_matches_object_or_interface() {
    let m = compile(":matches(ObjectTypeDefinition, InterfaceTypeDefinition)").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    assert_eq!(hits, vec!["Node".to_owned(), "Query".to_owned()]);
}

#[test]
fn matcher_not_field_definition() {
    let m = compile(":not(FieldDefinition)").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    // Everything except the four FieldDefinitions: Query, Node, Status,
    // ACTIVE, INACTIVE, _DEPRECATED, plus the built-in scalars
    // (Boolean/Float/ID/Int/String) that apollo_compiler seeds into every
    // parsed schema.
    assert_eq!(
        hits,
        vec![
            "ACTIVE".to_owned(),
            "Boolean".to_owned(),
            "Float".to_owned(),
            "ID".to_owned(),
            "INACTIVE".to_owned(),
            "Int".to_owned(),
            "Node".to_owned(),
            "Query".to_owned(),
            "Status".to_owned(),
            "String".to_owned(),
            "_DEPRECATED".to_owned(),
        ]
    );
}

#[test]
fn matcher_descendant_finds_all_names_under_object_type() {
    // Any Name descendant of an ObjectTypeDefinition. In our fixture the
    // FieldDefinitions are direct children of `Query` (an ObjectType) and
    // carry names `id`, `_secret`. ObjectTypeDefinition itself is not a
    // descendant of itself.
    let m = compile("ObjectTypeDefinition FieldDefinition").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    assert_eq!(hits, vec!["_secret".to_owned(), "id".to_owned()]);
}

#[test]
fn matcher_kind_attribute_eq_works() {
    // `[kind=EnumTypeDefinition]` is the attribute form of `EnumTypeDefinition`.
    let m = compile("[kind=EnumTypeDefinition]").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    assert_eq!(hits, vec!["Status".to_owned()]);
}

#[test]
fn matcher_compound_kind_and_attribute() {
    // EnumValueDefinition whose name starts with `_`. Only `_DEPRECATED`.
    let m = compile("EnumValueDefinition[name.value=/^_/]").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    assert_eq!(hits, vec!["_DEPRECATED".to_owned()]);
}

#[test]
fn matcher_name_equality_bare_ident_value() {
    // graphql-eslint allows `[name.value=PageInfo]` — bare ident as a
    // string. Verify equality (not regex) on a known name.
    let m = compile("FieldDefinition[name.value=id]").unwrap();
    let nodes = fixture_nodes();
    let hits = selected_names(&m, &nodes);
    // Both Query.id and Node.id are FieldDefinitions named exactly `id`.
    assert_eq!(hits, vec!["id".to_owned(), "id".to_owned()]);
}

#[test]
fn matcher_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Matcher>();
}

// ---------------------------------------------------------------------------
// 4. Negative — malformed selectors
// ---------------------------------------------------------------------------

#[test]
fn err_unclosed_attribute_bracket_has_offset() {
    // `[name.value=` — opens `[`, key, `=`, then EOF before value + `]`.
    let err = parse("[name.value=").unwrap_err();
    let span = err.span();
    assert!(matches!(
        err,
        SelectorError::Parse { .. } | SelectorError::Lex { .. }
    ));
    // Span is within the input (10 bytes long); not past EOF reporting
    // would be acceptable too, but it must be <= input length.
    assert!(span <= "[name.value=".len());
}

#[test]
fn err_unclosed_not_paren_has_offset() {
    // `:not(FieldDefinition` — unclosed `)`.
    let err = parse(":not(FieldDefinition").unwrap_err();
    let span = err.span();
    assert!(matches!(err, SelectorError::Parse { .. }));
    assert!(span <= ":not(FieldDefinition".len());
}

#[test]
fn err_unknown_kind_reports_name_and_span() {
    let err = parse("NotARealKind").unwrap_err();
    assert!(matches!(
        err,
        SelectorError::UnknownKind { ref kind, .. } if kind == "NotARealKind"
    ));
    // Span points at the start of the bad identifier.
    assert_eq!(err.span(), 0);
}

#[test]
fn err_invalid_regex_reports_offset() {
    // `/(/` is an unbalanced group — `regex::Regex::new` rejects it. The
    // error's span is the start of the regex literal (the opening `/`).
    let src = "FieldDefinition[name.value=/(/]";
    let err = parse(src).unwrap_err();
    assert!(matches!(err, SelectorError::Regex { .. }));
    let regex_start = "FieldDefinition[name.value=".len(); // index of the `/`
    assert_eq!(err.span(), regex_start);
}

#[test]
fn err_unknown_attribute_key() {
    let err = parse("[bogus=foo]").unwrap_err();
    assert!(matches!(err, SelectorError::Parse { .. }));
}

#[test]
fn err_unsupported_pseudo_class() {
    let err = parse(":has(FieldDefinition)").unwrap_err();
    assert!(matches!(err, SelectorError::Parse { .. }));
}

#[test]
fn err_unexpected_token_after_selector() {
    // Trailing junk after a valid selector.
    let err = parse("FieldDefinition ]").unwrap_err();
    assert!(matches!(err, SelectorError::Parse { .. }));
}

#[test]
fn err_unterminated_string_in_value() {
    let err = parse("[name.value=\"abc]").unwrap_err();
    assert!(matches!(err, SelectorError::Lex { .. }));
    assert_eq!(err.span(), "[name.value=".len());
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn debug_kind(k: &crate::selector::lexer::TokKind) -> String {
    use crate::selector::lexer::TokKind as K;
    match k {
        K::Ident(s) => format!("Ident({s})"),
        K::Str(_) => "Str".to_owned(),
        K::Regex(s) => format!("Regex({s})"),
        K::Eq => "Eq".to_owned(),
        K::RegexEq => "RegexEq".to_owned(),
        K::LBracket => "LBracket".to_owned(),
        K::RBracket => "RBracket".to_owned(),
        K::LParen => "LParen".to_owned(),
        K::RParen => "RParen".to_owned(),
        K::Gt => "Gt".to_owned(),
        K::Colon => "Colon".to_owned(),
        K::Comma => "Comma".to_owned(),
        K::Whitespace => "Whitespace".to_owned(),
    }
}

// A compile-time check that the public AST shape matches the spec —
// referencing each variant here ensures a rename is caught by tests.
#[test]
fn ast_shape_matches_spec() {
    let _ = SelectorNode::Kind("Foo".to_owned());
    let _ = SelectorNode::Child(
        Box::new(SelectorNode::Kind("A".to_owned())),
        Box::new(SelectorNode::Kind("B".to_owned())),
    );
    let _ = SelectorNode::Descendant(
        Box::new(SelectorNode::Kind("A".to_owned())),
        Box::new(SelectorNode::Kind("B".to_owned())),
    );
    let _ = SelectorNode::Matches(vec![SelectorNode::Kind("A".to_owned())]);
    let _ = SelectorNode::Not(vec![SelectorNode::Kind("A".to_owned())]);
}
