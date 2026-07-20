# Spec-033: match-document-filename

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/operations/match_document_filename.rs`)

## Goal

Port `match-document-filename`: the operation name in a `.graphql` file must
match the file's stem (e.g. `GetUser.graphql` must contain `query GetUser`).
File-name-based rule — no AST walking beyond reading the operation name.

## Source

`packages/plugin/src/rules/match-document-filename/index.ts`

## Scope

**In scope:**

- Rule id `match-document-filename`, category `Operations`.
- Options: `{ matchObjectType: "Query" | "Mutation" | "Subscription" | "any",
  caseInsensitive: bool }` (port exact shape).
- For each operation in a file, compare its name to the file stem; report on
  mismatch with `Operation name "${op}" should match file name "${stem}"`
  (verify exact wording).
- Anonymous operations: skip (spec-016 covers them).
- Files with multiple operations: report each whose name doesn't match (or
  skip per option — confirm TS behavior).

**Out of scope:**

- Fragment-file matching (graphql-eslint is operation-only here).

## Dependencies

- spec-008, spec-009, spec-011, spec-012, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/operations/match_document_filename.rs`.
- `rules-fixtures/match-document-filename/`.
- `tests/rule_match_document_filename.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    match_object_type: MatchType,         // default Any
    #[serde(default)] case_insensitive: bool,
}
```

## Behavior

- `matchObjectType` filters which operation kinds are checked.
- File stem via `path.file_stem()`.
- `caseInsensitive` lowercases both sides before compare.

## Testing

- `rglint_test_suite!("match-document-filename")`.
- Unit: `GetUser.graphql` with `query getUser` + `caseInsensitive: true` → pass;
  case-sensitive → 1 diagnostic.

## Risks / Notes

- The rule needs the file path; `RuleContext::file.path()` provides it. For
  multi-operation files, confirm graphql-eslint reports per-mismatch or once
  per file.
