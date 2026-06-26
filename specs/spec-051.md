# Spec-051: require-type-pattern-with-oneof

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/require_type_pattern_with_oneof.rs`)

## Goal

Port `require-type-pattern-with-oneof`: `@oneOf` input fields must each have a
distinct type (no two fields share the same type) so the oneOf choice is
unambiguous from the provided value's type alone.

## Source

`packages/plugin/src/rules/require-type-pattern-with-oneof/index.ts`

## Scope

**In scope:**

- Rule id `require-type-pattern-with-oneof`, category `Schema`.
- For each `@oneOf` input (spec-049), group fields by their (unwrapped) type
  name; report any group with >1 field with message `@oneOf fields "${a}" and "${b}" share type "${type}"` (verify exact wording).
- `requires_schema: true`.

**Out of scope:**

- Nullability (spec-050).

## Dependencies

- spec-049, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_type_pattern_with_oneof.rs`.
- `rules-fixtures/require-type-pattern-with-oneof/`.
- `tests/rule_require_type_pattern_with_oneof.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "require-type-pattern-with-oneof", category = "schema", requires_schema)]
pub struct RequireTypePatternWithOneof;
```

## Behavior

- Type comparison by the base type name (unwrap non-null + list).
- Reports each colliding pair (or one diagnostic listing all colluders —
  confirm from fixtures).

## Testing

- `rglint_test_suite!("require-type-pattern-with-oneof")`.

## Risks / Notes

- Confirm the report granularity (per-pair vs per-type-group).
