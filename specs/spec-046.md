# Spec-046: relay-connection-types

> Plan reference: §5 Phase 5, §3 (`crates/rglint-rules/src/schema/relay_connection_types.rs`)

## Goal

Port `relay-connection-types`: Connection types must have the correct shape —
`edges: [XEdge!]!` and `pageInfo: PageInfo!` fields (and optional `totalCount`).

## Source

`packages/plugin/src/rules/relay-connection-types/index.ts`

## Scope

**In scope:**

- Rule id `relay-connection-types`, category `Schema`.
- For each type matching the Connection pattern (spec-044), verify:
  - `edges` field exists, is `NonNull(List(NonNull(EdgeType)))` (`[XEdge!]!`).
  - The edge type name matches `${connectionNameWithout "Connection"}Edge` (or
    the configured pattern).
  - `pageInfo` field exists, is `NonNull(PageInfo)`.
- Report each violation with graphql-eslint's message.
- `requires_schema: true`.

**Out of scope:**

- Edge/PageInfo internal shape (specs 047, 048).

## Dependencies

- spec-044, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/relay_connection_types.rs`.
- `rules-fixtures/relay-connection-types/`.
- `tests/rule_relay_connection_types.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { /* RelayOpts passthrough */ }
```

## Behavior

- `edges` missing → report; wrong wrapper (`[XEdge]` not `[XEdge!]!`) →
  report with the specific mismatch.
- Edge type name derivation: `UserConnection` → `UserEdge` (strip `Connection`,
  add `Edge`); configurable via pattern.

## Testing

- `rglint_test_suite!("relay-connection-types")`.

## Risks / Notes

- Verify exact wrapper strictness from fixtures (graphql-eslint may accept
  `[XEdge!]` vs require `[XEdge!]!`).
