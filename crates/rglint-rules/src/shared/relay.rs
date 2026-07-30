//! Shared predicates for the Relay cursor-connection rules (spec-044).
//!
//! The rule engine works with Apollo Compiler's merged schema model rather
//! than the parser's per-definition AST.  That lets these helpers see fields
//! contributed by type extensions and makes the returned types usable by the
//! schema rules that consume this module.

use apollo_compiler::{ast, schema, Schema};
use regex::Regex;

/// Naming and type-name options shared by the Relay rules.
#[derive(Debug, Clone)]
pub struct RelayOpts {
    /// Pattern identifying Connection object types.
    pub connection_pattern: Regex,
    /// Pattern identifying Edge object types.
    pub edge_pattern: Regex,
    /// Exact name of the PageInfo object type.
    pub page_info_name: String,
}

impl Default for RelayOpts {
    fn default() -> Self {
        Self {
            connection_pattern: Regex::new(r"Connection$").expect("valid Relay default regex"),
            edge_pattern: Regex::new(r"Edge$").expect("valid Relay default regex"),
            page_info_name: "PageInfo".to_owned(),
        }
    }
}

/// Returns whether an object has a field with the supplied name.
fn has_field(object: &schema::ObjectType, field_name: &str) -> bool {
    object.fields.contains_key(field_name)
}

/// Returns whether `object` has the Relay Connection name and required fields.
///
/// Shape-specific checks (for example, whether `edges` is a list) belong to
/// `relay-connection-types`; this predicate deliberately remains useful when
/// that rule is reporting a malformed Connection.
pub fn is_connection_type(object: &schema::ObjectType, opts: &RelayOpts) -> bool {
    opts.connection_pattern.is_match(object.name.as_str())
        && has_field(object, "edges")
        && has_field(object, "pageInfo")
}

/// Returns whether `object` has the Relay Edge name and required fields.
pub fn is_edge_type(object: &schema::ObjectType, opts: &RelayOpts) -> bool {
    opts.edge_pattern.is_match(object.name.as_str())
        && has_field(object, "node")
        && has_field(object, "cursor")
}

/// Returns whether `object` has the configured PageInfo name and all four
/// standard fields.
pub fn is_page_info_type(object: &schema::ObjectType, opts: &RelayOpts) -> bool {
    object.name.as_str() == opts.page_info_name.as_str()
        && ["hasNextPage", "hasPreviousPage", "startCursor", "endCursor"]
            .iter()
            .all(|field_name| has_field(object, field_name))
}

/// Resolve a field's return type when it is a configured Connection object.
pub fn connection_for_field<'schema>(
    field: &ast::FieldDefinition,
    schema: &'schema Schema,
    opts: &RelayOpts,
) -> Option<&'schema schema::ObjectType> {
    let type_name = field.ty.inner_named_type();
    if !opts.connection_pattern.is_match(type_name.as_str()) {
        return None;
    }

    match schema.types.get(type_name) {
        Some(schema::ExtendedType::Object(object)) => Some(object),
        _ => None,
    }
}

/// Resolve the object type wrapped by a Connection's `edges` list.
pub fn edge_of_connection<'schema>(
    connection: &schema::ObjectType,
    schema: &'schema Schema,
) -> Option<&'schema schema::ObjectType> {
    let edges = connection.fields.get("edges")?;
    if !edges.ty.is_list() {
        return None;
    }

    match schema.types.get(edges.ty.inner_named_type()) {
        Some(schema::ExtendedType::Object(object)) => Some(object),
        _ => None,
    }
}

/// Compatibility alias for callers that use the shorter helper name from the
/// spec prose.
pub fn edge_for_connection<'schema>(
    connection: &schema::ObjectType,
    schema: &'schema Schema,
) -> Option<&'schema schema::ObjectType> {
    edge_of_connection(connection, schema)
}

fn has_argument(field: &ast::FieldDefinition, argument_name: &str) -> bool {
    field
        .arguments
        .iter()
        .any(|argument| argument.name == argument_name)
}

/// Whether a field has both forward pagination arguments (`first` and
/// `after`).
pub fn has_forward_pagination(field: &ast::FieldDefinition) -> bool {
    has_argument(field, "first") && has_argument(field, "after")
}

/// Whether a field has both backward pagination arguments (`last` and
/// `before`).
pub fn has_backward_pagination(field: &ast::FieldDefinition) -> bool {
    has_argument(field, "last") && has_argument(field, "before")
}

/// Whether a field has forward pagination and no complete backward pair.
pub fn is_forward_only(field: &ast::FieldDefinition) -> bool {
    has_forward_pagination(field) && !has_backward_pagination(field)
}

/// Whether a field has backward pagination and no complete forward pair.
pub fn is_backward_only(field: &ast::FieldDefinition) -> bool {
    has_backward_pagination(field) && !has_forward_pagination(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELAY_SCHEMA: &str = include_str!("fixtures/relay/schema.graphqls");

    fn schema() -> Schema {
        Schema::parse_and_validate(RELAY_SCHEMA, "relay/schema.graphqls")
            .expect("Relay fixture should parse and validate")
            .into_inner()
    }

    fn object<'a>(schema: &'a Schema, name: &str) -> &'a schema::ObjectType {
        match schema.types.get(name) {
            Some(schema::ExtendedType::Object(object)) => object,
            other => panic!("{name} should be an object, got {other:?}"),
        }
    }

    fn field<'a>(
        schema: &'a Schema,
        object_name: &str,
        field_name: &str,
    ) -> &'a ast::FieldDefinition {
        object(schema, object_name)
            .fields
            .get(field_name)
            .expect("fixture field should exist")
            .as_ref()
    }

    #[test]
    fn default_predicates_require_names_and_fields() {
        let schema = schema();
        let opts = RelayOpts::default();

        assert!(is_connection_type(object(&schema, "UserConnection"), &opts));
        assert!(!is_connection_type(object(&schema, "BadConnection"), &opts));
        assert!(is_edge_type(object(&schema, "UserEdge"), &opts));
        assert!(!is_edge_type(object(&schema, "BadEdge"), &opts));
        assert!(is_page_info_type(object(&schema, "PageInfo"), &opts));
        assert!(!is_page_info_type(
            object(&schema, "IncompletePageInfo"),
            &opts
        ));
    }

    #[test]
    fn resolves_connections_and_edges_from_schema() {
        let schema = schema();
        let opts = RelayOpts::default();
        let connection = connection_for_field(field(&schema, "Query", "users"), &schema, &opts)
            .expect("users should return a Connection");

        assert_eq!(connection.name, "UserConnection");
        assert_eq!(
            edge_of_connection(connection, &schema).unwrap().name,
            "UserEdge"
        );
        assert_eq!(
            edge_for_connection(connection, &schema).unwrap().name,
            "UserEdge"
        );
        assert!(connection_for_field(field(&schema, "Query", "health"), &schema, &opts).is_none());
    }

    #[test]
    fn classifies_complete_pagination_pairs() {
        let schema = schema();

        assert!(is_forward_only(field(&schema, "Query", "forwardUsers")));
        assert!(is_backward_only(field(&schema, "Query", "backwardUsers")));
        assert!(!is_forward_only(field(&schema, "Query", "users")));
        assert!(!is_backward_only(field(&schema, "Query", "users")));
    }

    #[test]
    fn custom_names_are_supported() {
        let schema = schema();
        let opts = RelayOpts {
            connection_pattern: Regex::new(r"ConnectionType$").unwrap(),
            edge_pattern: Regex::new(r"EdgeType$").unwrap(),
            page_info_name: "PagingInfo".to_owned(),
        };

        assert!(is_connection_type(
            object(&schema, "UserConnectionType"),
            &opts
        ));
        assert!(is_edge_type(object(&schema, "UserEdgeType"), &opts));
        assert!(is_page_info_type(object(&schema, "PagingInfo"), &opts));
    }
}
