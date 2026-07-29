# Spec-042: require-selections

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/schema/require_selections.rs`)

## Goal

Port graphql-eslint's `require-selections` rule. When a selected object,
interface, or union can provide one or more configured fields, require those
fields to be selected. Named fragments, nested fragments, inline fragments,
aliases, and sibling documents participate in the check.

## Source

`packages/plugin/src/rules/require-selections/index.ts` and
`packages/plugin/src/rules/require-selections/index.test.ts`.

## Scope

**In scope:**

- Rule id `require-selections`, category `Operations`.
- `requires_schema: true`, `requires_siblings: true`, and
  `has_suggestions: true`.
- Options are an object with `fieldName` (a string or array of strings,
  defaulting to `"id"`) and `requireAllFields` (default `false`). When false,
  selecting any available configured field satisfies the rule; when true, each
  available configured field is required.
- Object and interface result types, union selections through inline or named
  fragments, aliases, and recursive fragment spreads.
- Diagnostics use graphql-eslint's exact message template and point at the
  opening brace of the offending selection set. Direct operation selections
  receive `Add \`field\` selection` suggestions; fragment selections do not,
  because a fragment may be maintained in another file.

**Out of scope:**

- A type-to-field map such as `selections: { TypeName: [...] }`; that is not
  the upstream rule's API.
- `__typename` requirements (separate concern).

## Dependencies

- spec-004, spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_selections.rs`.
- `rules-fixtures/require-selections/`.
- `crates/rglint-rules/tests/rule_require_selections.rs`.

## Behavior

- The default rule requires `id` whenever that field exists on the selected
  result type.
- A configured field selected directly, by alias, through an inline fragment,
  or through a recursively resolved named fragment satisfies the requirement.
- For unions, fields available on selected concrete member types are checked;
  a field selected in any applicable inline or named fragment satisfies it.
- `requireAllFields = true` reports one diagnostic per missing available field;
  otherwise a single diagnostic reports the configured alternatives.
- Rules run per loaded source file. As required by the current engine, fixture
  sibling documents are real lint inputs, so source-owned fragment diagnostics
  are included in expected parity output.

## Testing

- `cargo test -p rglint-rules --test rule_require_selections` via
  `rglint_test_suite!("require-selections")`.
- Fixtures cover default and custom fields, alternatives, aliases, nested and
  sibling fragments, inline fragments, unions, and `requireAllFields`.

## Risks / Notes

- The imported upstream fixtures used legacy extensionless filenames and
  placeholder `"<unknown>"` messages. They were normalized to the current
  harness suffixes and updated from the upstream source's exact message
  template.
