# Spec-045: relay-arguments

> Plan reference: §5 Phase 5, §3 (`crates/rglint-rules/src/schema/relay_arguments.rs`)

## Goal

Port `relay-arguments`: fields returning a Connection must expose the standard
forward (`first`, `after`) and/or backward (`last`, `before`) pagination
arguments with correct types.

## Source

Parity was checked against graphql-eslint commit
`f0f200ef0b030cb8a905bbcb32fe346b87cc2e24` (2026-07-30):

- [Rule source](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-arguments/index.ts)
- [Rule tests](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-arguments/index.test.ts)
- [Parity snapshot](https://github.com/graphql-hive/graphql-eslint/blob/f0f200ef0b030cb8a905bbcb32fe346b87cc2e24/packages/plugin/src/rules/relay-arguments/snapshot.md)

## Scope

**In scope:**

- Rule id `relay-arguments`, category `Schema`.
- Options: `{ includeBoth: bool }` (whether both forward+backward must be
  present; default `true`).
- For each field whose return type name ends in `Connection` (the connection
  shape is checked by spec-046), check its arguments: `first: Int`,
  `after: String` (forward) and `last: Int`, `before: String` (backward).
- `after` and `before` accept `String` or any scalar type; `first` and `last`
  accept `Int`, with or without non-null wrappers. Lists are rejected.
- Report missing/mistyped pagination args with graphql-eslint's exact message.
- `requires_schema: true`.

**Out of scope:**

- Connection/Edge/PageInfo type shape (specs 046-048).

## Dependencies

- spec-044 (Relay predicates — hard prerequisite).
- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/relay_arguments.rs`.
- `rules-fixtures/relay-arguments/`.
- `tests/rule_relay_arguments.rs`.

## Interface / API

```rust
#[derive(Deserialize)]
struct Opts {
    #[serde(default = "default_true")] include_both: bool,
}
```

## Behavior

- A Connection field with only forward args + `includeBoth: true` → report
  missing backward args.
- Argument type mismatches (`first: String`) → report the argument name.
- The general missing-pair diagnostic is reported at the field name. A missing
  individual argument is also reported at the field name; a present but
  mistyped argument is reported at its argument name.

## Testing

- `rglint_test_suite!("relay-arguments")`.

## Risks / Notes

- The upstream selector uses the literal `Connection` suffix. The shared
  `RelayOpts` naming options are therefore not applied by this rule because
  graphql-eslint does not expose them here.
