# Spec-021: alphabetize

> Plan reference: §5 Phase 1, §3 (`crates/rglint-rules/src/schema/alphabetize.rs`)

## Goal

Port `alphabetize`: enforce that fields, enum values, type definitions, etc.
appear in alphabetical order within their container. Good exercise for AST
walking + string comparison; the most involved Phase 1 rule.

## Source

`packages/plugin/src/rules/alphabetize/index.ts`

## Scope

**In scope:**

- Rule id `alphabetize`, category `Schema`.
- Options (port the full option shape from the TS rule):
  - `fields: "alphabetical" | "lexicographical" | null` (default `"alphabetical"`).
  - `values: ...` for enum values.
  - `definitions: ...` for top-level type definitions.
  - `groups: ["Query", "Mutation", ...]` prefix ordering before alpha within.
  - `selectors: { ... }` per-container overrides.
- For each container in scope, compare adjacent entries; report each
  out-of-order pair with the graphql-eslint message + a suggestion (`Fix::
  Replace` swapping the two entries' source spans).
- `hasSuggestions: true` (the swap fix).

**Out of scope:**

- Auto-fix application (engine `--fix`, spec-061, consumes the suggestions).

## Dependencies

- spec-008, spec-009, spec-011, spec-012 (node_name), spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/alphabetize.rs`.
- `rules-fixtures/alphabetize/` (likely the largest Phase 1 fixture set).
- `tests/rule_alphabetize.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    fields: Option<SortMode>,
    values: Option<SortMode>,
    definitions: Option<SortMode>,
    #[serde(default)] groups: Vec<String>,
    #[serde(default)] selectors: serde_json::Value,
}
enum SortMode { Alphabetical, Lexicographical }
```

## Behavior

- `Alphabetical` = case-insensitive `str::cmp` with locale-agnostic fold;
  `Lexicographical` = raw byte `str::cmp`.
- `groups`: entries whose name starts with a group prefix are ordered by
  group index first, then alphabetically within the group; non-grouped entries
  sort after grouped ones (verify against TS).
- Suggestion text: `Swap "${a}" and "${b}"`.
- The fix `Replace` swaps the full source spans of the two entries (including
  trailing newline/whitespace up to the next entry — verify exact span
  behavior against TS suggestions in fixtures).

## Testing

- `rglint_test_suite!("alphabetize")` — message + location parity.
- Snapshot of a suggestion's fix applied (the `--fix` dry-run output) for one
  case.
- Property test: for any permutation of N field names, applying the suggested
  fixes yields a sorted list (rule idempotence — PLAN §6.6).

## Risks / Notes

- Suggestion span math is the fiddly part; the fixtures' `output:` field in TS
  (the expected post-fix source) is the oracle — extract it via spec-015 and
  assert the fix produces that exact source.
