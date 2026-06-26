# Spec-050: require-nullable-fields-with-oneof

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/require_nullable_fields_with_oneof.rs`)

## Goal

Port `require-nullable-fields-with-oneof`: an `@oneOf` input object's fields
must be nullable (no `!`) — the oneOf semantics is "exactly one is provided",
which non-null would break.

## Source

`packages/plugin/src/rules/require-nullable-fields-with-oneof/index.ts`

## Scope

**In scope:**

- Rule id `require-nullable-fields-with-oneof`, category `Schema`.
- For each `@oneOf` input (spec-049), for each field whose type is
  `NonNull(...)`, report `@oneOf field "${field}" must be nullable` (verify
  exact wording).
- `requires_schema: true`.

**Out of scope:**

- Type-pattern enforcement (spec-051).

## Dependencies

- spec-049, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_nullable_fields_with_oneof.rs`.
- `rules-fixtures/require-nullable-fields-with-oneof/`.
- `tests/rule_require_nullable_fields_with_oneof.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "require-nullable-fields-with-oneof", category = "schema", requires_schema)]
pub struct RequireNullableFieldsWithOneof;
```

## Behavior

- Unwrap the field type; if outermost is `NonNull`, report.
- List-of-non-null (`[String!]`) is fine (only the outer wrapper matters —
  confirm from fixtures).

## Testing

- `rglint_test_suite!("require-nullable-fields-with-oneof")`.

## Risks / Notes

- Confirm whether inner `!` (`[String!]`) is reported or only outer — fixtures
  decide.
