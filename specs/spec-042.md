# Spec-042: require-selections

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/schema/require_selections.rs`)

## Goal

Port `require-selections`: require certain fields to be selected whenever a
given type is selected (e.g. always select `id` on `Node` types). Cross-doc via
siblings.

## Source

`packages/plugin/src/rules/require-selections/index.ts`

## Scope

**In scope:**

- Rule id `require-selections`, category `Operations`.
- Options: `{ selections: { TypeName: [fieldName, ...] } }` — required fields
  per type.
- For each operation, walk its selection set; whenever a type with required
  selections is selected, verify all required fields are present (in the
  selection set or via a fragment); report missing with message (verify
  wording).
- `requires_schema: true`, `requires_siblings: true` (fragments may satisfy).

**Out of scope:**

- `__typename` requirements (separate concern).

## Dependencies

- spec-004, spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_selections.rs`.
- `rules-fixtures/require-selections/`.
- `tests/rule_require_selections.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    selections: AHashMap<String, Vec<String>>,
    #[serde(default)] allow_fragments: bool,  // confirm option name from TS
}
```

## Behavior

- A required field present via an inline fragment or a named fragment counts
  (if `allowFragments` — confirm default).
- Report per missing field, at the selection set span of the offending type
  usage.
- Fragments: recurse via siblings; if a fragment selects the required field,
  the spread satisfies the requirement.

## Testing

- `rglint_test_suite!("require-selections")`.

## Risks / Notes

- Confirm the exact option name (`selections` vs `requiredSelections`) and
  whether wildcards / `*` are supported.
