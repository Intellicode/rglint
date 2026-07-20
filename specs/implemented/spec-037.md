# Spec-037: no-scalar-result-type-on-mutation

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/no_scalar_result_type_on_mutation.rs`)

## Goal

Port `no-scalar-result-type-on-mutation`: mutation fields must not return a
bare scalar — they should return an object (mutation payload) to allow
extending responses later.

## Source

`packages/plugin/src/rules/no-scalar-result-type-on-mutation/index.ts`

## Scope

**In scope:**

- Rule id `no-scalar-result-type-on-mutation`, category `Schema`.
- Options: `{ allowed: [string] }` — scalar names to allow (default empty).
- For each field on the `Mutation` root type whose return type (unwrap
  non-null + list) is a scalar, report
  `Mutation "${field}" returns a scalar; return an object type instead`
  (verify exact wording).
- `requires_schema: true`.

**Out of scope:**

- Subscription/query scalar returns (not restricted by this rule).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_scalar_result_type_on_mutation.rs`.
- `rules-fixtures/no-scalar-result-type-on-mutation/`.
- `tests/rule_no_scalar_result_type_on_mutation.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { #[serde(default)] allowed: Vec<String> }
```

## Behavior

- Unwrap `NonNull`/`List` to find the base type.
- Built-in scalars (`Int`, `String`, `Boolean`, `ID`, `Float`) + custom
  scalars all trigger unless in `allowed`.
- Reports at the field definition span.

## Testing

- `rglint_test_suite!("no-scalar-result-type-on-mutation")`.

## Risks / Notes

- Resolve `Mutation` root via the schema's root operation type map (a schema
  may rename it — see spec-032 note).
