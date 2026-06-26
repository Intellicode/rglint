# Spec-038: require-nullable-result-in-root

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/require_nullable_result_in_root.rs`)

## Goal

Port `require-nullable-result-in-root`: root type (`Query`/`Mutation`/
`Subscription`) fields must return a nullable type (no `!`) — a robust schema
design convention.

## Source

`packages/plugin/src/rules/require-nullable-result-in-root/index.ts`

## Scope

**In scope:**

- Rule id `require-nullable-result-in-root`, category `Schema`.
- Options: `{ root: ["Query" | "Mutation" | "Subscription"] }` (default all).
- For each field on a configured root whose return type is `NonNull(...)`,
  report `Root field "${field}" must return a nullable type` (verify exact
  wording). Suggestion: strip the trailing `!`.
- `hasSuggestions: true`, `requires_schema: true`.

**Out of scope:**

- Non-root nullable rules.

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_nullable_result_in_root.rs`.
- `rules-fixtures/require-nullable-result-in-root/`.
- `tests/rule_require_nullable_result_in_root.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { #[serde(default = "all_roots")] root: Vec<RootKind> }
```

## Behavior

- Suggestion `Fix::Remove` the trailing `!` in the source.
- List-wrapped non-null (`[X!]!`) — only the outer `!` is the violation; the
  suggestion removes the outer `!` (confirm vs TS).

## Testing

- `rglint_test_suite!("require-nullable-result-in-root")`.
- Snapshot of the suggestion applied.

## Risks / Notes

- Confirm whether graphql-eslint checks only the outermost `!` or any `!` in
  the type; fixtures decide.
