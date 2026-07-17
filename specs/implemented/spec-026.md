# Spec-026: require-deprecation-reason

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/require_deprecation_reason.rs`)

## Goal

Port `require-deprecation-reason`: every `@deprecated` directive must carry a
non-empty `reason` argument.

## Source

`packages/plugin/src/rules/require-deprecation-reason/index.ts`

## Scope

**In scope:**

- Rule id `require-deprecation-reason`, category `Schema`.
- Subscribe to `@deprecated` directive usages on field/enum value/type.
- Report when `reason` is absent or empty with message
  `@deprecated should have a reason` (verify exact wording).
- `requires_schema: true`.

**Out of scope:**

- `require-deprecation-date` (spec-027 — separate date argument).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_deprecation_reason.rs`.
- `rules-fixtures/require-deprecation-reason/`.
- `tests/rule_require_deprecation_reason.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "require-deprecation-reason", category = "schema", requires_schema)]
pub struct RequireDeprecationReason;
```

## Behavior

- `reason` absent → report.
- `reason: ""` → report.
- `reason: "some text"` → pass.
- Diagnoses the directive node's span (graphql-eslint's choice — verify).

## Testing

- `rglint_test_suite!("require-deprecation-reason")`.

## Risks / Notes

- graphql-eslint may treat the directive's *location* (the field) as the
  report site rather than the directive itself — confirm in fixtures.
