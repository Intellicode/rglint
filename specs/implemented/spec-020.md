# Spec-020: lone-executable-definition

> Plan reference: §5 Phase 1, §3 (`crates/rglint-rules/src/schema/lone_executable_definition.rs`)

## Goal

Port `lone-executable-definition`: a `.graphql` file containing executable
definitions (operations/fragments) may contain at most one definition, unless
the others are system definitions. Mirrors graphql-js's
`lone-executable-definition` graphql-eslint wrapper.

## Source

`packages/plugin/src/rules/lone-executable-definition/index.ts`

## Scope

**In scope:**

- Rule id `lone-executable-definition`, category `Operations`.
- `Handler::finalize`: count executable definitions (operations + fragment
  definitions) in the **current file**; if >1, report all-but-the-first with
  message `This file contains ${n} executable definitions; only 1 is allowed` —
  verify exact wording against the TS source.
- Option `allowAllDefinitions: bool` (default false) — when true, skip.
- `requires_schema: false`, `requires_siblings: false`.

**Out of scope:**

- Schema-side lone-definition rules (graphql-eslint doesn't have one).

## Dependencies

- spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/lone_executable_definition.rs`.
- `rules-fixtures/lone-executable-definition/`.
- `tests/rule_lone_executable_definition.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { #[serde(default)] allow_all_definitions: bool }

#[derive(Rule)]
#[rule(id = "lone-executable-definition", category = "operations")]
pub struct LoneExecutableDefinition;
```

## Behavior

- Counts only `OperationDefinition` + `FragmentDefinition` (not type/system
  definitions if a file mixes them — graphql-eslint's exact scoping; confirm
  in TS source).
- The first definition is kept; each subsequent one is reported.
- `allowAllDefinitions: true` disables the rule entirely.

## Testing

- `rglint_test_suite!("lone-executable-definition")`.
- Unit: 3 operations in one file + default opts → 2 diagnostics; with
  `allowAllDefinitions: true` → 0.

## Risks / Notes

- Confirm whether graphql-eslint counts fragments separately from operations
  for the "lone" check; the TS source is authoritative.
