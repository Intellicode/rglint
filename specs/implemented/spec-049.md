# Spec-049: shared/oneof.rs helpers

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/shared/oneof.rs`)

## Goal

Port the `@oneOf` directive helper predicates shared by
`require-nullable-fields-with-oneof` (spec-050) and
`require-type-pattern-with-oneof` (spec-051): detect whether an input type is
`@oneOf`-annotated and walk its fields accordingly.

## Source

`packages/plugin/src/rules/require-nullable-fields-with-oneof/index.ts` and
`require-type-pattern-with-oneof/index.ts` shared `@oneOf` detection.

Parity was checked against graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24`:

- [require-nullable-fields-with-oneof source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-nullable-fields-with-oneof/index.ts)
- [require-type-pattern-with-oneof source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-type-pattern-with-oneof/index.ts)
- [oneOf rule tests](https://github.com/graphql-hive/graphql-eslint/tree/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules)

## Scope

**In scope:**

- `is_one_of_input(t) -> bool` — input object type with the `@oneOf` directive
  (on the type, not fields).
- `one_of_fields(t) -> Vec<&InputValueDefinition>` — the input's fields.
- `directive_arg` accessor for `@oneOf` (currently no args, but keep the helper
  future-proof).
- Option: the directive name is fixed `oneOf` (graphql-eslint convention —
  confirm no custom-name option).

**Out of scope:**

- The two consuming rules (specs 050, 051).

## Dependencies

- spec-004 (Schema).

## Deliverables

- `crates/rglint-rules/src/shared/oneof.rs`.
- Re-export from `shared/mod.rs`.
- Unit test over a fixture with one `@oneOf` input and one plain input.

## Interface / API

```rust
pub fn is_one_of_input(t: &ast::InputObjectTypeDefinition) -> bool;
pub fn one_of_fields(t: &ast::InputObjectTypeDefinition) -> Vec<&ast::InputValueDefinition>;
pub fn directive_arg<'s>(t: &'s ast::InputObjectTypeDefinition, name: &str) -> Option<&'s ast::Value>;
```

## Behavior

- Detects `@oneOf` regardless of directive argument presence.
- A plain input definition is not a oneOf input; object-type handling remains
  with the consuming output rule in spec-051.

The upstream rules also inspect `type` definitions. This shared helper is
intentionally input-specific because its public API is designed for the
input-field rule in spec-050; spec-051 will add the corresponding output-type
logic without widening this helper's source-AST contract.

## Testing

- Unit: `input Foo @oneOf { a: String, b: Int }` → `is_one_of_input` true,
  `one_of_fields` len 2; `input Bar { a: String }` → false.

## Risks / Notes

- `@oneOf` is a client convention (not in the GraphQL spec until 2025); the
  directive must be declared in the schema or it's a schema error (apollo-
  compiler may flag — that's spec-053's concern, not here).
