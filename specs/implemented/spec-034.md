# Spec-034: no-deprecated

> Plan reference: §5 Phase 3, §3 (`crates/rglint-rules/src/schema/no_deprecated.rs`)

## Goal

Port `no-deprecated`: forbid operations from selecting fields/enum values that
are marked `@deprecated` in the schema. First schema-aware operations rule —
cross-references the schema's deprecation annotations.

## Source

`packages/plugin/src/rules/no-deprecated/index.ts`

## Scope

**In scope:**

- Rule id `no-deprecated`, category `Operations`.
- Options: `{ skip: [string] }` — list of type/field paths to skip (e.g.
  `["User.email"]`).
- Walk each operation's selection set; for each selected field, look up the
  schema field; if it's `@deprecated`, report
  `Field "${field}" is marked as deprecated in your GraphQL schema (reason: ${reason})`
  (verify exact wording — this is the message asserted in fixtures).
- Same for enum values selected via literal.
- `requires_schema: true`, `requires_siblings: true` (to follow fragment
  spreads into deprecated selections).

**Out of scope:**

- Schema-side deprecation policy (specs 026/027).

## Dependencies

- spec-004, spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/no_deprecated.rs`.
- `rules-fixtures/no-deprecated/`.
- `tests/rule_no_deprecated.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { #[serde(default)] skip: Vec<String> }

#[derive(Rule)]
#[rule(id = "no-deprecated", category = "operations", requires_schema, requires_siblings)]
pub struct NoDeprecated;
```

## Behavior

- Resolve selected field → schema field via `apollo_compiler` type info.
- `reason` empty → message omits `(reason: ...)` (match graphql-eslint
  conditional wording exactly).
- `skip` entries formatted `TypeName.fieldName` suppress that field.
- Fragments: recurse via `Siblings::get_fragments_in_use` so a deprecated
  field inside a fragment is also caught.

## Testing

- `rglint_test_suite!("no-deprecated")` — message parity is critical (fixture
  in PLAN §6.1 uses this exact message).

## Risks / Notes

- Enum value deprecation requires resolving the enum type; ensure
  apollo-compiler exposes `@deprecated` on `EnumValueDefinition`.
