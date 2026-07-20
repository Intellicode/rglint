# Spec-036: no-unreachable-types

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/no_unreachable_types.rs`)

## Goal

Port `no-unreachable-types`: schema types not reachable from any root type
(`Query`/`Mutation`/`Subscription`) via field type transitively (and union
members, interface implementations) are reported as unreachable. Graph
traversal over the schema.

## Source

`packages/plugin/src/rules/no-unreachable-types/index.ts`

## Scope

**In scope:**

- Rule id `no-unreachable-types`, category `Schema`.
- Options: `{ ignoreType: [string] }`.
- Build the schema type graph: edges from a type to each type referenced by
  its fields (named + list + non-null wrapper stripped), union members,
  interface implementations.
- BFS/DFS from each root type; report each type never visited with
  `Type "${name}" is unreachable` (verify exact wording).
- `requires_schema: true`.

**Out of scope:**

- Unused fields (spec-035).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_unreachable_types.rs`.
- `rules-fixtures/no-unreachable-types/`.
- `tests/rule_no_unreachable_types.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { #[serde(default)] ignore_type: Vec<String> }
```

## Behavior

- Scalar/built-in types are never reported (they're terminal).
- `@deprecated` types still count as reachable (deprecation ≠ unreachable).
- `ignoreType` suppresses specific types.
- Directives are not considered types (no report on directive definitions).

## Testing

- `rglint_test_suite!("no-unreachable-types")`.
- Unit: a schema with `type Orphan { x: Int }` (no root references) → 1
  diagnostic.

## Risks / Notes

- Watch the `__Type` introspection types — apollo-compiler may expose them as
  synthetic types; filter them out before reporting.
