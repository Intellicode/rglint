# Spec-027: require-deprecation-date

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/require_deprecation_date.rs`)

## Goal

Port `require-deprecation-date`: every `@deprecated` directive must carry a
`reason` ending with (or containing) an RFC-3339 date, signaling when the
deprecation occurred. Optionally validate the date format.

## Source

`packages/packages/plugin/src/rules/require-deprecation-date/index.ts`

## Scope

**In scope:**

- Rule id `require-deprecation-date`, category `Schema`.
- Options: `{ reasonRegex: string }` (default matches a date in the reason
  text; port the exact default regex from TS).
- For each `@deprecated`, test `reason` against the regex; report when no
  match with message `@deprecated should have a date in its reason` (verify
  exact wording).
- `requires_schema: true`.

**Out of scope:**

- `require-deprecation-reason` (spec-026).

## Dependencies

- spec-004, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_deprecation_date.rs`.
- `rules-fixtures/require-deprecation-date/`.
- `tests/rule_require_deprecation_date.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { reason_regex: Option<String> } // default applied in create()
```

## Behavior

- Default regex matches RFC-3339 dates (`YYYY-MM-DD` and the full form) inside
  the reason text.
- Custom `reasonRegex` replaces the default (string regex; compile once at
  `create`).
- Empty reason → report (no date to match).

## Testing

- `rglint_test_suite!("require-deprecation-date")`.
- Unit: a custom regex `\\[JIRA-\\d+\\]` matches `reason: "see [JIRA-123]"`.

## Risks / Notes

- If the default regex uses lookbehind, switch this rule's regex to
  `fancy-regex` (PLAN §2 / §8). Confirm during port.
