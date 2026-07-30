# Spec-043: no-one-place-fragments

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/operations/no_one_place_fragments.rs`)

## Goal

Port `no-one-place-fragments`: a fragment definition that is spread in exactly
one location should be inlined (it adds indirection without reuse). Uses
siblings to count spread usage across all operations and fragment definitions.

## Source

Rule: `packages/plugin/src/rules/no-one-place-fragments/index.ts`

Test: `packages/plugin/src/rules/no-one-place-fragments/index.test.ts`

Verified against the upstream `master` sources at these immutable revisions
(fetched 2026-07-30):

- Rule: https://github.com/graphql-hive/graphql-eslint/blob/e94f813ae4180b5908855989e1243e5b958581c1/packages/plugin/src/rules/no-one-place-fragments/index.ts
- Tests: https://github.com/graphql-hive/graphql-eslint/blob/01ace44e07d330ac98369b318d175b67ab5c5605/packages/plugin/src/rules/no-one-place-fragments/index.test.ts

## Scope

**In scope:**

- Rule id `no-one-place-fragments`, category `Operations`.
- No options. The upstream rule reports only the exact-one-use case.
- For each fragment definition, count its fragment-spread occurrences across
  all sibling operations and fragment definitions. A single operation that
  spreads the same fragment twice counts as two uses.
- Report the fragment name with the exact upstream message:
  `Fragment \`<name>\` used only once. Inline him in "<file>".`
- `requires_siblings: true`; no schema and no suggestions.

**Out of scope:**

- Auto-inlining (suggestion only; `--fix` is spec-061).

## Dependencies

- spec-006, spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/no_one_place_fragments.rs`.
- `rules-fixtures/no-one-place-fragments/`.
- `crates/rglint-rules/tests/rule_no_one_place_fragments.rs`.

## Behavior

- Count spreads by name across all operations and fragment definitions (a
  single operation using it twice = count 2, not 1).
- Unused fragments (count 0) are skipped; `no-unused-fragments` owns that
  diagnostic.
- Report at the fragment name node, matching the upstream visitor
  `FragmentDefinition > Name`.
- Include the basename of the file containing the only spread in the message,
  matching the fixture/test helper's relative-file rendering.

## Testing

- `rglint_test_suite!("no-one-place-fragments")`.
- Unit: a fragment used twice → 0 diagnostics; used once → 1 diagnostic.

## Risks / Notes

- The upstream test source confirms count is per-spread-site and that the
  diagnostic points at the fragment name, not the full definition.
