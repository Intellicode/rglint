# Spec-018: unique-operation-name

> Plan reference: §5 Phase 1, §3 (`crates/rglint-rules/src/operations/unique_operation_name.rs`)

## Goal

Port `unique-operation-name`: across all sibling documents in a project, no two
**named** operations share the same name. Mirrors `unique-fragment-name` but
for operations; anonymous operations are ignored (spec-016 covers those).

## Source

`packages/plugin/src/rules/unique-operation-name/index.ts`

## Scope

**In scope:**

- Rule id `unique-operation-name`, category `Operations`, `requires_siblings: true`.
- `Handler::finalize`: collect named operations across siblings; report each
  duplicate occurrence after the first with message `Operation "${name}" is defined multiple times`.

**Out of scope:**

- Fragment uniqueness (spec-017).
- Anonymous operations (spec-016).

## Dependencies

- spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/unique_operation_name.rs`.
- `rules-fixtures/unique-operation-name/`.
- `tests/rule_unique_operation_name.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "unique-operation-name", category = "operations", requires_siblings)]
pub struct UniqueOperationName;
```

## Behavior

- Same "report duplicates after the first" semantics as spec-017.
- Anonymous operations (name `None`) are skipped.

## Testing

- `rglint_test_suite!("unique-operation-name")`.
- Unit: 2 files with `query Foo` + 1 anonymous → exactly 1 diagnostic.

## Risks / Notes

- Share the dedup helper with spec-017 if it falls out naturally; otherwise
  duplicate the ~10 LOC (preferred over premature abstraction).
