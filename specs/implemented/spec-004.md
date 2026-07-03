# Spec-004: Schema loader & cache

> Plan reference: §3 (`crates/rglint-core/src/schema.rs`), §1 ("Schema Loader"), §4.4

## Goal

Load GraphQL schemas from disk (single file, glob, or multi-file concat) into
`apollo_compiler::Schema`, carrying source spans through. Mirrors
`packages/plugin/src/schema.ts`. Provides an in-memory cache keyed by resolved
schema identity so multiple documents in one project share one `Schema`.

## Scope

**In scope:**

- `SchemaLoader` that resolves a schema spec (path / glob / list of files) into
  a single combined source, then `apollo_compiler::Schema::parse`.
- Handles `*.graphql` and `*.graphqls` files; concatenates multiple files with
  newlines (matching graphql-config behavior).
- Records per-file `SourceFile` so a diagnostic can still point at the right
  physical file even when the schema is a union.
- Parse-error routing: apollo-parser errors collected from the compiler become
  `Diagnostic` with `rule_id = "parse-error"` (the engine surfaces these;
  spec-011 wires it, this spec exposes a `parse_errors() -> Vec<Diagnostic>`).
- Cache keyed by content hash (delegates to spec-013's `Cache`).

**Out of scope:**

- HTTP/registry schema fetch (PLAN §11 stretch — out of scope for v1).
- Document loading (spec-005).

## Dependencies

- spec-002 (SourceFile, Span).
- spec-003 (Diagnostic).
- spec-013 (Cache) — soft dependency; if 013 not done, use a plain `HashMap`.

## Deliverables

- `crates/rglint-core/src/schema.rs`.
- Unit tests loading a 2-file schema and asserting a known type is present.

## Interface / API

```rust
pub struct LoadedSchema {
    pub compiler: apollo_compiler::Schema,
    pub sources: Vec<Arc<SourceFile>>,   // each physical file
    pub combined: Arc<SourceFile>,        // concatenated view (for span resolution)
    pub parse_errors: Vec<Diagnostic>,
}

pub struct SchemaLoader { /* globset, ignore */ }
impl SchemaLoader {
    pub fn new() -> Self;
    pub fn load(&self, spec: &SchemaSpec, base: &Path) -> Result<LoadedSchema>;
}

pub enum SchemaSpec {
    File(PathBuf),
    Glob(String),
    Files(Vec<PathBuf>),
    Inline(String),
}
```

## Behavior

- File order for globs is deterministic (sorted by path) so spans are stable.
- Concatenated source line offsets are remapped so a `NodeLocation` in the
  combined source can be attributed back to the originating physical file
  (helper `LoadedSchema::resolve_file(span) -> &SourceFile`).
- apollo-compiler's spec validation is **not** run here (spec-053 owns that);
  only parsing into `Schema`.

## Testing

- Load `tests/fixtures/schema/two-files/*.graphqls`, assert `LoadedSchema`
  contains `type Query` from file A and `type User` from file B.
- Malformed schema yields `parse_errors` with `rule_id = "parse-error"` and a
  span in the right file.
- Loading the same glob twice returns cached `LoadedSchema` (same `Arc` pointer
  when spec-013 is integrated).

## Risks / Notes

- apollo-compiler `Schema::parse` takes a single source; for multi-file we
  concatenate. Verify `NodeLocation` offsets remain meaningful across the
  concatenation boundary — if not, parse each file and merge via
  `apollo_compiler::Schema` builder APIs instead. Spike before committing.
