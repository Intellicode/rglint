//! Shared helpers for GraphQL input objects annotated with `@oneOf` (spec-049).
//!
//! These helpers intentionally use Apollo Compiler's source AST. The
//! consuming oneOf rules inspect the annotation on the local definition, so a
//! merged schema type would lose the distinction between the definition and
//! any extensions that contribute fields.

use apollo_compiler::ast;

const ONE_OF_DIRECTIVE: &str = "oneOf";

/// Returns whether `input` has a `@oneOf` directive on the input object.
pub fn is_one_of_input(input: &ast::InputObjectTypeDefinition) -> bool {
    input.directives.has(ONE_OF_DIRECTIVE)
}

/// Returns the fields declared by an input object, preserving source order.
pub fn one_of_fields(input: &ast::InputObjectTypeDefinition) -> Vec<&ast::InputValueDefinition> {
    input.fields.iter().map(|field| field.as_ref()).collect()
}

/// Returns a specified argument from the input object's `@oneOf` directive.
///
/// `@oneOf` currently has no standard arguments, but keeping this accessor
/// scoped to the directive makes future extensions available to consumers
/// without duplicating directive traversal. Defaults are deliberately not
/// resolved because this helper has no schema context.
pub fn directive_arg<'s>(
    input: &'s ast::InputObjectTypeDefinition,
    argument_name: &str,
) -> Option<&'s ast::Value> {
    input
        .directives
        .get(ONE_OF_DIRECTIVE)?
        .specified_argument_by_name(argument_name)
        .map(|value| &**value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_OF_SCHEMA: &str = include_str!("fixtures/oneof/schema.graphqls");

    fn input<'a>(document: &'a ast::Document, name: &str) -> &'a ast::InputObjectTypeDefinition {
        document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                ast::Definition::InputObjectTypeDefinition(input) if input.name == name => {
                    Some(input.as_ref())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("input {name} not found"))
    }

    #[test]
    fn detects_oneof_and_returns_fields_in_source_order() {
        let document = ast::Document::parse(ONE_OF_SCHEMA, "oneof/schema.graphqls")
            .expect("oneOf fixture should parse");
        let foo = input(&document, "Foo");
        let bar = input(&document, "Bar");

        assert!(is_one_of_input(foo));
        assert_eq!(
            one_of_fields(foo)
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(!is_one_of_input(bar));
        assert_eq!(one_of_fields(bar).len(), 1);
    }

    #[test]
    fn reads_a_specified_directive_argument_without_requiring_schema() {
        let document = ast::Document::parse(ONE_OF_SCHEMA, "oneof/schema.graphqls")
            .expect("oneOf fixture should parse");
        let foo = input(&document, "Foo");

        assert!(matches!(
            directive_arg(foo, "label"),
            Some(ast::Value::String(value)) if value.as_str() == "fixture"
        ));
        assert!(directive_arg(foo, "missing").is_none());
    }
}
