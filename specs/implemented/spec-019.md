# Spec-019: no-duplicate-fields

> Plan reference: §5 Phase 1, §3 (`crates/rglint-rules/src/schema/no_duplicate_fields.rs`)

## Goal

Port `no-duplicate-fields`: within a single object/interface type definition
(or a selection set), no two fields/share the same name. Schema-side and
operation-side both apply per graphql-eslint.

## Source

`packages/plugin/src/rules/no-duplicate-fields/index.ts`

## Scope

**In scope:**

- Rule id `no-duplicate-fields`, category `Schema` (graphql-eslint categorizes
  it under schema but it also fires on selection sets — keep `category = Schema`
  and let the handler subscribe to both `ObjectTypeDefinition`/
  `InterfaceTypeDefinition` and `SelectionSet`).
- For each scanned container, collect field names; report each duplicate
  occurrence after the first with message `Field "${name}" is defined multiple times` (schema) /
  `Field "${name}" is selected multiple times` (operation).
- `requires_schema: false` (works on documents standalone).

**Out of scope:**

- Cross-type duplicate detection (that's a spec concern, handled by
  apollo-compiler, spec-053).

## Dependencies

- spec-008, spec-009, spec-011, spec-012 (node_name), spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_duplicate_fields.rs`.
- `rules-fixtures/no-duplicate-fields/`.
- `tests/rule_no_duplicate_fields.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "no-duplicate-fields", category = "schema")]
pub struct NoDuplicateFields;
```

## Behavior

- Reports the 2nd+ occurrence within the same container only (not across
  types).
- Handles input object types too (`InputObjectTypeDefinition`).
- Operation selection sets: dedup within one selection set, not across
  fragments.

## Testing

- `rglint_test_suite!("no-duplicate-fields")`.
- Unit: `type X { a: Int, a: Int, b: Int, b: Int }` → 2 diagnostics (one per
  second `a` and `b`).

## Risks / Notes

- Verify whether graphql-eslint reports both schema and operation duplicates
  from the same rule id; if it splits them, follow the same split (single id,
  two handler paths is fine).
