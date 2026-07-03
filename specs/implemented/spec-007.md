# Spec-007: Project resolution (graphql-config)

> Plan reference: §3 (`crates/rglint-core/src/project.rs`), §1 ("Project Resolution")

## Goal

Resolve a "project" — a `(schema, documents)` pair — from either an explicit
`.rglintrc` `projects` map or an interoperable `.graphqlrc` / `.graphqlconfig`
file. Mirrors `packages/plugin/src/processor.ts` and `graphql-config.ts`. One
workspace may contain multiple projects, each linted independently.

## Scope

**In scope:**

- `ProjectConfig` struct: `name`, `schema: SchemaSpec`, `documents:
  DocumentSpec`, `include`/`ignore` globs.
- `ProjectResolver` that:
  - Reads `projects` from `.rglintrc` (spec-054) OR `.graphqlrc.{yml,json}`
    (spec-055) OR `.graphqlconfig.json` (legacy).
  - If no projects key, synthesizes a single default project from top-level
    `schema`/`documents` keys.
  - Resolves relative paths against the config file's directory.
- `Project::resolve()` → produces the bound `(LoadedSchema, LoadedDocuments,
  Siblings)` by delegating to specs 004/005/006.

**Out of scope:**

- The `.rglintrc` schema itself (spec-054) — this spec only consumes the
  `projects` field.
- `.graphqlrc` parsing details (spec-055) — this spec calls into it.

## Dependencies

- spec-004 (SchemaLoader).
- spec-005 (DocumentLoader).
- spec-006 (Siblings).
- spec-054 (Config loader) — runtime; for tests can use inline `ProjectConfig`.

## Deliverables

- `crates/rglint-core/src/project.rs`.
- Integration test with a 2-project fixture mirroring
  `examples/multiple-projects-graphql-config`.

## Interface / API

```rust
pub struct ProjectConfig {
    pub name: String,
    pub schema: SchemaSpec,
    pub documents: DocumentSpec,
    pub ignore: Vec<String>,
}

pub struct Project {
    pub config: ProjectConfig,
    pub schema: Option<LoadedSchema>,
    pub documents: LoadedDocuments,
    pub siblings: Siblings,
}

pub struct ProjectResolver { base: PathBuf }
impl ProjectResolver {
    pub fn new(base: PathBuf) -> Self;
    pub fn resolve(&self, cfgs: &[ProjectConfig]) -> Result<Vec<Project>>;
}
```

## Behavior

- Multiple projects → `Vec<Project>`, each fully independent (no shared schema
  object between them).
- A project with `schema: None` still loads documents (schema-less linting;
  `requires_schema` rules self-skip).
- Missing schema files → hard error with the offending glob in the message.
- Missing document files when `documents` is set → error (an empty document set
  is suspicious). When `documents` is unset → no documents, schema-only lint.

## Testing

- `tests/fixtures/multi-project/` with `.graphqlrc` declaring `web` and `admin`
  projects, each with its own schema + one document. Assert `resolve()` returns
  2 `Project`s with correct schema types.
- Default-project synthesis: a `.rglintrc` with only top-level `schema`/`documents`
  yields one project named `"default"`.

## Risks / Notes

- `graphql-config` supports `schema: <http-url>`; v1 ignores that (PLAN §11
  stretch). Reject with a clear "not supported yet" error.
