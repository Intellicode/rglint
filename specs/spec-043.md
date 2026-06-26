# Spec-043: no-one-place-fragments

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/schema/no_one_place_fragments.rs`)

## Goal

Port `no-one-place-fragments`: a fragment definition that is spread in exactly
one location should be inlined (it adds indirection without reuse). Uses
siblings to count spread usage across all operations.

## Source

`packages/plugin/src/rules/no-one-place-fragments/index.ts`

## Scope

**In scope:**

- Rule id `no-one-place-fragments`, category `Operations`.
- Options: `{ max: usize }` (default 1 — report fragments used ≤ `max` times;
  confirm semantics).
- For each fragment definition, count its spread occurrences across all
  sibling operations; if count ≤ `max`, report
  `Fragment "${name}" is only used in ${count} place; inline it` (verify exact
  wording).
- `requires_siblings: true`.

**Out of scope:**

- Auto-inlining (suggestion only; `--fix` is spec-061).

## Dependencies

- spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_one_place_fragments.rs`.
- `rules-fixtures/no-one-place-fragments/`.
- `tests/rule_no_one_place_fragments.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts { #[serde(default = "one")] max: usize }
```

## Behavior

- Count spreads by name across all operations (a single op using it twice =
  count 2, not 1 — confirm against TS).
- Unused fragments (count 0) — does graphql-eslint report them here or via a
  separate rule? Confirm; if separate, skip count-0 here.
- Report at the fragment definition span.

## Testing

- `rglint_test_suite!("no-one-place-fragments")`.
- Unit: a fragment used in 2 ops with `max: 1` → 0 diagnostics; used in 1 op
  → 1 diagnostic.

## Risks / Notes

- Verify whether count is per-operation or per-spread-site; the fixtures
  disambiguate.
