# Spec-044: shared/relay.rs predicates

> Plan reference: §5 Phase 5 ("Port shared/relay.rs first"), §3 (`crates/rglint-rules/src/shared/relay.rs`)

## Goal

Port the shared Relay predicate helpers used by all four Relay rules
(specs 045-048): `is_connection_type`, `is_edge_type`, `is_page_info_type`,
`connection_field_edge`, plus argument-shape validators. Must land **before**
the Relay rules so they drop in.

## Source

`packages/plugin/src/rules/relay-*/index.ts` shared helpers (consolidate the
common predicates into one `shared/relay.rs`).

## Scope

**In scope:**

- `is_connection_type(t) -> bool` — a type whose name ends with `Connection`
  and has fields `edges` (list of an Edge type) and `pageInfo` (PageInfo type).
- `is_edge_type(t) -> bool` — name ends with `Edge`, has `node` and `cursor`.
- `is_page_info_type(t) -> bool` — name is `PageInfo` (or matches option),
  has `hasNextPage`, `hasPreviousPage`, `startCursor`, `endCursor`.
- `connection_for_field(field) -> Option<&Type>` — given a field, return its
  connection type if its return type is a Connection.
- `edge_for_connection(conn) -> Option<&Type>`.
- Option carriers: `{ connectionNamePattern, edgeNamePattern, pageInfoName }`
  with Relay defaults.
- `is_forward_only` / `is_backward_only` argument classifiers (used by
  `relay-arguments`).

**Out of scope:**

- The four Relay rules themselves (specs 045-048).

## Dependencies

- spec-004 (Schema).
- spec-011 (not strictly, but rules using this do).

## Deliverables

- `crates/rglint-rules/src/shared/relay.rs`.
- Re-export from `shared/mod.rs`.
- Unit tests over a hand-built Relay-compliant schema fixture.

## Interface / API

```rust
pub struct RelayOpts {
    pub connection_pattern: Regex,   // default /Connection$/
    pub edge_pattern: Regex,          // default /Edge$/
    pub page_info_name: String,       // default "PageInfo"
}
impl Default for RelayOpts { ... }

pub fn is_connection_type(t: &ast::ObjectTypeDefinition, opts: &RelayOpts) -> bool;
pub fn is_edge_type(t: &ast::ObjectTypeDefinition, opts: &RelayOpts) -> bool;
pub fn is_page_info_type(t: &ast::ObjectTypeDefinition, opts: &RelayOpts) -> bool;
pub fn edge_of_connection<'s>(conn: &'s ast::ObjectTypeDefinition, schema: &Schema) -> Option<&'s ast::ObjectTypeDefinition>;
```

## Behavior

- Predicates match by **name pattern + required fields** (both conditions).
- All four PageInfo fields are required (not just `hasNextPage` — confirm
  graphql-eslint's exactness from the `relay-page-info` fixtures).
- Helpers borrow from `apollo_compiler::Schema` (lifetime managed by caller).

## Testing

- Unit: a fixture `relay/schema.graphqls` with a compliant `UserConnection` +
  non-compliant `BadConnection` (missing `pageInfo`); assert predicates return
  true/false correctly.

## Risks / Notes

- Consolidate predicates here even if the TS source duplicates them across the
  four rule files — DRY at the Rust layer.
