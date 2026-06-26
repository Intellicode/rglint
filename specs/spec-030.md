# Spec-030: strict-id-in-types

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/strict_id_in_types.rs`)

## Goal

Port `strict-id-in-types`: enforce that types representing entities expose an
`id` field of type `ID!` (or a configured type), with options for which types
are scanned and which field name/type are required.

## Source

`packages/plugin/src/rules/strict-id-in-types/index.ts`

## Scope

**In scope:**

- Rule id `strict-id-in-types`, category `Schema`.
- Options (port from TS):
  - `acceptedIdNames: [String]` (default `["id"]`).
  - `acceptedTypes: [String]` (default `["ID"]`).
  - `list: "all" | "some" | "none"` (whether all/some/none of the accepted
    names must be present).
  - `strictInterfaces: bool`.
- For each object/interface type (excluding `Query`/`Mutation`/`Subscription`
  per options), check its `id`-like field; report mismatches with the exact
  TS message.
- `requires_schema: true`.

**Out of scope:**

- Relay `id` requirements (spec-044+ — different rule family).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/strict_id_in_types.rs`.
- `rules-fixtures/strict-id-in-types/`.
- `tests/rule_strict_id_in_types.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts {
    #[serde(default = "default_id_names")] accepted_id_names: Vec<String>,
    #[serde(default = "default_id_types")] accepted_types: Vec<String>,
    #[serde(default)] list: ListMode,
    #[serde(default)] strict_interfaces: bool,
}
```

## Behavior

- Skips operation root types unless configured otherwise.
- `list: "all"` → every accepted name must be present with an accepted type.
- Reports the offending field (or the type if the field is absent).

## Testing

- `rglint_test_suite!("strict-id-in-types")`.

## Risks / Notes

- Verify the default `acceptedTypes` is `["ID"]` (string, not the AST `ID`
  scalar node) and matches type names as written in the schema.
