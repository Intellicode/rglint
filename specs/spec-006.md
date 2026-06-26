# Spec-006: Sibling operations index (FragmentTracker)

> Plan reference: §3 (`crates/rglint-core/src/siblings.rs`), §1 ("Sibling Operations Index"), §4.4

## Goal

Build the cross-document index of operations and fragment definitions across
all documents in a project, so rules can answer "which fragments does this
operation use?" and "where is fragment `X` defined?". Mirrors
`packages/plugin/src/siblings.ts`. Required by Phase 4 rules and any rule with
`requires_siblings: true`.

## Scope

**In scope:**

- `Siblings` struct indexing `LoadedDocuments`:
  - `operations: Vec<OperationDefinition>` (with back-ref to source file).
  - `fragment_index: AHashMap<String, FragmentDef>` keyed by fragment name.
  - `document_by_file: AHashMap<PathBuf, usize>`.
- `Siblings::get_fragments_in_use(op) -> Vec<&FragmentDef>` — recursively
  walks an operation's selection set, resolving fragment spreads, handling
  cycles (visited-set guard).
- `Siblings::get_operation_by_name(name) -> Option<&OperationDef>`.
- `Siblings::get_fragment_by_name(name) -> Option<&FragmentDef>`.

**Out of scope:**

- Depth computation (that's `selection-set-depth`, spec-040, built on top of
  this).
- Loading documents (spec-005).

## Dependencies

- spec-005 (LoadedDocuments).
- spec-002 (SourceFile).

## Deliverables

- `crates/rglint-core/src/siblings.rs`.
- Unit tests: cyclic fragments (`A → B → A`) don't infinite-loop; nested
  fragments resolve transitively.

## Interface / API

```rust
pub struct FragmentDef {
    pub name: String,
    pub source: Arc<SourceFile>,
    pub span: Span,
    pub node: ast::FragmentDefinition,   // borrow from apollo-compiler
}

pub struct OperationDef {
    pub name: Option<String>,
    pub source: Arc<SourceFile>,
    pub span: Span,
    pub node: ast::OperationDefinition,
}

pub struct Siblings {
    operations: Vec<OperationDef>,
    fragments: AHashMap<String, FragmentDef>,
    doc_by_file: AHashMap<PathBuf, usize>,
}

impl Siblings {
    pub fn from_documents(docs: &LoadedDocuments) -> Self;
    pub fn get_fragments_in_use(&self, op: &ast::OperationDefinition) -> Vec<&FragmentDef>;
    pub fn get_operation_by_name(&self, name: &str) -> Option<&OperationDef>;
    pub fn get_fragment_by_name(&self, name: &str) -> Option<&FragmentDef>;
    pub fn operations(&self) -> &[OperationDef];
    pub fn fragments(&self) -> impl Iterator<Item = (&String, &FragmentDef)>;
}
```

## Behavior

- `get_fragments_in_use` performs a recursive walk with a `HashSet<String>`
  visited guard; returns deduplicated, topologically-stable order (insertion
  order of first encounter).
- Fragment name collisions across files: last-wins with a logged warning
  (graphql-eslint treats duplicate fragment names as a `unique-fragment-name`
  violation; that rule — spec-017 — fires separately; here we just index).

## Testing

- Fixture: 3 documents — `op.graphql` (uses `A` and `B`), `fragA.graphql`
  (`fragment A { ... B }`), `fragB.graphql` (`fragment B { ... }`). Assert
  `get_fragments_in_use(op)` returns `[A, B]`.
- Cyclic fixture: `A` uses `B`, `B` uses `A` → terminates, returns `[A, B]`.

## Risks / Notes

- Lifetime management: storing `ast::FragmentDefinition` (which borrows from
  `apollo_compiler::ExecutableDocument`) requires either copying the relevant
  data out or holding `Arc<ExecutableDocument>` alongside. Prefer copying
  name + span + a lightweight selection-set reference to avoid lifetime
  gymnastics. Decide during spike.
