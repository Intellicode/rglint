# Spec-050: require-nullable-fields-with-oneof

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/require_nullable_fields_with_oneof.rs`)

## Goal

Port `require-nullable-fields-with-oneof`: fields on an `@oneOf` input object
or object type must be nullable. The oneOf semantics is “exactly one is
provided”, which an outer non-null wrapper would break.

## Upstream parity

Parity is pinned to graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24`:

- [rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-nullable-fields-with-oneof/index.ts)
- [rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-nullable-fields-with-oneof/index.test.ts)

The upstream message is
`field "${field}" in ${container} "${type}" must be nullable when "@oneOf" is in use`.
The selector visits both `INPUT_OBJECT_TYPE_DEFINITION` and
`OBJECT_TYPE_DEFINITION`; the Rust handler preserves source ownership and
reports the field-name span. Only the outer `NonNull` wrapper is rejected, so
`[String!]` remains valid while `[String]!` is reported.

## Scope

**In scope:**

- Rule id `require-nullable-fields-with-oneof`, category `Schema`.
- `@oneOf` fields on input objects and object types.
- `requires_schema: true`, with local source definitions matched to compiled
  schema field types.

**Out of scope:**

- Type-pattern enforcement (spec-051).

## Dependencies

- spec-049, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_nullable_fields_with_oneof.rs`.
- `rules-fixtures/require-nullable-fields-with-oneof/`.
- `crates/rglint-rules/tests/rule_require_nullable_fields_with_oneof.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(
    id = "require-nullable-fields-with-oneof",
    category = "schema",
    requires_schema = true,
    kinds = "DIRECTIVE|NAME"
)]
pub struct RequireNullableFieldsWithOneof;
```

## Fixture ownership

Fixtures use `kind = "schema"`, and declare the custom `@oneOf` directive in
the SDL for both `INPUT_OBJECT` and `OBJECT` locations. This keeps schema
validation separate from rule diagnostics and exercises the local-definition
matching path.

## Testing

- `cargo test -p rglint-rules --test rule_require_nullable_fields_with_oneof`.
- Workspace build, clippy, and test commands before handoff.
