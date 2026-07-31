# Spec-048: relay-page-info

> Plan reference: §5 Phase 5, §3 (`crates/rglint-rules/src/schema/relay_page_info.rs`)

## Goal

Port `relay-page-info`: the `PageInfo` type must define the four standard
fields with correct types: `hasNextPage: Boolean!`, `hasPreviousPage:
Boolean!`, `startCursor: String`, `endCursor: String` (cursors nullable per
spec).

## Pinned parity source

Parity was checked against graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24` (2026-07-30):

- [Rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-page-info/index.ts)
- [Rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-page-info/index.test.ts)
- [Parity snapshot](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-page-info/snapshot.md)

## Scope

**In scope:**

- Rule id `relay-page-info`, category `Schema`.
- For the `PageInfo` type (spec-044), verify each of the four fields exists
  with the exact type (nullable/non-null per graphql-eslint's spec).
- Report missing/mistyped fields.
- `requires_schema: true`.
- The Rust port accepts the shared Relay `pageInfoName` option; the pinned
  upstream rule currently hardcodes `PageInfo`, so the default behavior and
  diagnostics remain byte-identical while custom naming is a deliberate Rust
  extension.

**Out of scope:**

- Connection/Edge shape (specs 046, 047).

## Dependencies

- spec-044, spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/relay_page_info.rs`.
- `rules-fixtures/relay-page-info/`.
- `tests/rule_relay_page_info.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { /* RelayOpts passthrough (pageInfoName) */ }
```

## Behavior

- `hasNextPage` / `hasPreviousPage` must be `Boolean!` (non-null).
- `startCursor` / `endCursor` must be `String` (nullable per Relay spec —
  confirm graphql-eslint enforces nullable, not non-null).
- Reports at the PageInfo type's span (or the field's — confirm).

Exact default diagnostics:

- `The server must provide a \`PageInfo\` object.`
- `\`PageInfo\` must be an Object type.`
- `\`PageInfo\` must contain a field \`FIELD\`, that return TYPE.`
- `Field \`FIELD\` must return TYPE.`

The base `type PageInfo` definition is checked using fields declared in that
definition. Schema extensions remain visible for type resolution but do not
silently make a base definition satisfy fields that the upstream AST visitor
reports as missing.

## Testing

- `rglint_test_suite!("relay-page-info")`.

## Risks / Notes

- The nullable-vs-non-null choice for cursors is the most-likely divergence
  point; the fixture is authoritative.
