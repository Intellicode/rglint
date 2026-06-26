# Spec-040: selection-set-depth

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/operations/selection_set_depth.rs`), §8 (graphql-depth-limit risk)

## Goal

Port `selection-set-depth`: limit the maximum depth of an operation's
selection set, recursing through fragment spreads. Reimplements
`graphql-depth-limit` (~80 LOC) in Rust, built on `Siblings::get_fragments_in_use`.

## Source

`packages/plugin/src/rules/selection-set-depth/index.ts` (wraps
`graphql-depth-limit`).

## Scope

**In scope:**

- Rule id `selection-set-depth`, category `Operations`.
- Options: `{ maxDepth: usize, ignore: [string] (field paths), depths: { Type.field: n } }` (port exact shape).
- For each operation, compute max selection depth via recursive walk:
  - scalar/enum field = depth 1.
  - object/interface field = 1 + max(child depths).
  - fragment spread = inline the fragment's selection set (via siblings),
    guarding cycles.
- Report when depth > `maxDepth` with graphql-eslint's message (verify wording).
- `requires_siblings: true` (fragments), `requires_schema: true` (to know
  which fields are scalar vs object).

**Out of scope:**

- Inline fragments depth (handle if graphql-eslint does — confirm).

## Dependencies

- spec-004, spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/selection_set_depth.rs`.
- `rules-fixtures/selection-set-depth/`.
- `tests/rule_selection_set_depth.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts {
    max_depth: usize,
    #[serde(default)] ignore: Vec<String>,
    #[serde(default)] depths: AHashMap<String, usize>,
}
```

## Behavior

- `ignore` field paths (`User.friends`) skip that field's children (count it
  as depth 1 but don't recurse).
- `depths` overrides the depth of a specific `Type.field` (for whitelisting
  deep-but-cheap fields).
- Cycle guard: a `HashSet<String>` of in-progress fragment names; revisiting
  returns depth 0 for that branch.
- Report at the operation definition span (graphql-eslint's choice — verify).

## Testing

- `rglint_test_suite!("selection-set-depth")`.
- Unit: cyclic fragment doesn't infinite-loop; a 7-deep op with `maxDepth: 6`
  → 1 diagnostic.

## Risks / Notes

- §8 risk: "verify it handles fragments (cyclic + shared)." The cycle guard
  is the crux; cover it with a dedicated unit test.
