# Spec-051: require-type-pattern-with-oneof

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/require_type_pattern_with_oneof.rs`)

## Goal

Port `require-type-pattern-with-oneof`: object types annotated with `@oneOf`
must define both `error` and `ok` output fields.

## Source

`packages/plugin/src/rules/require-type-pattern-with-oneof/index.ts`

## Upstream parity

Parity is pinned to graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24`:

- [rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-type-pattern-with-oneof/index.ts)
- [rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/require-type-pattern-with-oneof/index.test.ts)

The verified upstream rule is narrower than the original draft of this spec:
it checks only `OBJECT_TYPE_DEFINITION` nodes, ignores input objects and types
without `@oneOf`, and reports one diagnostic for each missing field in the
order `error`, then `ok`. The exact message is
`type "T" is defined as output with "@oneOf" and must be defined with "error" field`.
Reports point at the object type's name. Local fields are authoritative, so a
field contributed only by an extension does not satisfy the definition.

## Scope

**In scope:**

- Rule id `require-type-pattern-with-oneof`, category `Schema`.
- For each local object type definition with `@oneOf`, require fields named
  `error` and `ok`.
- Report each missing field with the byte-identical upstream message and the
  object type name span.
- `requires_schema: true`.

**Out of scope:**

- Input objects and type extensions.

## Dependencies

- spec-049, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_type_pattern_with_oneof.rs`.
- `rules-fixtures/require-type-pattern-with-oneof/`.
- `crates/rglint-rules/tests/rule_require_type_pattern_with_oneof.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(
    id = "require-type-pattern-with-oneof",
    category = "schema",
    requires_schema = true
)]
pub struct RequireTypePatternWithOneof;
```

## Behavior

- Field presence is checked from the local source definition, not only from a
  merged schema.

## Testing

- `rglint_test_suite!("require-type-pattern-with-oneof")`.

## Risks / Notes

- The rule intentionally uses the local definition's fields; a merged schema
  extension cannot satisfy a missing `error` or `ok` field.
