# Spec-045: relay-arguments

> Plan reference: §5 Phase 5, §3 (`crates/rglint-rules/src/schema/relay_arguments.rs`)

## Goal

Port `relay-arguments`: fields returning a Connection must expose the standard
forward (`first`, `after`) and/or backward (`last`, `before`) pagination
arguments with correct types.

## Source

`packages/plugin/src/rules/relay-arguments/index.ts`

## Scope

**In scope:**

- Rule id `relay-arguments`, category `Schema`.
- Options: `{ includeBoth: bool }` (whether both forward+backward must be
  present; default — confirm from TS).
- For each field whose return type is a Connection (via spec-044), check its
  arguments: `first: Int`, `after: String` (forward) and `last: Int`,
  `before: String` (backward).
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
    #[serde(default)] include_both: bool,
    // + passthrough RelayOpts (connection/edge/pageInfo patterns)
}
```

## Behavior

- A Connection field with only forward args + `includeBoth: true` → report
  missing backward args.
- Argument type mismatches (`first: String`) → report.
- Reports at the field definition span.

## Testing

- `rglint_test_suite!("relay-arguments")`.

## Risks / Notes

- Verify default `includeBoth` from TS fixtures (likely false — forward-only
  is common).
