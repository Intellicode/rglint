# Spec-035: no-unused-fields

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/no_unused_fields.rs`)

## Goal

Port `no-unused-fields`: schema fields that are never selected by any
operation across the project's sibling documents are reported as unused.
Schema + operations cross-check.

## Source

`packages/plugin/src/rules/no-unused-fields/index.ts`

## Scope

**In scope:**

- Rule id `no-unused-fields`, category `Schema`.
- Options: `{ ignoreType: [string], ignoreField: [string] }` skip lists.
- Build the set of all fields selected across all sibling operations
  (recursing through fragments); for each schema field not in that set,
  report `Field "${type}.${field}" is unused` (verify exact wording).
- `requires_schema: true`, `requires_siblings: true`.

**Out of scope:**

- `no-unreachable-types` (spec-036 — graph reachability, different algorithm).

## Dependencies

- spec-004, spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_unused_fields.rs`.
- `rules-fixtures/no-unused-fields/`.
- `tests/rule_no_unused_fields.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    #[serde(default)] ignore_type: Vec<String>,
    #[serde(default)] ignore_field: Vec<String>,
}
```

## Behavior

- "Selected" = appears in any operation's selection set (directly or via
  fragment). Interface fields count as selected if any implementing type's
  field is selected (match graphql-eslint).
- Skips root types' own fields unless selected (a `Query.foo` field that no
  op selects is unused).
- `ignoreField` entries formatted `TypeName.fieldName`.

## Testing

- `rglint_test_suite!("no-unused-fields")`.

## Risks / Notes

- With zero sibling documents, every field is "unused" — graphql-eslint
  likely no-ops when siblings are empty. Confirm and self-skip in that case to
  avoid noise.
