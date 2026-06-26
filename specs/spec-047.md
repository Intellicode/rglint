# Spec-047: relay-edge-types

> Plan reference: §5 Phase 5, §3 (`crates/rglint-rules/src/schema/relay_edge_types.rs`)

## Goal

Port `relay-edge-types`: Edge types must have `node: <NodeType>!` and
`cursor: String!`.

## Source

`packages/plugin/src/rules/relay-edge-types/index.ts`

## Scope

**In scope:**

- Rule id `relay-edge-types`, category `Schema`.
- For each Edge type (spec-044), verify `node: NonNull(...)` and
  `cursor: NonNull(String)`.
- Report violations with graphql-eslint's message.
- `requires_schema: true`.

**Out of scope:**

- Connection/PageInfo shape (specs 046, 048).

## Dependencies

- spec-044, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/relay_edge_types.rs`.
- `rules-fixtures/relay-edge-types/`.
- `tests/rule_relay_edge_types.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { /* RelayOpts passthrough */ }
```

## Behavior

- `node` missing or nullable → report.
- `cursor` missing, nullable, or non-`String` → report.
- Reports at the Edge type's span.

## Testing

- `rglint_test_suite!("relay-edge-types")`.

## Risks / Notes

- Confirm whether `node`'s inner type must be non-null (`node: User!`) or may
  be nullable per the spec variant graphql-eslint chose (fixtures decide).
