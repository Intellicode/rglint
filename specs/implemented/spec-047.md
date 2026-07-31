# Spec-047: relay-edge-types

> Plan reference: §5 Phase 5, §3
> (crates/rglint-rules/src/schema/relay_edge_types.rs)

## Goal

Port `relay-edge-types`, preserving the behavior of graphql-eslint's Relay
Edge validation rule.

## Pinned parity source

Parity was checked against graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24` (2026-07-30):

- [Rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-edge-types/index.ts)
- [Rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-edge-types/index.test.ts)
- [Parity snapshot](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-edge-types/snapshot.md)

## Scope

**In scope:**

- Rule id `relay-edge-types`, category `Schema`, with `requires_schema: true`.
- Edge types are object types returned by a `Connection`-suffixed object's
  `edges` field. A non-object type used there reports an object-type error.
- `node` must be a named type or non-null named type; `cursor` must be
  `String` or a custom/built-in scalar, optionally non-null. Lists are rejected.
- Options, all defaulting to `true`, are `withEdgeSuffix`,
  `shouldImplementNode`, and `listTypeCanWrapOnlyEdgeType`.
- When enabled, edge names must end in `Edge`, object `node` targets must
  implement `Node`, and top-level list fields must wrap an edge type.
- Diagnostics use graphql-eslint's exact messages and node locations.

**Out of scope:**

- Connection shape and PageInfo shape (specs 046 and 048).
- A stricter `node: Node!` requirement: the upstream rule accepts any named
  scalar, enum, object, interface, or union type and only conditionally checks
  object targets for `Node`.

## Dependencies

- spec-044 (Relay predicates — hard prerequisite).
- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Exact diagnostics

- `Edge type must be an Object type.`
- `Edge type must have "Edge" suffix.`
- `A list type should only wrap an edge type.`
- `Edge type must contain a field \`node\` that return either a Scalar, Enum, Object, Interface, Union, or a non-null wrapper around one of those types.`
- `Field \`node\` must return either a Scalar, Enum, Object, Interface, Union, or a non-null wrapper around one of those types.`
- `Edge type must contain a field \`cursor\` that return either a String, Scalar, or a non-null wrapper wrapper around one of those types.`
- `Field \`cursor\` must return either a String, Scalar, or a non-null wrapper wrapper around one of those types.`
- `Edge type's field \`node\` must implement \`Node\` interface.`

## Deliverables

- `crates/rglint-rules/src/schema/relay_edge_types.rs`.
- `rules-fixtures/relay-edge-types/` with canonical filenames and a pinned
  manifest covering all five valid and six invalid upstream cases.
- `crates/rglint-rules/tests/rule_relay_edge_types.rs`.
- This spec moved to `specs/implemented/` and the status index updated.

## Testing

- `cargo test -p rglint-rules --test rule_relay_edge_types`.
- Unit tests assert metadata, option defaults, and node type semantics.
