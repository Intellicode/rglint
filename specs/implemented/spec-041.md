# Spec-041: require-import-fragment

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/operations/require_import_fragment.rs`)

## Goal

Port `require-import-fragment`: when an operation references a fragment by
`...FragmentName`, the fragment must be defined in the same document (file) —
otherwise it must be imported via a graphql-tools `# import` comment. Catches
missing cross-file fragment imports.

## Source

`packages/plugin/src/rules/require-import-fragment/index.ts` and its upstream
RuleTester cases.

## Scope

- Rule id `require-import-fragment`, category `Operations`.
- For each `FragmentSpread`, check whether the fragment is defined in the same
  file; otherwise accept a document-wide named or default import comment whose
  imported path maps to a sibling fragment definition.
- Report unimported cross-file fragment spreads with the upstream message
  `Expected "FragmentName" fragment to be imported.` at the fragment-name span.
- `requires_siblings: true`.

Import resolution is intentionally limited to normalized path identity; it does
not resolve globs or implement suggestions/fixes in the current engine.

## Dependencies

- spec-006, spec-024 (comment scanner), spec-008, spec-009, spec-011,
  spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/require_import_fragment.rs`.
- `rules-fixtures/require-import-fragment/`.
- `crates/rglint-rules/tests/rule_require_import_fragment.rs`.

## Testing

- `cargo test -p rglint-rules --test rule_require_import_fragment`.
- Workspace build, clippy, and test checks.

## Notes

The upstream rule advertises suggestions. The Rust rule preserves that metadata
for registry parity, but suggestion edits remain out of scope until the engine's
fix/suggestion support is implemented.
