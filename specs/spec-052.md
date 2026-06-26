# Spec-052: input-name

> Plan reference: §5 Phase 6, §3 (`crates/rglint-rules/src/schema/input_name.rs`)

## Goal

Port `input-name`: input object types should be named with an `Input` suffix
(e.g. `CreateUserInput`), and a separate convention for the field that
receives them on mutations.

## Source

`packages/plugin/src/rules/input-name/index.ts`

## Scope

**In scope:**

- Rule id `input-name`, category `Schema`.
- Options: `{ suffix: "Input", argumentName: "input" }` (port exact shape;
  may include `checkInputType: bool`).
- For each input object type whose name lacks the suffix → report.
- For each mutation field's `input` argument whose type isn't an input type
  (or whose name doesn't match `argumentName`) → report.
- `requires_schema: true`.

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
    #[serde(default = "default_input")] suffix: String,
    #[serde(default = "default_arg")] argument_name: String,
    #[serde(default)] check_input_type: bool,
}
```

## Behavior

- A mutation field's argument named `input` (or `argumentName`) must be of an
  input type ending in `suffix`.
- Input type missing suffix → report with suggestion to rename + strip.
- `hasSuggestions: true` if graphql-eslint offers renames (confirm).

## Testing

- `rglint_test_suite!("input-name")`.

## Risks / Notes

- Confirm whether graphql-eslint checks only `Mutation` field args or all
  field args; fixtures decide.
