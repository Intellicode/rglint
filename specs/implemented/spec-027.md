# Spec-027: require-deprecation-date

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/require_deprecation_date.rs`)

## Goal

Port `require-deprecation-date`: every `@deprecated` directive must carry a
`deletionDate` argument (configurable via `argumentName`) whose value is a
valid date in `DD/MM/YYYY` format. If the date is in the past, report that
the deprecated item can be removed.

## Source

`packages/plugin/src/rules/require-deprecation-date/index.ts`

## Scope

**In scope:**

- Rule id `require-deprecation-date`, category `Schema`.
- Options: `{ argumentName: string }` (default `"deletionDate"`).
- Four diagnostic messages, matching the original byte-for-byte:
  - `Directive "@deprecated" must have a deletion date for {nodeName}`
  - `Deletion date must be in format "DD/MM/YYYY" for {nodeName}`
  - `Invalid "{deletionDate}" deletion date for {nodeName}`
  - `{nodeName} сan be removed` (note: Cyrillic `с` U+0441)
- `nodeName` uses `getNodeName` from the original: for field / input value /
  enum value definitions, includes the container type
  (e.g. `field "oldField" in type "Old"`).
- Report locations match the original: directive name for missing argument,
  argument value for invalid format/date, definition name for past-date.
- `requires_schema: true`.

**Out of scope:**

- Suggestions / fix support (original has `suggest: [Remove Old]`).
- `require-deprecation-reason` (spec-026).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.
- spec-022 (case styles) — not needed directly, but the shared `display_kind_label` /
  `get_node_name` pattern from spec-025 / spec-026 informs the node-name helpers.

## Deliverables

- `crates/rglint-rules/src/schema/require_deprecation_date.rs`.
- `rules-fixtures/require-deprecation-date/` (fixtures already exist; update
  `config.toml` to add `kind = "schema"` and `expected.json` with correct messages).
- `tests/rule_require_deprecation_date.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    argument_name: Option<String>,  // maps JSON option `argumentName`
}
```

## Behavior

1. For each `@deprecated` directive, check that it has an argument whose name
   matches `argumentName` (default `"deletionDate"`).
2. If no such argument → report with `Directive "@deprecated" must have a
   deletion date for {nodeName}` at the directive name.
3. If the argument value does not match `/^\d{2}\/\d{2}\/\d{4}$/` → report with
   `Deletion date must be in format "DD/MM/YYYY" for {nodeName}` at the value.
4. If the value matches the regex but is not a valid calendar date (e.g.
   `"32/08/2021"`) → report with `Invalid "{value}" deletion date for {nodeName}`
   at the value.
5. If the date is valid but in the past (before `Date.now()`) → report with
   `{nodeName} сan be removed` at the definition name.

## Testing

- `rglint_test_suite!("require-deprecation-date")`.
- Fixtures cover: no deprecated at all, valid date (future), custom argument name,
  missing argument, past date, invalid format, invalid calendar date.

## Risks / Notes

- The `с` in `сan be removed` is Cyrillic Small Letter Es (U+0441), not
  Latin `c`. Must be byte-identical.
- `getNodeName` helper must replicate graphql-eslint's `displayNodeName` /
  `DisplayNodeNameMap`: top-level definitions get just the kind label + name
  (e.g. `scalar "Old"`), while fields / input values / enum values also include
  the container (e.g. `field "oldField" in type "Old"`).
- Date validation is done by parsing day/month/year components — no external
  date library needed.
