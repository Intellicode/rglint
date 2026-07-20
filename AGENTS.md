# AGENTS.md — conventions for implementing specs

## Spec lifecycle

1. Next unimplemented spec = lowest-numbered `spec-NNN.md` still in `specs/` (not in `specs/implemented/`). Check `specs/README.md` for the status index.
2. Read the spec, understand dependencies (they state which prior specs must be done).
3. Check `rules-fixtures/<rule-id>/` — if fixtures already exist, they are the ground truth. The spec text may be stale or simplified; always match the fixture `expected.json` messages and the original graphql-eslint source linked in `manifest.json`. If fixture source files lack a `.graphql`/`.gql` extension, rename them (extensionless files won't be found by the harness).
4. Implement per the spec's Deliverables (amended by fixture reality if applicable).
5. Move `specs/spec-NNN.md` → `specs/implemented/spec-NNN.md`.
6. Update `specs/README.md`: fix the link path (add `implemented/` prefix) and change status to `[x]`.
7. Build + test before committing.

## Rule implementation template

New rule file: `crates/rglint-rules/src/<category>/<snake_case>.rs`

```rust
//! `<rule-id>` (spec-NNN).

use rglint_core::{Handler, RuleContext};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(id = "<rule-id>", category = "<category>")]
pub struct PascalRule;

impl PascalRule {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(PascalHandler)
    }
}

struct PascalHandler;

impl Handler for PascalHandler { /* on_node and/or finalize */ }

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_NNN() {
        let rule = PascalRule;
        let meta = rule.meta();
        assert_eq!(meta.id, "<rule-id>");
        assert_eq!(meta.category, Category::Schema); // or Operations / Other
        assert_eq!(meta.severity, Severity::Warn);
        // add assertion for requires_schema / requires_siblings as needed
    }
}
```

Register in `crates/rglint-rules/src/<category>/mod.rs`: add `pub mod <snake_case>;`
Create the `<category>/mod.rs` if it doesn't exist yet.
Also add `pub mod <category>;` in `crates/rglint-rules/src/lib.rs`.

## `on_node` rules subscribing to multiple kinds (spec-019 pattern)

Use pipe-separated kind names in `kinds = "KIND1|KIND2|KIND3"`:

```rust
#[derive(Rule)]
#[rule(
    id = "some-rule",
    category = "schema",
    kinds = "FIELD_DEFINITION|INPUT_VALUE_DEFINITION|FIELD"
)]
pub struct SomeRule;
```

The derive macro splits on `|` or `,` and emits `rglint_core::SyntaxKind::KIND1, rglint_core::SyntaxKind::KIND2, ...`.

### Parent-chain walking to find containers

When a rule needs to find a containing node (e.g. which type definition a field belongs to), walk up the `Node.parent` chain from the `_parent` parameter:

```rust
fn find_container<'a>(parent: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = parent;
    loop {
        if matches!(current.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::SELECTION_SET
        ) {
            return Some(current);
        }
        current = current.parent?;
    }
}
```

### Tracking state per container

Use `HashMap<usize, HashMap<String, Span>>` keyed by the container's span offset:

```rust
struct SomeHandler {
    // container_offset → field_name → first_occurrence_span
    seen: HashMap<usize, HashMap<String, Span>>,
}
```

### Diagnostic message formatting

`format!` requires a string literal — use inline format strings rather than const `&str` patterns:

```rust
// GOOD:
format!("Field \"{name}\" is defined multiple times")

// BAD (compile error):
const MSG: &str = "Field \"{}\" is defined multiple times";
format!(MSG, name)  // error: format argument must be a string literal
```

## `requires_siblings` rules (spec-017 pattern)

Use `#[rule(…, requires_siblings = true)]`. Work is done in `Handler::finalize`:

```rust
fn finalize(&mut self, ctx: &mut RuleContext) {
    let Some(siblings) = ctx.siblings else { return; };
    // Use siblings.operations() or siblings.fragments_all()
    // Count occurrences, then attribute to current file via op.source.path() == this_file
}
```

Report via `ctx.report(DiagnosticBuilder::new(ctx.rule_id(), …, span, message))`.

## `requires_schema` rules (spec-034 pattern)

Schema-aware operation rules collect context during `on_node` and resolve against the compiled schema in `finalize`:

```rust
#[derive(Rule)]
#[rule(
    id = "no-deprecated",
    category = "operations",
    requires_schema = true,
    kinds = "FIELD|ARGUMENT|ENUM_VALUE|OBJECT_FIELD"
)]
pub struct SomeRule;

struct Candidate { name: String, span: Span, field_chain: Vec<String> }

struct SomeHandler { candidates: Vec<Candidate> }

impl Handler for SomeHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        // Walk parent chain to collect field names for context
        let mut field_chain: Vec<String> = Vec::new();
        let mut current = node.parent;
        while let Some(p) = current {
            if let Some(ref n) = p.name { field_chain.push(n.clone()); }
            current = p.parent;
        }
        field_chain.reverse();
        self.candidates.push(Candidate { name: ..., span: ..., field_chain });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let schema = match ctx.schema { Some(s) => s, None => return };
        // Resolve types against schema using field_chain + root types
    }
}
```

### Resolving root types from schema

```rust
fn root_type_names(schema: &apollo_compiler::Schema) -> Vec<String> {
    let def = &schema.schema_definition;
    vec![
        def.query.as_ref().map(|n| n.to_string()).unwrap_or_else(|| "Query".into()),
        def.mutation.as_ref().map(|n| n.to_string()).unwrap_or_else(|| "Mutation".into()),
        def.subscription.as_ref().map(|n| n.to_string()).unwrap_or_else(|| "Subscription".into()),
    ]
}
```

### Looking up fields from root types

The compiled schema's `ObjectType` and `InterfaceType` have `fields: IndexMap<Name, Component<FieldDefinition>>`. Use `indexmap`'s `.get()` with a `&str`, then deref through `Component` → `Node` → `FieldDefinition`:

```rust
use std::ops::Deref;

let field_def: &apollo_compiler::ast::FieldDefinition =
    obj.fields.get(field_name)?.deref().deref();
// Component<FieldDefinition> -> Deref -> Node<FieldDefinition> -> Deref -> FieldDefinition
```

### Checking deprecation via DirectiveList

The `ast::DirectiveList::get(name)` returns `Option<&Node<Directive>>`. Use auto-deref to call methods on `Directive`:

```rust
fn is_deprecated(directives: &apollo_compiler::ast::DirectiveList) -> bool {
    directives.get("deprecated").is_some()
}

fn deprecation_reason(directives: &apollo_compiler::ast::DirectiveList, schema: &apollo_compiler::Schema) -> String {
    directives
        .get("deprecated")
        .and_then(|dir| dir.argument_by_name("reason", schema).ok())
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| "No longer supported".to_owned())
}
```

### Resolving type names from Type enum

Strip NonNull/List wrappers to get the base named type:

```rust
fn resolve_base_type_name(ty: &apollo_compiler::ast::Type) -> Option<&str> {
    match ty {
        apollo_compiler::ast::Type::Named(name)
        | apollo_compiler::ast::Type::NonNullNamed(name) => Some(name.as_str()),
        apollo_compiler::ast::Type::List(inner)
        | apollo_compiler::ast::Type::NonNullList(inner) => resolve_base_type_name(inner),
    }
}
```

Then look up the resolved name in the schema:

```rust
schema.get_enum("EnumTypeName")     // -> Option<&Node<EnumType>>
schema.get_input_object("InputName") // -> Option<&Node<InputObjectType>>
```

## Test file template

New file: `crates/rglint-rules/tests/rule_<snake_case>.rs`

```rust
use std::fs;
use std::path::Path;

use rglint_test_harness::rglint_test_suite;

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

rglint_test_suite!("<rule-id>", root = "../../rules-fixtures");

#[test]
fn extra_unit_test() {
    // build LintEngine + Project + Siblings manually
    // lint + assert diagnostics
}
```

The `#[used]` force-link is required so the test binary includes the `#[derive(Rule)]` linkme registrations. Every rule test crate must have one.

### Schema-only extra unit test pattern

For schema rules, load the schema inline and construct empty documents manually:

```rust
use rglint_core::{
    LintEngine, LoadedDocuments, Project, ProjectConfig, RuleConfig, RulesConfig,
    SchemaLoader, SchemaSpec, Severity, Siblings,
};

let schema_loader = SchemaLoader::new();
let schema = schema_loader
    .load(&SchemaSpec::Inline("type Query { x: Int }".to_owned()), std::path::Path::new(""))
    .expect("schema loads");

let documents = LoadedDocuments {
    docs: Vec::new(),
    by_file: std::collections::HashMap::new(),
};

let siblings = Siblings::from_documents(&documents);
let project = Project { config: ProjectConfig { .. }, schema: Some(schema), documents, siblings };
let result = LintEngine::new(&RulesConfig { rules: vec![RuleConfig { id: "my-rule".to_owned(), severity: Severity::Error, options: serde_json::Value::Null }] })
    .expect("rule resolves")
    .lint(&project)
    .expect("lint runs");
```

## Fixture layout

Each rule gets a tree under `rules-fixtures/<rule-id>/`:

```
rules-fixtures/<rule-id>/
  manifest.json          # metadata (update valid_count / invalid_count / cases)
  valid/
    01/
      01.graphql         # source under lint (use .graphql or .gql extension)
      01.sibling.graphql # sibling document (for requires_siblings rules)
      01.config.toml     # sibling_documents = ["01.sibling.graphql"]
  invalid/
    01/
      01.graphql
      01.sibling.graphql
      01.config.toml
      01.expected.json   # { "errors": [{ "rule": "...", "message": "...", "line": 1, "column": 0 }] }
```

Source files must use `.graphql` or `.gql` extension — the harness looks for filenames ending with these suffixes. Avoid extensionless filenames (e.g. bare `graphql`), which won't be found.

For rules that inspect the file path (e.g. `match-document-filename` which checks operation name vs filename), the harness now always uses `DocumentSpec::Files` so `SourceFile::path()` preserves the real on-disk path. The file's stem and extension are available via `source_path.file_stem()` / `source_path.extension()`. Name fixture source files with meaningful names matching each case's expected messages.

Config fields: `schema`, `schema_path`, `kind` (operations/schema), `loose_message`, `[options]`, `sibling_documents`, `documents`.

### `kind = "schema"` for schema-category fixtures

Schema rules fire on SDL type definitions, not operation documents. Set `kind = "schema"` in the case's `config.toml`:

```toml
kind = "schema"
```

When `kind = "schema"`, the harness loads the `.graphql` source as the project's **schema** (via `SchemaSpec::Inline`) and loads **no** operation documents by default. The engine walks the schema sources and dispatches `on_node` for matching CST kinds (`FIELD_DEFINITION`, `INPUT_VALUE_DEFINITION`, etc.).

Omit the config entirely (or set `kind = "operations"`, the default) for operation-side fixtures (selection set duplicates, etc.).

### `documents` field for inline sibling operations

When `kind = "schema"`, set `documents = """..."""` to provide inline sibling operation documents:

```toml
kind = "schema"
documents = """
  { user(id: 1) { id } }
"""
```

The harness splits the content on blank lines, writes each segment to a temp file, and loads them via `DocumentSpec::Files`. This enables schema rules that also need `requires_siblings` (e.g. `no-unused-fields`) to check which schema fields are selected across sibling operations.

Config fields: `schema`, `schema_path`, `kind` (operations/schema), `loose_message`, `[options]`, `sibling_documents`, `documents`.

## `requires_schema` + `requires_siblings` combined (spec-035 pattern)

When a rule needs both the compiled schema AND sibling operations, set both attributes:

```rust
#[derive(Rule)]
#[rule(
    id = "no-unused-fields",
    category = "schema",
    requires_schema = true,
    requires_siblings = true,
    kinds = "FIELD_DEFINITION"
)]
pub struct SomeRule;
```

Subscribe to `kinds = "FIELD_DEFINITION"` (or another schema-only CST kind) so `on_node` fires only for schema source files. Use a flag to gate `finalize` — this prevents duplicate reporting when the engine calls `finalize` on every source file:

```rust
struct SomeHandler { is_schema_source: bool }

impl Handler for SomeHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        if node.kind == SyntaxKind::FIELD_DEFINITION {
            self.is_schema_source = true;
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        if !self.is_schema_source { return; }
        let Some(schema) = ctx.schema else { return; };
        let Some(siblings) = ctx.siblings else { return; };
        // Walk siblings.operations() selection sets, cross-reference against schema types
    }
}
```

The work is done in `finalize` by iterating `siblings.operations()` and walking each operation's typed `SelectionSet`. The `apollo_compiler::executable::SelectionSet` has a `ty: NamedType` field that tells you which type the selection is on, so type resolution is built in.

### Skipping introspection types in schema scans

When iterating `schema.types` to check fields, skip built-in introspection types (names starting with `__`):

```rust
for (type_name, ext_type) in &schema.types {
    if type_name.as_str().starts_with("__") { continue; }
    // check fields...
}
```

### Computing expected columns

`column` in `expected.json` is 0-based byte offset from the start of the line (graphql-eslint style, via `SourceFile::location_eslint`). Count bytes, not characters. For single-line sources, column = byte offset from file start.

## Branch / PR workflow

- Branch name: `spec-NNN`
- Commit message: `spec-NNN: <rule-id>` (matches existing pattern)
- Push, then `gh pr create --title "spec-NNN: <rule-id>" --body "<description>"`
- Merge via `gh pr merge --squash`
- `git checkout main && git pull`

## Build / test commands

- `cargo build` — check compilation
- `cargo test` — full suite
- `cargo test -p rglint-rules --test rule_<snake_case>` — single rule test
- `cargo clippy` — lint

## Key conventions

- Rule id is kebab-case (matches original graphql-eslint).
- Module names are snake_case (Rust convention).
- Diagnostic messages must be byte-identical to graphql-eslint's output (parity requirement).
- `Severity::Warn` is the default for all rules (matching graphql-eslint).
- `line` in expected.json is 1-based; `column` is 0-based byte offset (graphql-eslint style).
- Sibling rules: the first occurrence in iteration order is canonical (not reported); subsequent ones are reported.
