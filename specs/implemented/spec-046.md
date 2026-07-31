# Spec-046: relay-connection-types

> Plan reference: §5 Phase 5, §3
> (crates/rglint-rules/src/schema/relay_connection_types.rs)

## Goal

Port relay-connection-types, preserving the behavior of graphql-eslint's
Relay connection-shape rule.

## Pinned parity source

Parity was checked against graphql-eslint commit
f0f200ef0b030cb8a905bbcb32fe346b87cc2e24 (2026-07-30):

- [Rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-connection-types/index.ts)
- [Rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-connection-types/index.test.ts)

## Scope

**In scope:**

- Rule id relay-connection-types, category Schema.
- requires_schema: true in rglint so the rule runs against schema sources.
- A scalar, union, input object, enum, or interface whose name ends in
  Connection reports that the Connection type must be an Object type.
- An object definition or extension with both edges and pageInfo but no
  Connection suffix reports the missing suffix.
- An object definition or extension whose name ends in Connection reports
  missing edges and/or pageInfo fields.
- edges accepts either a list type or a non-null list type.
- pageInfo must be exactly a non-null named PageInfo type.
- Object-definition and field-type diagnostics point to the same name/type
  nodes selected by the upstream visitors.

**Deliberate parity corrections to the original spec text:**

- The upstream rule does not validate the edge type name, so rglint does not
  require EDGE_NAME_PATTERN.
- The upstream rule does not require the stricter [XEdge!]! wrapper; all list
  wrappers accepted by the upstream tests remain valid.
- The upstream rule has no configurable options and hard-codes the Connection
  suffix and PageInfo name. RelayOpts is therefore not used by this rule.
- The upstream rule's metadata does not require a compiled schema, but rglint
  uses schema availability to lint schema source files and to distinguish
  non-null named types from list wrappers.

**Out of scope:**

- Edge/PageInfo internal shape (specs 047 and 048).

## Deliverables

- crates/rglint-rules/src/schema/relay_connection_types.rs.
- rules-fixtures/relay-connection-types/ with canonical
  NN.graphql/NN.config.toml/NN.expected.json names and pinned manifest.
- crates/rglint-rules/tests/rule_relay_connection_types.rs.
- This spec moved to specs/implemented/.
- specs/README.md updated to mark the spec complete.

## Exact diagnostics

- Connection type must be an Object type.
- Connection type must have `Connection` suffix.
- Connection type must contain a field `edges` that return a list type.
- Connection type must contain a field `pageInfo` that return a non-null `PageInfo` Object type.
- `edges` field must return a list type.
- `pageInfo` field must return a non-null `PageInfo` Object type.

## Testing

- cargo test -p rglint-rules --test rule_relay_connection_types.
- rglint_test_suite!("relay-connection-types") covers every upstream valid
  and invalid case.
- Unit tests assert rule metadata and exact PageInfo wrapper semantics.
