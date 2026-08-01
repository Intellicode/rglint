# Spec-052: input-name

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/input_name.rs`)

## Goal

Port `input-name`: mutation arguments should be named `input`, and, when
enabled, their named input type should follow the `<mutationName>Input`
convention (for example, `SetMessageInput`). Queries can opt into the same
argument/type checks.

## Source

`packages/plugin/src/rules/input-name/index.ts` at immutable upstream revision
`89ec798db4b78fb872078b1ce7e6046856b704fe`.

The parity tests are in
`packages/plugin/src/rules/input-name/index.test.ts` at the same revision.

## Scope

**In scope:**

- Rule id `input-name`, category `Schema`.
- Options: `checkInputType` (default `false`),
  `caseSensitiveInputType` (default `true`), `checkQueries` (default `false`),
  and `checkMutations` (default `true`).
- For checked `Mutation`/`Query` object definitions and extensions, every
  argument other than `input` is reported.
- With `checkInputType`, the named type of every argument on a checked field
  must be exactly `<fieldName>Input`, subject to the case-insensitive option.
- The rule intentionally checks the local SDL shape, matching the upstream
  visitor; it does not determine whether the referenced type is actually an
  input object and does not require a compiled schema.

**Out of scope:**

- General naming conventions (spec-028).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/input_name.rs`.
- `rules-fixtures/input-name/`.
- `tests/rule_input_name.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts {
    #[serde(default)] check_input_type: bool,
    #[serde(default = "default_case_sensitive")] case_sensitive_input_type: bool,
    #[serde(default)] check_queries: bool,
    #[serde(default = "default_check_mutations")] check_mutations: bool,
}
```

## Behavior

- A non-`input` argument on a checked root object reports:
  `Input "<argument>" should be named "input" for "<Root>.<field>"`.
- With `checkInputType`, a named argument type that does not match the field
  convention reports:
  `Input type \`<actual>\` name should be \`<field>Input\`.`.
- Argument-name reports suggest renaming to `input`; type-name reports suggest
  renaming to the expected `<field>Input`.
- Rule metadata is `category = Schema`, `requires_schema = false`, and
  `has_suggestions = true`.

## Testing

- `rglint_test_suite!("input-name")`.

## Risks / Notes

- This is a deliberate correction from the original draft: upstream does not
  inspect input-object definitions, expose `suffix`/`argumentName`, or require
  schema compilation. Fixtures and the implementation must follow the pinned
  source and tests above.
