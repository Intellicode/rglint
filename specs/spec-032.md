# Spec-032: no-root-type

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/no_root_type.rs`)

## Goal

Port `no-root-type`: forbid the declaration of one or more root types
(`Query`, `Mutation`, `Subscription`) — useful for subgraphs/schemas that
shouldn't define their own roots.

## Source

`packages/plugin/src/rules/no-root-type/index.ts`

## Scope

**In scope:**

- Rule id `no-root-type`, category `Schema`.
- Options: `{ forbidden: ["Query" | "Mutation" | "Subscription"] }` (default
  all three; or a single string accepted).
- For each forbidden root type present in the schema, report
  `Root type "${name}" is forbidden` (verify exact wording).

**Out of scope:**

- Root type *naming* (that's `naming-convention`).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-012, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_root_type.rs`.
- `rules-fixtures/no-root-type/`.
- `tests/rule_no_root_type.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts {
    #[serde(default = "all_roots")] forbidden: Vec<RootKind>,
}
enum RootKind { Query, Mutation, Subscription }
```

## Behavior

- Matches root types either by name (`Query` etc.) or by schema's
  `query_root`/`mutation_root`/`subscription_root` — use the schema's declared
  roots when available (a renamed root `schema { query: MyQuery }` counts).
- Reports at the type definition span.

## Testing

- `rglint_test-suite!("no-root-type")`.
- Unit: schema with `schema { query: MyQuery } type MyQuery { ... }` +
  default opts → 1 diagnostic on `MyQuery`.

## Risks / Notes

- Use `apollo_compiler::Schema::root_operation_type_name` (or equivalent) to
  resolve the actual root types rather than hardcoding `Query`/`Mutation`/
  `Subscription` names.
