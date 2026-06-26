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

## Scope

**In scope:**

- `is_one_of_input(t) -> bool` — input object type with the `@oneOf` directive
  (on the type, not fields).
- `one_of_fields(t) -> Vec<&FieldDefinition>` — the input's fields.
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
pub fn one_of_fields<'s>(t: &'s ast::InputObjectTypeDefinition) -> Vec<&'s ast::InputValueDefinition>;
```

## Behavior

- Detects `@oneOf` regardless of directive argument presence.
- Returns `false` for non-input types.

## Testing

- Unit: `input Foo @oneOf { a: String, b: Int }` → `is_one_of_input` true,
  `one_of_fields` len 2; `input Bar { a: String }` → false.

## Risks / Notes

- `@oneOf` is a client convention (not in the GraphQL spec until 2025); the
  directive must be declared in the schema or it's a schema error (apollo-
  compiler may flag — that's spec-053's concern, not here).
