# Spec-044: shared/relay.rs predicates

> Plan reference: §5 Phase 5 ("Port shared/relay.rs first"), §3 (`crates/rglint-rules/src/shared/relay.rs`)

## Goal

Port the shared Relay predicate helpers used by all four Relay rules
(specs 045-048): `is_connection_type`, `is_edge_type`, `is_page_info_type`,
`connection_field_edge`, plus argument-shape validators. Must land **before**
the Relay rules so they drop in.

## Source

Parity was checked against graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24` (2026-07-30):

- [relay-connection-types/index.ts](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-connection-types/index.ts)
- [relay-edge-types/index.ts](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-edge-types/index.ts)
- [relay-page-info/index.ts](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-page-info/index.ts)
- [relay-arguments/index.ts](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-arguments/index.ts)
- [Relay tests](https://github.com/graphql-hive/graphql-eslint/tree/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules)

The upstream rules duplicate these checks across AST visitors. rglint
centralizes them, adapting the object-type inputs to Apollo Compiler's merged
`schema::ObjectType` so extensions are visible to later schema rules.

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

pub fn is_connection_type(t: &schema::ObjectType, opts: &RelayOpts) -> bool;
pub fn is_edge_type(t: &schema::ObjectType, opts: &RelayOpts) -> bool;
pub fn is_page_info_type(t: &schema::ObjectType, opts: &RelayOpts) -> bool;
pub fn connection_for_field<'s>(
    field: &ast::FieldDefinition,
    schema: &'s Schema,
    opts: &RelayOpts,
) -> Option<&'s schema::ObjectType>;
pub fn edge_of_connection<'s>(
    conn: &schema::ObjectType,
    schema: &'s Schema,
) -> Option<&'s schema::ObjectType>;
pub fn is_forward_only(field: &ast::FieldDefinition) -> bool;
pub fn is_backward_only(field: &ast::FieldDefinition) -> bool;
```

## Behavior

- Predicates match by **name pattern + required fields** (both conditions).
- All four PageInfo fields are required (not just `hasNextPage` — confirm
  graphql-eslint's exactness from the `relay-page-info` fixtures).
- Helpers borrow from `apollo_compiler::Schema` (lifetime managed by caller).
- Predicates intentionally check names plus required field presence; exact
  wrappers and scalar/object return types remain the responsibility of the
  individual Relay rules, matching the upstream rules' separation of checks.

## Testing

- Unit: `crates/rglint-rules/src/shared/fixtures/relay/schema.graphqls` with a
  compliant `UserConnection` plus non-compliant Connection, Edge, and PageInfo
  objects; assert predicates, schema resolution, pagination classifiers, and a
  custom naming option set.

## Risks / Notes

- Consolidate predicates here even if the TS source duplicates them across the
  four rule files — DRY at the Rust layer.
