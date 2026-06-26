# Spec-039: require-field-of-type-query-in-mutation-result

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/require_field_of_type_query_in_mutation_result.rs`)

## Goal

Port `require-field-of-type-query-in-mutation-result`: mutation payload types
should expose a `query: Query` field so clients can re-query after the
mutation in the same round-trip.

## Source

`packages/plugin/src/rules/require-field-of-type-query-in-mutation-result/index.ts`

## Scope

**In scope:**

- Rule id `require-field-of-type-query-in-mutation-result`, category `Schema`.
- Options: `{ queryTypeName: "Query" }` (renamed query root support).
- Identify payload types: types returned by `Mutation` root fields (unwrap
  non-null/list).
- For each payload type without a field of type `Query` (the configured query
  root name), report `Mutation result type "${type}" must expose a "query" field of type "${QueryType}"` (verify exact wording).
- `requires_schema: true`.

**Out of scope:**

- Mutation input types (this is result-only).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_field_of_type_query_in_mutation_result.rs`.
- `rules-fixtures/require-field-of-type-query-in-mutation-result/`.
- `tests/rule_require_field_of_type_query_in_mutation_result.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { #[serde(default = "default_query")] query_type_name: String }
```

## Behavior

- A payload type that already has a `query: Query` field passes.
- Field name must be `query` (graphql-eslint's fixed convention — confirm).
- Reports at the payload type's definition span.

## Testing

- `rglint_test_suite!("require-field-of-type-query-in-mutation-result")`.

## Risks / Notes

- Resolve the actual query root type name from the schema rather than assuming
  `Query` (default `queryTypeName` matches the schema's `query` root).
