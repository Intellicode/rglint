# Spec-017: unique-fragment-name

> Plan reference: §5 Phase 1, §3 (`crates/rglint-rules/src/operations/unique_fragment_name.rs`)

## Goal

Port `unique-fragment-name`: across all sibling documents in a project, no two
fragment definitions share the same name. First rule to use `requires_siblings`.

## Source

`packages/plugin/src/rules/unique-fragment-name/index.ts`

## Scope

**In scope:**

- Rule id `unique-fragment-name`, category `Operations`.
- `requires_siblings: true`.
- `Handler::finalize`: collect all fragment names across siblings; for each
  name occurring >1 times, report each duplicate occurrence after the first
  with message `Fragment "${name}" is defined multiple times`.
- Suggestion: rename to a unique name (optional; v1 = no suggestion, set
  `hasSuggestions: false`).

**Out of scope:**

- `unique-operation-name` (spec-018).

## Dependencies

- spec-006 (Siblings).
- spec-008, spec-009, spec-011.
- spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/unique_fragment_name.rs`.
- `rules-fixtures/unique-fragment-name/` (verify + fix).
- `tests/rule_unique_fragment_name.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "unique-fragment-name", category = "operations", requires_siblings)]
pub struct UniqueFragmentName;
```

## Behavior

- Reports the 2nd, 3rd, … occurrence (not the 1st) so the first definition is
  treated as canonical (matches graphql-eslint).
- Diagnostics are emitted on the file containing each duplicate, at the
  fragment definition's span.
- When `siblings` is unavailable (schema-only lint), the rule self-skips.

## Testing

- `rglint_test_suite!("unique-fragment-name")`.
- Extra unit test: 3 files each defining `fragment X` → exactly 2 diagnostics,
  both naming `X`, on files 2 and 3.

## Risks / Notes

- Ensure the engine (spec-011) passes `siblings` to this rule's context; the
  `requires_siblings` flag is what triggers that.
