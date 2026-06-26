# Rust GraphQL Linter — Extensive Plan

A from-scratch Rust port of `graphql-eslint`, built on `apollo-parser` + `apollo-compiler`, with `graphql-eslint`'s rule set and test fixtures as the behavioral acceptance oracle.

---

## 1. High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         CLI (clap)                                   │
│   rglint <path...> --config .rglintrc.toml --format pretty|json|sarif │
└───────────────┬──────────────────────────────────────────────────────┘
                │
        ┌───────▼────────┐
        │  Config Loader │  → structs from .rglintrc.toml / .graphqlconfig
        └───────┬────────┘     (mirrors graphql-config + eslint config shape)
                │
        ┌───────▼────────┐
        │   Project      │  groups schema + documents per "project"
        │   Resolution   │  (mirrors packages/plugin/src/processor.ts)
        └───────┬────────┘
                │
   ┌────────────┴────────────┐
   ▼                         ▼
┌──────────┐          ┌─────────────────┐
│  Schema  │          │  Documents      │
│  Loader  │          │  (globs → files) │
└────┬─────┘          └────────┬────────┘
     │ apollo_compiler::Schema     │ Vec<Document>
     │ (incl. extensions)          │ (operations + fragments)
     ▼                             ▼
┌────────────────────────────────────────────┐
│            Lint Engine                     │
│  ┌────────────────────────────────────┐    │
│  │  Rule Registry                     │    │  HashMap<RuleId, Box<dyn Rule>>
│  │  (built-in + GraphQL-JS specs)     │    │  trait object dispatch
│  └────────────────────────────────────┘    │
│  ┌────────────────────────────────────┐    │
│  │  Visitor Pipeline                  │    │  Walk CST/AST once per document,
│  │  (apollo-compiler ASTVisitor)      │    │  multiplex to subscribed rules
│  └────────────────────────────────────┘    │
│  ┌────────────────────────────────────┐    │
│  │  Selector Engine                   │    │  eslint-style attribute selectors
│  │  `:matches(FieldDefinition)`,       │    │  compiled to predicate fn
│  │  `[name.value=/^Foo/]`, `:not(...)` │    │
│  └────────────────────────────────────┘    │
│  ┌────────────────────────────────────┐    │
│  │  Sibling Operations Index          │    │  cross-document fragment refs
│  │  (FragmentTracker)                │    │  (mirrors siblings.ts)
│  └────────────────────────────────────┘    │
│  ┌────────────────────────────────────┐    │
│  │  Diagnostics                        │    │  miette-based, syntactic spans
│  └────────────────────────────────────┘    │
└─────────────────────┬──────────────────────┘
                      ▼
              ┌───────────────┐
              │   Reporters   │  pretty (default), json, sarif, github-annotations
              └───────────────┘
```

### Architectural principles (mandates, not suggestions)

1. **One source of truth for AST.** `apollo_compiler::ExecutableDocument` for operations, `apollo_compiler::Schema` for the schema. No custom AST.
2. **Never lose source spans.** Lint diagnostics are 1:1 with graphql-eslint on line/column. Test fixtures assert exact offsets, so we always carry `SyntaxNode`/`NodeLocation` from apollo-parser up through rule code.
3. **Error-resilient.** apollo-parser yields a partial CST + collected errors — we route those as `ParseError` diagnostics, then continue linting the partial tree.
4. **Rule = data, not trait object dispatch overhead.** Rules are registered via `inventory` or a macro-built static array; the engine iterates rules and dispatches handlers per-AST-kind (mirrors eslint's `create()` returning a listener map).
5. **Spec validation is *not* a rule.** `apollo-compiler::validate` runs as a separate pre-pass; its output feeds rules (e.g. `GraphQLError`s surfaced as lint diagnostics) but it is not reimplemented.
6. **graphql-eslint fixtures are the oracle.** Every Rust rule has a `fixtures/` directory copied verbatim from `packages/plugin/src/rules/<rule>/index.test.ts`; an `insta`-driven harness asserts message + location parity.

---

## 2. Tooling & Crate Choices

| Concern | Crate | Rationale |
|---|---|---|
| Parser/CST | `apollo-parser` | Error-resilient CST with trivia; spec-compliant |
| Semantic AST | `apollo-compiler` | Builds `Schema` + `ExecutableDocument`, runs spec validation |
| Diagnostics | `miette` + `thiserror` | First-class source spans, multi-format renderers |
| Config | `serde` + `toml` + `jsonschema` for rule option validation (mirrors graphql-eslint's JSON-schema meta) |
| CLI | `clap` (derive) | Standard |
| Globbing | `globset` + `walkdir` + `ignore` (gitignore-aware) |
| Regex (rule options) | `regex` or `fancy-regex` if look-around needed (graphql-eslint uses some lookbehind — verify) |
| Parallel walk | `rayon` for per-file linting |
| Hashing for caching | `xxhash-rust` (mirror Hive) |
| Testing | `insta` (snapshot golden tests), `pretty_assertions`, `rstest` for parametric tests |
| Fixtures harness | custom, see §6 |
| WASM build (later phase) | `wasm-bindgen`, `getrandom` for browsers |
| SARIF output | `serde_json` + hand-rolled SARIF 2.1.0 schema (no mature Rust SARIF crate) |

Pin exact versions in `Cargo.toml`:

```toml
[dependencies]
apollo-parser = "0.8"
apollo-compiler = "1.32"
miette = { version = "7", features = ["fancy-no-backtrace"] }
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
jsonschema = { version = "0.18", default-features = false, features = ["draft202012"] }
regex = "1"
clap = { version = "4", features = ["derive", "cargo"] }
globset = "0.4"
walkdir = "2"
ignore = "0.4"
rayon = "1"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
```

---

## 3. Directory Structure

```
rglint/
├── Cargo.toml                      # workspace
├── Cargo.lock
├── rustfmt.toml
├── clippy.toml
├── deny.toml                       # cargo-deny: license + advisories
├── README.md
├── ARCHITECTURE.md
├── docs/
│   ├── rules/                      # generated markdown per rule
│   ├── contributing.md
│   └── design-records/             # ADRs
│
├── crates/
│   ├── rglint/                     # binary crate (CLI)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli.rs              # clap derives
│   │       ├── reporter/
│   │       │   ├── mod.rs
│   │       │   ├── pretty.rs
│   │       │   ├── json.rs
│   │       │   ├── sarif.rs
│   │       │   └── github.rs
│   │       └── exit.rs
│   │
│   ├── rglint-core/                # engine (no I/O)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs           # LintEngine: orchestrates everything
│   │       ├── context.rs          # RuleContext (mirrors GraphQLESLintRuleContext)
│   │       ├── source.rs           # SourceFile abstraction (path, content, line table)
│   │       ├── schema.rs           # Schema loading + cache (mirrors schema.ts)
│   │       ├── documents.rs        # Document loading + dedup (mirrors documents.ts)
│   │       ├── siblings.rs         # FragmentTracker (siblings.ts)
│   │       ├── project.rs          # GraphqlConfigProject (graphql-config.ts)
│   │       ├── cache.rs            # content-hash cache, mirrors cache.ts
│   │       ├── diagnostics.rs      # Diagnostic, Severity, Suggestion, Fix
│   │       ├── location.rs         # Span/Location types; line+col from CST
│   │       ├── node_name.rs        # getNodeName helper (utils.ts)
│   │       ├── utils.rs            # shared helpers (ARRAY_DEFAULT_OPTIONS etc.)
│   │       │
│   │       ├── selector/
│   │       │   ├── mod.rs
│   │       │   ├── lexer.rs        # selector "language" lexer
│   │       │   ├── parser.rs      # esquery-like: matches, attr, :not, descendant
│   │       │   ├── ast.rs
│   │       │   ├── matcher.rs
│   │       │   └── tests.rs
│   │       │
│   │       └── test_harness/       # NOT in lib — feature-gated
│   │           ├── mod.rs
│   │           ├── fixture.rs     # parse graphql-eslint-style test cases
│   │           └── oracle.rs       # compare to expected diagnostics
│   │
│   ├── rglint-rules/               # all 36+ policy rules
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # pub fn all_rules() -> Vec<RuleEntry>
│   │       ├── meta.rs             # RuleMeta { id, docs, schema, category, default_options }
│   │       ├── schema/
│   │       │   ├── mod.rs
│   │       │   ├── alphabetize.rs
│   │       │   ├── description_style.rs
│   │       │   ├── input_name.rs
│   │       │   ├── naming_convention.rs
│   │       │   ├── no_deprecated.rs
│   │       │   ├── no_duplicate_fields.rs
│   │       │   ├── no_hashtag_description.rs
│   │       │   ├── no_one_place_fragments.rs
│   │       │   ├── no_root_type.rs
│   │       │   ├── no_scalar_result_type_on_mutation.rs
│   │       │   ├── no_typename_prefix.rs
│   │       │   ├── no_unreachable_types.rs
│   │       │   ├── no_unused_fields.rs
│   │       │   ├── relay_arguments.rs
│   │       │   ├── relay_connection_types.rs
│   │       │   ├── relay_edge_types.rs
│   │       │   ├── relay_page_info.rs
│   │       │   ├── require_deprecation_date.rs
│   │       │   ├── require_deprecation_reason.rs
│   │       │   ├── require_description.rs
│   │       │   ├── require_field_of_type_query_in_mutation_result.rs
│   │       │   ├── require_nullable_fields_with_oneof.rs
│   │       │   ├── require_nullable_result_in_root.rs
│   │       │   ├── require_type_pattern_with_oneof.rs
│   │       │   ├── require_import_fragment.rs
│   │       │   ├── require_selections.rs
│   │       │   ├── strict_id_in_types.rs
│   │       │   ├── unique_enum_value_names.rs
│   │       │   └── lone_executable_definition.rs
│   │       │
│   │       ├── operations/
│   │       │   ├── mod.rs
│   │       │   ├── no_anonymous_operations.rs
│   │       │   ├── match_document_filename.rs
│   │       │   ├── selection_set_depth.rs
│   │       │   ├── unique_fragment_name.rs
│   │       │   └── unique_operation_name.rs
│   │       │
│   │       └── shared/
│   │           ├── mod.rs
│   │           ├── case.rs             # convertCase helper
│   │           ├── case_styles.rs       # camelCase/PascalCase/...
│   │           ├── relay.rs            # shared Relay predicates
│   │           ├── oneof.rs             # @oneOf directive helpers
│   │           └── deprecated.rs
│   │
│   ├── rglint-graphql-spec/        # thin wrapper around apollo-compiler::validate
│   │   └── src/
│   │       ├── lib.rs              # exposes the ~28 graphql-js spec rules as rglint Rule entries
│   │       ├── spec_rules.rs      # bridges apollo-compiler ValidationErrors to RuleContext.report
│   │       └── names.rs           # rule id mapping (e.g. FieldsOnCorrectType → "fields-on-correct-type")
│   │
│   ├── rglint-config/             # config schema + load (mirrors graphql-config)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs           # serde structs for .rglintrc
│   │       ├── graphql_config.rs   # parse .graphqlrc / graphql-config
│   │       └── validate.rs          # JSON-schema-validate rule options
│   │
│   ├── rglint-derive/             # proc-macros
│   │   └── src/
│   │       ├── lib.rs
│   │       └── rule_derive.rs      # #[derive(Rule)] to auto-register
│   │
│   └── rglint-test-harness/       # fixture loader
│       └── src/
│           ├── lib.rs
│           ├── fixture.rs          # parse JS-style fixture cases from input .txt
│           ├── expected.rs        # expected errors (message + line:col)
│           └── runner.rs
│
├── rules-fixtures/                 # mirror of graphql-eslint per-rule test cases
│   └── <rule-id>/
│       ├── valid/
│       │   ├── 01.graphql
│       │   └── 01.toml             # options, schema
│       └── invalid/
│           ├── 01.graphql
│           ├── 01.toml             # expected: errors
│           └── 01.expected.json
│
├── benches/
│   ├── parser.rs                   # criterion: parse vs apollo-parser baseline
│   ├── linters.rs                  # full document lint throughput
│   └── corpora/                    # real-world schemas (GitHub, Shopify, etc.)
│
├── tests/
│   ├── integration/
│   │   ├── cli.rs
│   │   ├── config.rs
│   │   ├── multi_project.rs       # mirrors multiple-projects-graphql-config example
│   │   └── parity/                # end-to-end parity against graphql-eslint snapshots
│   │       └── *.md                # snapshot.md from packages/plugin/src/rules/*/snapshot.md
│   │
│   └── conformance/
│       └── graphql-js/             # vendored graphql-js language tests
│
└── xtask/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── port_fixture.rs        # converts TS test cases to .toml/.expected.json
        ├── gen_docs.rs            # generates docs/rules/*.md from rule meta
        └── check_parity.rs        # runs both TS and Rust linters, diff outputs
```

---

## 4. Core Abstractions

### 4.1 `Rule` trait

Mirrors graphql-eslint's `{ meta, create(context) }` shape, dispatched by an engine that walks the AST once per document and reroutes to subscribed handler closures.

```rust
pub trait Rule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn create<'s>(&'s self, ctx: &'s mut RuleContext) -> Box<dyn Handler + 's>;
}

pub trait Handler {
    /// Called for each AST node; the handler decides whether it matches.
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        let _ = (node, parent);
    }
    /// Called after the walk for document-global checks.
    fn finalize(&mut self, _ctx: &mut RuleContext) {}
}

pub struct RuleMeta {
    pub id: &'static str,
    pub category: Category, // Schema | Operations | Other
    pub severity: Severity,
    pub docs: &'static str,
    pub option_schema: Option<jsonschema::Validator>, // shared, immutable
    pub default_options: Option<serde_json::Value>,
    pub requires_schema: bool,
    pub requires_siblings: bool,
    pub deprecated: bool,
    pub replaced_by: Option<&'static str>,
    pub has_suggestions: bool,
}
```

The engine subscribes each rule to a list of `SyntaxKind`s the rule declares in its meta (we cache this so unwanted rules don't get walked at all).

### 4.2 `RuleContext`

```rust
pub struct RuleContext<'a> {
    pub file: &'a SourceFile,
    pub schema: Option<&'a Schema>,
    pub siblings: Option<&'a Siblings>,
    pub options: &'a serde_json::Value,
    pub project: &'a ProjectConfig,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> RuleContext<'a> {
    pub fn report(&mut self, d: DiagnosticBuilder);
    pub fn source_code(&self) -> &SourceFile;
    pub fn require_schema(&self, rule_id: &str) -> Result<&Schema>;
    pub fn require_operations(&self, rule_id: &str) -> Result<&Siblings>;
}
```

### 4.3 Selector engine

graphql-eslint uses esquery-style selectors like
`ObjectTypeDefinition > FieldDefinition[name.value=/^_/]` and `:matches(...)`,
`:not(...)`. Ported as:

- **Lexer** over the selector string → tokenizes `:matches`, `[k=v]`, `[k=~regex]`, `:not(...)`, `>`, descendant, kind names.
- **Parser** → `SelectorNode` tree with `Child`, `Descendant`, `Attribute`, `Matches`, `Not`.
- **Matcher** → `Matcher: Fn(&Node, Option<&Node>) -> bool`, compiled once at rule registration. Walks the AST once per selector and selects nodes.

`name.value` is a special attribute path; we model attributes as a small enum:
```
AttrKind::NameValue, AttrKind::Kind, AttrKind::DescriptionValue,
AttrKind::ValueRaw (literal attributes like [name.value=PageInfo])
```

### 4.4 Sibling operations index

Mirrors `packages/plugin/src/siblings.ts`:

```rust
pub struct Siblings {
    operations: Vec<ExecutableDocument>,
    fragment_index: AHashMap<String, FragmentDefinition>,
    document_by_file: AHashMap<PathBuf, usize>,
}

impl Siblings {
    pub fn get_fragments_in_use(&self, op: &ExecutableDocument) -> Vec<&FragmentDefinition>;
    pub fn get_operation_by_name(&self, name: &str) -> Option<&ExecutableDocument>;
}
```

### 4.5 Diagnostics

```rust
pub struct Diagnostic {
    pub rule_id: String,
    pub file: PathBuf,
    pub span: Span,            // byte offset + length, with line/col computed lazily
    pub message: String,
    pub severity: Severity,
    pub suggestions: Vec<Suggestion>,
    pub data: serde_json::Value,
}

pub struct Suggestion {
    pub desc: String,
    pub fix: Fix, // replace(range, text) | insert(text) | remove(range)
}
```

`miette` renders graph + source snippet for the `pretty` reporter.

---

## 5. Rule Port Plan — Order and Effort

Rules ordered by dependency on shared infrastructure (port shared helpers first so subsequent rules drop in). Each rule ≈ 0.5–2 days of porting including fixture extraction.

### Phase 0 (foundation) — ~5 days
0. Project skeleton, workspace, `cargo deny`, `xtask port-fixture`.
1. `rglint-core`: SourceFile, location, schema/document loaders.
2. Selector engine.
3. Test harness: fixture loader, expected-error comparator.
4. Tooling: `xtask port-fixture` reads `packages/plugin/src/rules/<rule>/index.test.ts`, extracts `{ valid: ... , invalid: ... with errors }` to `rules-fixtures/<rule-id>/`. (Not perfectly; manual cleanup expected.)

### Phase 1 (leaf rules, no shared deps) — ~3 days
Easy, self-contained, validates the skeleton:

- `no-anonymous-operations`
- `unique-fragment-name`
- `unique-operation-name`
- `no-duplicate-fields`
- `lone-executable-definition`
- `alphabetize` (involves comparison of strings + AST walking; good exercise)

### Phase 2 (schema-only, no cross-doc) — ~5 days
- `description-style`
- `no-hashtag-description` (needs CST trivia — apollo-parser trivia support verified here)
- `require-description`
- `require-deprecation-reason`
- `require-deprecation-date`
- `naming-convention` (port `case.rs` once here, biggest single rule ~579 lines)
- `unique-enum-value-names`
- `strict-id-in-types`
- `no-typename-prefix`
- `no-root-type`
- `match-document-filename` (file-name based, no AST)

### Phase 3 (schema-aware operations) — ~4 days
- `no-deprecated` (uses Schema's deprecated annotations)
- `no-unused-fields` (schema + operations cross-check)
- `no-unreachable-types` (graph traversal over schema)
- `no-scalar-result-type-on-mutation`
- `require-nullable-result-in-root`
- `require-field-of-type-query-in-mutation-result`

### Phase 4 (siblings + cross-document) — ~3 days
- `selection-set-depth` (needs sibling fragments in use; reimplement `graphql-depth-limit` in Rust, ~80 LOC)
- `require-import-fragment`
- `require-selections`
- `no-one-place-fragments`

### Phase 5 (Relay suite) — ~3 days
Port `shared/relay.rs` first (predicate functions for `Connection`/`Edge`/`PageInfo`), then:
- `relay-arguments`
- `relay-connection-types`
- `relay-edge-types`
- `relay-page-info`

### Phase 6 (oneOf + remaining) — ~2 days
- `require-nullable-fields-with-oneof`
- `require-type-pattern-with-oneof`
- `input-name`

### Phase 7 (spec rules) — ~2 days
- `rglint-graphql-spec` bridge. Apollo-compiler already validates spec rules. Map each of the ~28 ID-able rules (see `graphql-js-validation.ts:14`–`46`) to:
  ```
  rule_id: "fields-on-correct-type"   → apollo validation error .code() == "FieldsOnCorrectType"
  ```
  Each becomes a thin adapter wrapping `apollo_compiler::validate` output.

### Phase 8 (config + CLI polish) — ~4 days
- `.rglintrc` loader; default-recommended config preset (mirror `configs/*.ts`).
- Formatters: pretty (miette), JSON, SARIF, GitHub annotations.
- `--fix` mode applying suggestions.
- Config schema JSON-schema validation.
- `.graphqlrc.yaml` interoperability.

### Phase 9 (performance + packaging) — ~3 days
- Rayon parallelisation per file (engine is `Send + Sync`).
- Content-hash cache for incremental runs.
- Benchmarks against `benches/corpora`.
- Release binary build, `.tar.gz`, installers via `cargo-binstall`.

### Phase 10 (napi bridge for JS interop) — ~1 week (optional)
- `napi-rs` wrapper so `@graphql-eslint/eslint-plugin` *or* the wider eslint ecosystem could consume Rust rules from Node. Mirrors apollo-rs's `examples/validation-wasm-demo`. This is the only path for adoption; otherwise the linter is standalone CLI.

**Total rough estimate: ~6–8 weeks for one focused engineer with LLM assist.**

---

## 6. Testing Strategy

### 6.1 The "oracle parity" approach (core idea)

`graphql-eslint` ships ~500 fixture cases across 36 rules, each with explicit `{ code, schema, options, errors:[{message, line, column}] }`. These define the **observable behavior** of every rule. Treat them as the spec.

Each fixture is transformed by `xtask port-fixture` into:

```
rules-fixtures/<rule-id>/invalid/01.graphql      # source
rules-fixtures/<rule-id>/invalid/01.config.toml  # options + schema
rules-fixtures/<rule-id>/invalid/01.expected.json # exact expected diagnostics
```

Format of `01.config.toml`:

```toml
schema = """
type Query { user: User }
type User { id: ID! name: String @deprecated(reason: "old") }
"""
options = { maxDepth = 7 }
```

Format of `01.expected.json`:

```json
{
  "errors": [
    { "rule": "no-deprecated",
      "message": "Field \"name\" is marked as deprecated in your GraphQL schema (reason: old)",
      "line": 2,
      "column": 9 }
  ]
}
```

### 6.2 Test layers

| Layer | Crate | Purpose | Count |
|---|---|---|---|
| Unit | each crate `#[cfg(test)]` mod | Per-function correctness | hundreds |
| Fixture | `rglint-test-harness` driven from `rules-fixtures/` | Rule parity vs graphql-eslint | ~500 cases |
| Snapshot | `insta` golden `.snap` per rule | Pin diagnostic formatting output | 36 |
| Integration | `tests/integration/` | CLI end-to-end + multi-project configs | dozens |
| Conformance | `tests/conformance/graphql-js/` vendored from graphql-js | Parser spec correctness | skip if apollo-compiler handles |
| Property | `proptest` | Round-trip: parse → format → parse; rule idempotence | few per rule |
| Performance | `benches/` (criterion) | Detect ≥10% regressions vs pinned baseline | per-rule + per-doc |
| Parity harness | `xtask check-parity` | Run Rust linter + `pnpm test` from this repo, diff JSON outputs | end-to-end |

### 6.3 Specific parity rules we enforce

Each fixture must report **identical**:
- Number of errors
- Error messages (verbatim)
- Rule IDs
- Line/column coordinates (note: graphql-eslint uses 1-based line, 0-based column on `loc.column`; verify off-by-one when going through apollo-parser which is 1-based on both — add a normalization layer in `location.rs`)
- Suggestions count + description text

We relax **byte-exact** offsets (frankly graphql-eslint's token→column math is fiddly); we assert line + column equivalence only.

### 6.4 Snapshot files (`packages/plugin/src/rules/*/snapshot.md`)

These exist in this repo. `xtask check-parity` runs the existing `pnpm test` and writes a regenerated `.snap` we then diff against `rglint`'s format output. This catches drift across the whole rule list in one shot.

### 6.5 Negative-path coverage

For every rule, deliberately craft at least one malformed input to verify the error-resilient parser path produces *some* useful diagnostics rather than panicking or silently passing.

### 6.6 Cross-cutting invariant tests

- "Disabling rule X via config suppresses all its diagnostics."
- "Parse errors yield `parse-error` diagnostics and abort rule execution."
- "Running on a workspace with N schemas produces N independent lint passes."

### 6.7 Coverage gate

CI runs `cargo tarpaulin --workspace` with a 85% line coverage floor; rule modules must hit ≥90% (most rule code is short linear logic).

---

## 7. Configuration Format

`.rglintrc` (JSON or TOML). Example:

```json
{
  "projects": {
    "web": { "schema": "schema/*.graphql", "documents": "src/**/*.{ts,graphql}" }
  },
  "rules": {
    "naming-convention": ["error", {
      "types": "PascalCase",
      "FieldDefinition": { "style": "camelCase", "forbiddenPrefixes": ["__"] }
    }],
    "require-description": ["error", { "types": true, "FieldDefinition": true }],
    "selection-set-depth": ["error", { "maxDepth": 7 }],
    "no-deprecated": "error"
  },
  "ignore": ["**/generated/**"],
  "format": "pretty"
}
```

`severity: "off" | "warn" | "error"` mirrors eslint. Rule *options schema* comes from `RuleMeta::option_schema` and is JSON-schema-validated at load time.

---

## 8. Implementation Risks & Mitigations

| Risk | Mitigation |
|---|---|
| apollo-parser CST doesn't expose comments as addressable nodes (needed by `no-hashtag-description`) | Phase 1 spike — write a throwaway test before committing. If blocked, fall back to its `tokenizer.rs`-style raw token iteration over `cst.errors()`-companion tokens; or pin a version where trivia is exposed. Worst case, hand-roll a small comment-scanner in `rglint-core` called only by this rule. |
| Selector engine is more complex than expected (graphql-eslint uses esquery with many features) | Phase 0 spike. Start with `:matches`, `:not`, attribute `[kind=...]`, descendant + child combinators only. Defer general esquery parity to when a rule actually uses a missing feature. |
| `naming-convention` is 579 LOC with extensive option combinations | Port helpers (`convertCase`, regex groups) up front as `shared/case.rs`; port rule last in Phase 2. Add a property test that any `naming-convention` config from `rules-fixtures/naming-convention/` produces byte-identical messages. |
| `graphql-depth-limit` reimplementation | ~80 LOC; mirror its `visit()`-based depth counter. Verify it handles fragments (cyclic + shared) — `selection-set-depth` already uses `siblings.getFragmentsInUse`. |
| off-by-one in line/column matching graphql-eslint | Centralized `location.rs` normalizes; covered by the parity tests; the test harness is set up in Phase 0 before any rules are ported, so regressions surface in CI immediately. |
| JSON-schema option validation differs from TS-typed defaults | `xtask port-fixture` also extracts default values from `meta.docs.configOptions`. Run rule with default options when fixture options missing; assert match. |
| apollo-compiler validation errors have slightly different messages than graphql-js | For spec rules, we *accept* message variance and document it; compare on rule-id + location, not message text. Capture variances in `tests/conformance/graphql-js/known-divergences.md`. |
| Performance regression hidden by complexity of N rules over M docs | Bench per-rule; CI fails if >10% slowdown vs the last green commit. |

---

## 9. CI Pipeline

GitHub Actions matrix (Linux/mac/Windows):

1. `cargo fmt --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo deny check`
4. `cargo test --workspace` — unit + fixture + snapshot
5. `cargo tarpaulin --workspace` — coverage gate
6. `cargo bench --no-run` (compile only in CI)
7. `xtask check-parity` — install `pnpm`, run graphql-eslint tests, diff Rust output
8. `xtask gen-docs --check` — README rule table generated from `RuleMeta`

---

## 10. Release & Distribution

- Semver; 0.1.0 milestone = Phase 1–3 (schema + leaf rules + Relay).
- `cargo-binstall` manifest on releases.
- Homebrew tap (post-1.0).
- npm distribution via `napi-rs` triplet packages (post-1.0).
- SARIF output for GitHub code scanning upload baked in.

---

## 11. Stretch Goals (nice-to-have, not mandated)

- **VSCode extension** consuming JSON output via LSP diagnostic protocol.
- **`--fix` rewrites in place** for all `hasSuggestions: true` rules.
- **Lint GraphQL embedded in `.ts`/`.vue`/`.svelte`** — port of the *processors* (`packages/plugin/src/processor.ts` and the `examples/*-code-file`) using `tree-sitter-embedded-graphql`.
- **Lint Apollo federation directives** as a separate `rglint-apollo-*` crate.
- **Pub-schema registry pull** (`graphql-config`'s `schema: http://…/schema`).
- **WASM build** for browser-based playground.

---

## 12. Decision summary (for stakeholders)

| Question | Answer | Why |
|---|---|---|
| Roll own parser modeled on graphql-js? | **No** | graphql-js has no error resilience or CST trivia; we'd pay months for a worse apollo-parser. |
| Use apollo-parser + apollo-compiler? | **Yes** | Spec-tested, error-resilient, CST + trivia, with spec validation already implemented. |
| Use graphql-eslint rules as oracle? | **Yes** | Its ~500 fixtures are the spec for what each policy rule should fire on; better than rolling fresh ones. |
| Use Hive's `graphql-tools`? | **No** (for now) | AST shaped for router planning; less diagnostic-oriented; coupled to Hive's repo. Worth reconsidering if we ever build a router. |
| Cargo workspace vs monolith? | **Workspace** | Lets rules, core, CLI, and harness release independently; smaller compiles during dev. |
| Selector engine from scratch? | **Yes**, scoped subset | Avoids pulling `esquery` (Rust) which doesn't understand GraphQL kind names; we customize the AST attr model. |
| Napi bridge to JS users? | **Phase 10 stretch** | Significant adoption unlock; do not block Phase 1–8 on it. |

---

This plan is intentionally opinionated and exists to be wrong in discoverable places — the Phase 0 spikes (trivia support, selector engine basics, parity harness) are the first three things to execute, and whatever they reveal should drive a revision of §4 and §5 before any rules are ported in earnest.

Estimated end-to-end: **6–8 weeks** to a 1.0 with all 36 rules at parity, with the spec-rule set for free from apollo-compiler.