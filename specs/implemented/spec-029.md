# Spec-029: unique-enum-value-names

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/unique_enum_value_names.rs`)

## Goal

Port `unique-enum-value-names`: enum values within a single enum must have
unique names. (graphql-eslint's variant: case-insensitive uniqueness to catch
`Active` vs `ACTIVE` collisions — confirm exact semantics from TS.)

## Source

`packages/plugin/src/rules/unique-enum-value-names/index.ts`

## Scope

**In scope:**

- Rule id `unique-enum-value-names`, category `Schema`.
- For each `EnumTypeDefinition`, collect value names; report duplicates per
  graphql-eslint's exact rule (case-sensitive vs insensitive — confirm).
- Message: `Enum value "${name}" is defined multiple times` (verify wording).

**Out of scope:**

- Cross-enum uniqueness (not a graphql-eslint rule).

## Dependencies

- spec-008, spec-009, spec-011, spec-012, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/unique_enum_value_names.rs`.
- `rules-fixtures/unique-enum-value-names/`.
- `tests/rule_unique_enum_value_names.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "unique-enum-value-names", category = "schema")]
pub struct UniqueEnumValueNames;
```

## Behavior

- Reports the 2nd+ occurrence within one enum.
- If case-insensitive, report the colliding pair (graphql-eslint's exact
  reporting choice — verify).

## Testing

- `rglint_test_suite!("unique-enum-value-names")`.

## Risks / Notes

- Confirm case-sensitivity from the TS source; the rule name suggests plain
  uniqueness but graphql-eslint's implementation may differ.
