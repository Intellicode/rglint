# Spec-031: no-typename-prefix

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/no_typename_prefix.rs`)

## Goal

Port `no-typename-prefix`: forbid type names prefixed with `__` (reserved for
introspection). Schema-only.

## Source

`packages/plugin/src/rules/no-typename-prefix/index.ts`

## Scope

**In scope:**

- Rule id `no-typename-prefix`, category `Schema`.
- For each type definition (object, interface, union, enum, input, scalar,
  directive), if the name starts with `__`, report
  `Type "${name}" must not start with "__"` (verify exact wording).

**Out of scope:**

- Field name `__` prefixes (covered by `naming-convention` `forbiddenPrefixes`).

## Dependencies

- spec-008, spec-009, spec-011, spec-012, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_typename_prefix.rs`.
- `rules-fixtures/no-typename-prefix/`.
- `tests/rule_no_typename_prefix.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "no-typename-prefix", category = "schema")]
pub struct NoTypenamePrefix;
```

## Behavior

- Reports at the type name span.
- Trivial predicate: `name.starts_with("__")`.

## Testing

- `rglint_test_suite!("no-typename-prefix")`.

## Risks / Notes

- None expected; small rule.
