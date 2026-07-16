# Spec-025: require-description

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/require_description.rs`)

## Goal

Port `require-description`: require descriptions on categories of definitions
(types, fields, enum values, directives, arguments, operations, etc.) per the
option map. Schema-only (plus operation descriptions).

## Source

`packages/plugin/src/rules/require-description/index.ts`

## Scope

**In scope:**

- Rule id `require-description`, category `Schema`.
- Options: a map of `nodeKind -> bool` (e.g.
  `{ types: true, FieldDefinition: true, EnumValueDefinition: false, ... }`)
  plus convenience flags. Port the exact option schema from TS
  (`meta.docs.configOptions`).
- For each in-scope node lacking a `description`, report
  `Description is required for ${nodeKind}` (verify exact wording).
- `requires_schema: true` when checking schema-side nodes (always for this
  rule).

**Out of scope:**

- Description *style* (spec-023).
- Hashtag comments (spec-024).

## Dependencies

- spec-004 (schema).
- spec-008, spec-009, spec-011, spec-012 (node_name), spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_description.rs`.
- `rules-fixtures/require-description/`.
- `tests/rule_require_description.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    #[serde(default)] types: bool,
    #[serde(default)] field_definition: bool,    // FieldDefinition
    #[serde(default)] enum_value_definition: bool,
    // ... full set from TS
    // plus an escape hatch: `ObjectExpression: bool` etc.
}
```

## Behavior

- Each option key gates one node kind; `true` = required, `false`/absent = skip.
- A node "has a description" iff its `description` child is present (block or
  single-quote — `no-hashtag-description` is a separate concern).
- Reports at the node's name span (graphql-eslint's location choice — verify).

## Testing

- `rglint_test_suite!("require-description")`.
- Unit: schema with `type X { a: Int }` + `{ types: true, FieldDefinition: true }`
  → 2 diagnostics (type + field).

## Risks / Notes

- The option map is large; model it as a flat struct with `#[serde(default)]`
  per field and a `serde_json::Value` fallback for unknown keys (warn, don't
  error) to stay forward-compatible.
