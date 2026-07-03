# Spec-005: Document loader & dedup

> Plan reference: §3 (`crates/rglint-core/src/documents.rs`), §1 ("Documents (globs → files)"), §4.4

## Goal

Load GraphQL operation documents (`.graphql` files containing operations +
fragments) from disk into `apollo_compiler::ExecutableDocument`s, deduplicating
by content hash so the same document imported via overlapping globs is linted
once. Mirrors `packages/plugin/src/documents.ts`.

## Scope

**In scope:**

- `DocumentLoader` resolving a glob/list to `Vec<Arc<SourceFile>>`.
- Parsing each into `apollo_compiler::ExecutableDocument` (with an optional
  `Schema` for type-aware parsing).
- Content-hash dedup: identical files contribute one document.
- Parse-error routing → `Diagnostic` with `rule_id = "parse-error"`.
- A `LoadedDocuments` bundle holding the per-file documents + parse errors.

**Out of scope:**

- Sibling fragment indexing (spec-006 builds the cross-document index on top of
  this).
- Schema loading (spec-004).

## Dependencies

- spec-002 (SourceFile).
- spec-003 (Diagnostic).
- spec-004 (LoadedSchema — optional, passed for type-aware parsing).

## Deliverables

- `crates/rglint-core/src/documents.rs`.
- Unit tests: two identical files → one document; a glob with overlaps dedups.

## Interface / API

```rust
pub struct LoadedDocument {
    pub source: Arc<SourceFile>,
    pub document: apollo_compiler::ExecutableDocument,
    pub parse_errors: Vec<Diagnostic>,
}

pub struct LoadedDocuments {
    pub docs: Vec<LoadedDocument>,            // deduplicated
    pub by_file: AHashMap<PathBuf, usize>,    // path -> index into docs
}

pub struct DocumentLoader { /* globset, ignore */ }
impl DocumentLoader {
    pub fn new() -> Self;
    pub fn load(
        &self,
        spec: &DocumentSpec,
        base: &Path,
        schema: Option<&apollo_compiler::Schema>,
    ) -> Result<LoadedDocuments>;
}

pub enum DocumentSpec {
    Glob(String),
    Globs(Vec<String>),
    Files(Vec<PathBuf>),
    Inline(String),
}
```

## Behavior

- Dedup key = `xxhash` of file content (spec-013's cache primitive).
- Documents are parsed **with** the schema when provided so fragment/operation
  types resolve; if no schema, parse standalone (some rules will skip
  themselves — engine handles via `requires_schema`).
- `by_file` lets siblings (spec-006) and reporters map a diagnostic back to a
  file even when the executable doc was built from a combined source.

## Testing

- Two `.graphql` files with identical content → `LoadedDocuments.docs.len() == 1`.
- A malformed operation yields a `parse-error` diagnostic with the correct file.
- Glob with `**` recursion respects `.gitignore` (via `ignore` crate).

## Risks / Notes

- apollo-compiler `ExecutableDocument::parse` requires a schema for some
  validation; verify behavior when schema is `None` (standalone parse). If it
  forces a dummy schema, construct one.
