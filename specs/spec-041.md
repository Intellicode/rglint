# Spec-041: require-import-fragment

> Plan reference: §5 Phase 4, §3 (`crates/rglint-rules/src/schema/require_import_fragment.rs`)

## Goal

Port `require-import-fragment`: when an operation references a fragment by
`...FragmentName`, the fragment must be defined in the same document (file) —
otherwise it must be "imported" via a graphql-tools `#import` comment. Catches
missing cross-file fragment imports.

## Source

`packages/plugin/src/rules/require-import-fragment/index.ts`

## Scope

**In scope:**

- Rule id `require-import-fragment`, category `Operations`.
- For each `FragmentSpread` in an operation, check whether the fragment is
  defined in the same file; if not, check whether the file has a
  `#import "./frag.graphql" ...FragmentName`-style comment importing it (parse
  `#import` comments via the comment scanner from spec-024).
- Report unimported cross-file fragment spreads with message (verify wording).
- `requires_siblings: true` (to know where fragments live across files).

**Out of scope:**

- `#import` resolution semantics beyond name presence.

## Dependencies

- spec-006, spec-024 (comment scanner — reuse for `#import` parsing),
  spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/require_import_fragment.rs`.
- `rules-fixtures/require-import-fragment/`.
- `tests/rule_require_import_fragment.rs`.

## Interface / API

```rust
#[derive(Rule)]
#[rule(id = "require-import-fragment", category = "operations", requires_siblings)]
pub struct RequireImportFragment;
```

## Behavior

- A fragment spread whose target is in another file with no matching `#import`
  → report at the spread's span.
- `#import` syntax parsed: `#import "file.graphql"` (whole file) or
  `#import "file.graphql" #FragmentName` (specific fragments) — match
  graphql-eslint's accepted forms.
- In-file fragment definitions never require an import.

## Testing

- `rglint_test_suite!("require-import-fragment")`.

## Risks / Notes

- The `#import` grammar is a graphql-tools convention; reuses the comment
  scanner from spec-024 (which scans all `#` comments — add an `#import`
  parser on top).
