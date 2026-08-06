# AGENTS.md — conventions for implementing specs

## Repository and branch hygiene

Before touching a spec, run `git status -sb` and confirm the checkout is clean.
If it is not clean, preserve the existing work and ask which files belong in
the spec change; never hide, reset, or overwrite unrelated edits. The required
branch sequence is:

1. `git switch main`.
2. `git pull --ff-only origin main`.
3. Confirm `main` is up to date and create `spec-NNN` from that commit.

If the pull is not a fast-forward, stop and resolve the repository state before
creating the implementation branch. Keep the branch focused on one spec and
do not mix generated files, nested worktrees, or unrelated cleanup into it.

If `spec-NNN` already exists locally or on the remote, inspect it before
reusing it: verify that it is based on the current `main`, that its worktree
is clean, and that its diff contains only that spec. Never delete or force-
rewrite an existing implementation branch to make the naming sequence fit;
stop and ask for direction if it contains unrelated or unreviewed work.

For this check, inspect both namespaces before branching:
`git branch --list spec-NNN` and `git ls-remote --heads origin spec-NNN`.
If the remote cannot be reached, do not infer that the branch is absent from a
failed lookup; verify connectivity or stop before creating a potentially
colliding branch.

## GitHub CLI PR and merge workflow

Use the GitHub CLI for the complete handoff once implementation and local
validation are complete:

1. Confirm the CLI identity and repository before publishing:
   `gh auth status` and `gh repo view --json nameWithOwner,defaultBranchRef`.
2. Push only the focused implementation branch with
   `git push --set-upstream origin spec-NNN`.
3. Create the PR with `gh pr create --base main --head spec-NNN`, using a
   body that names the spec/rule, pinned upstream parity revision, focused
   test, full validation commands, and any deliberate scope difference.
4. Inspect the created PR with `gh pr view --json number,url,state,headRefName,baseRefName`
   and wait for required checks with `gh pr checks --watch`. If the repository
   has no checks, record that fact in the handoff; never treat an unobserved
   check state as a passing result.
5. Merge only the reviewed PR with `gh pr merge --squash` after checks pass.
   Do not use `--admin`, force-push, or bypass branch protection unless the
   user explicitly authorizes it. Verify the result with
   `gh pr view --json state,mergedAt,mergeCommit`.
6. Refresh the checkout after the merge using `git switch main` followed by
   `git pull --ff-only origin main`, then run `git status -sb`. Report any
   remaining changes; do not delete or hide them to manufacture a clean state.

When a PR is required, do not stop after pushing a branch: the PR URL, check
result, merge result, and refreshed `main` status are all part of completion.

## Spec lifecycle

1. Next unimplemented spec = lowest-numbered `spec-NNN.md` still in `specs/` (not in `specs/implemented/`). Check `specs/README.md` for the status index.
2. Before creating the implementation branch, switch to `main` and fast-forward it from `origin/main`; do not branch from a stale local default branch.
3. Read the spec, understand dependencies (they state which prior specs must be done).
4. Check `rules-fixtures/<rule-id>/` — if fixtures already exist, they are the ground truth. The spec text may be stale or simplified; always match the fixture `expected.json` messages and the original graphql-eslint source linked in `manifest.json`. If a fixture has placeholder messages like `"<unknown>"`, verify the upstream graphql-eslint snapshot/source and replace the placeholders with exact messages before treating the fixture as parity-complete. New fixtures must use the canonical `NN.graphql`/`NN.gql` names; the harness still accepts legacy extensionless names for compatibility, so do not use that compatibility path for new work.
   - When upstream injects helper schema through parser options, keep local fixtures self-contained by appending helper SDL after the upstream snippet whenever possible. That preserves upstream line/column offsets for diagnostics that point inside the snippet.
5. Implement per the spec's Deliverables (amended by fixture reality if applicable).
6. Move `specs/spec-NNN.md` → `specs/implemented/spec-NNN.md`.
7. Update `specs/README.md`: fix the link path (add `implemented/` prefix) and change status to `[x]`.
8. Run formatting, build, clippy, and tests before committing. Start with
   `cargo fmt --check`; if the clean baseline is not formatter-clean, run
   `rustfmt --check` only on touched Rust files and record that baseline
   limitation, then run `cargo build`, `cargo clippy`, and `cargo test` as
   separate commands so the first failure is unambiguous.

### Benchmark and baseline hygiene

- Put Criterion targets in the owning package and declare external benchmark
  paths explicitly when the repository-level `benches/` layout is part of the
  public workflow. Keep benchmark identifiers stable as `group/name`; changing
  an identifier invalidates the corresponding baseline and must be called out
  in the spec/PR.
- Benchmark the behavior the spec claims: move corpus loading, project
  resolution, and engine construction into Criterion setup when measuring lint
  throughput, and create a fresh cache or explicitly document cache-hit
  measurement. Do not let a persistent content-hash cache turn a full-lint
  benchmark into a cache benchmark accidentally.
- Treat committed performance baselines as deliberate pins, not generated
  build output. Regenerate them only with the repository's comparison script,
  review the complete key set, and record the command, host class, and reason
  for re-pinning in the PR. CI compile-only checks must use `cargo bench
  --no-run`; regression comparisons belong on dedicated or scheduled runners
  because shared CI timing is noisy.
- Every checked-in benchmark corpus needs a provenance/license note and must be
  immutable input during a run. Prefer small, representative, license-clean
  corpora over network downloads so benchmark results are reproducible offline.

### Configuration loader conventions

- Keep the file-facing serde model separate from the normalized engine model.
  JSON/TOML spelling and rule tuples belong in `rglint-config::schema`; the
  normalized `Config` owns named projects, resolved `Severity` values, and raw
  JSON options. Do not make the core engine parse config-file syntax.
- Preserve relative schema/document paths until `ProjectResolver` receives the
  config file's directory. Config loading must not accidentally resolve paths
  against the process CWD, which breaks nested project configurations.
- Discovery is nearest-directory first, with the documented filename
  precedence applied within each directory. Add a test whenever a config name
  or precedence rule changes, including the no-config case.
- Normalize top-level `ignore` before project-local ignores and synthesize the
  `default` project only when there is no explicit `projects` map. Unknown rule
  ids remain loadable for forward compatibility; malformed severities and
  tuple shapes must fail with a path and source location.
- Keep rule options as unvalidated JSON until spec-056. Config tests must cover
  both scalar severity (`"error"` becomes `{}` options) and tuple severity with
  camelCase options, plus JSON/TOML round trips.
- A focused config change must run `cargo test -p rglint-config` before the
  workspace checks. If `cargo fmt --check` fails on untouched baseline files,
  report that limitation and use `rustfmt --check` on the touched config files.

### Configuration validation and registry boundaries

- Before introducing a config error type, inspect the existing public error
  enum and preserve its variants for callers from earlier specs. Add a
  focused variant or payload for the new failure rather than replacing a
  stable error surface to match a draft interface literally.
- Keep option validation in `rglint-config` independent of the built-in rule
  crate: callers pass the `&RuleEntry` registry explicitly. Validate only
  known, non-`off` rules, skip unknown ids for forward compatibility, and
  return all option failures in one error so callers do not need a retry loop.
- Apply `RuleMeta::default_options` to a cloned option value with a shallow,
  user-wins merge before validation. Do not silently rewrite the normalized
  config unless the API explicitly promises persistence of defaults.
- JSON-Schema option paths in diagnostics should be JSON Pointers to the
  offending instance (escaping `~` as `~0` and `/` as `~1`), and the validator
  draft must be explicit when the dependency supports multiple drafts.
- If a later CLI or integration spec owns the call site, record that boundary
  in the current spec and add the call only when that entry point exists; do
  not add a dependency cycle merely to make an unimplemented caller compile.

### Built-in preset and registry parity

- Treat pinned upstream config snapshots as the authority for built-in preset
  rule ids, severities, and option objects. Keep the revision and the exact
  source paths in the owning spec, and test the complete normalized maps rather
  than only checking that a few representative rules are present.
- Presets may combine entries from multiple rule crates. Any CLI or integration
  entry point that consumes a preset must force-link and validate the complete
  registry, including `rglint-graphql-spec`; do not silently drop unknown preset
  entries or make a preset appear usable only in config-loader unit tests.
- Keep preset inheritance and user overrides separate: resolve built-in parent
  presets first, then apply user rules/options. Preserve user project paths and
  ignores, deduplicate merged ignores deterministically, and reject unknown
  preset names or inheritance cycles with actionable config errors.
- When a spec owns the default preset, update `--init` in the same focused
  change and test that the generated config contains the live preset reference.

### GraphQL config interoperability

- Keep `.graphqlrc`/`.graphqlconfig` parsing in `rglint-config`; the core
  engine consumes only the normalized `Config`/`ProjectConfig` model.
- Support the common filename aliases with nearest-directory discovery and a
  deterministic within-directory precedence. Treat top-level project maps as
  named projects, preserve relative paths, and keep the rules map empty so the
  default preset can be applied by its owning spec.
- Reject remote schema URLs during config loading with a dedicated error that
  includes the config path, project, and URL. Do not let an HTTP URL fall
  through as a local glob and produce a misleading no-match error.
- When the core resolver does not yet apply GraphQL config filters, record the
  conversion boundary explicitly in the spec and tests: `exclude` is retained
  as project ignore data and `include` can supply documents when `documents`
  is absent; when both are present, preserve the explicit `documents` value
  and document that the resolver still lacks include filtering. Do not
  silently broaden a filter.
- Add fixture-backed tests for multi-project YAML, legacy JSON, discovery
  precedence, extension-key tolerance, and the remote-schema failure. Keep
  the fixture paths relative so tests also verify config-directory resolution
  remains the resolver's responsibility.

### Upstream parity and handoff checklist

- **Spec correction protocol.** Treat the pinned upstream source and tests as
  the behavioral authority when they disagree with a draft spec. Before
  coding, update the spec's Goal/Scope/Behavior sections to describe the
  verified behavior, preserve the immutable revision and source/test links,
  and call out any deliberate scope difference. Keep the fixture manifest's
  `source`, `upstream_revision`, case list, and counts aligned with that
  corrected contract. The PR body must summarize the correction so reviewers
  do not have to infer why the implementation differs from the original
  wording.

- Inspect the upstream graphql-eslint rule **and its tests** before implementing
  a port. Copy exact diagnostic wording, report-node location, rule metadata
  (`requires_schema`, `requires_siblings`, `has_suggestions`), and suggestion
  behavior into the Rust design; if a capability is not supported by the
  current engine, document the deliberate scope difference in the spec/PR.
- Record the upstream source and test revision or URL used for the parity
  decision. When the spec is stale, update the spec to the verified upstream
  behavior rather than implementing an unconfirmed option or message.
- Pin the parity record to an immutable upstream commit, tag, or snapshot date;
  a `master`/`main` URL alone is not sufficient because rule wording and tests
  can change after implementation.
- When the upstream repository is the parity authority, fetch the pinned
  source and tests through `gh api` (or an equivalent immutable raw endpoint)
  when possible, and record the same revision in the fixture manifest. If the
  pinned revision cannot be reached, stop before implementing guessed
  diagnostics; do not silently substitute a moving branch or leave fixture
  messages as placeholders.
- When porting shared helpers, prefer the engine's canonical semantic model
  over a source-language AST type that cannot represent merged schema
  extensions. Keep the public Rust API borrow-based, document the adaptation in
  the spec, and add tests for both the default options and at least one custom
  option set.
- Add at least one fixture for every meaningful branch in the upstream tests,
  including valid cases that exercise accepted syntax. Keep fixture manifests,
  valid/invalid counts, and case lists synchronized; run the focused rule test
  before the workspace-wide checks.
- Before staging, run `git diff --check` and inspect `git status --short` plus
  the complete diff. Stage only files belonging to the spec, including the
  spec move and this index update; do not silently include generated artifacts,
  nested worktrees, or unrelated edits.

### Fixture validation checklist

- Use the full suffixes `NN.graphql`/`NN.gql`, `NN.config.toml`, and `NN.expected.json`. Extensionless legacy names remain readable for old fixtures, but are not an acceptable format for new or modified cases.
- Do not leave a placeholder manifest with zero cases for an implemented rule:
  every manifest case must have a matching directory, and its counts must
  equal the discovered valid/invalid directories.
- Keep `manifest.json` reviewable: record the immutable upstream revision used
  for parity, list every case, and update counts in the same change as fixture
  additions or removals. Do not use `loose_message` to avoid resolving a
  message mismatch.
- A rule declaring `requires_schema = true` must have `schema = ...` or `schema_path = ...` in every operation fixture, or use `kind = "schema"` for schema fixtures. Otherwise the engine intentionally skips the rule and a passing fixture can be false confidence.
- Schema fixtures that use custom directives must declare those directives in the
  fixture SDL, including every directive location exercised by the case. The
  schema loader validates the complete SDL before the rule runs, so relying on
  an unknown-directive parser fallback can turn an intended diagnostic into a
  schema-load failure (or make a valid case appear to pass without dispatch).
- For operation fixtures, put fragments in the main source or declare them through `sibling_documents`. The `documents = ...` helper is only consumed for `kind = "schema"` fixtures.
- Give sibling files stable, case-specific names when a diagnostic includes a
  path; expected messages must use the same rendered path as the upstream
  rule, not an absolute temporary path.
- Run the applicable formatting check and the new rule's focused parity test
  before the workspace checks so formatting, fixture, message, and location
  mismatches are isolated from unrelated failures.

### Fixture source ownership and false-confidence guard

- Schema-rule fixtures must set `kind = "schema"`; this makes the fixture's
  `.graphql`/`.gql` source the schema input and avoids executable-document
  parse errors or duplicate diagnostics caused by loading the same SDL through
  both `schema = ...` and an operation document.
- Operation-rule fixtures must keep executable source in the main document and
  declare the SDL separately with `schema = ...` or `schema_path = ...`.
- When a rule intentionally compares a local definition against a merged
  schema (especially type extensions), document that source-ownership decision
  in the spec and add a fixture that would fail if the implementation only
  inspected the merged schema. This prevents a green fixture from masking a
  mismatch between upstream AST visitor behavior and the engine's semantic
  model.

### Validation bridges and multi-entry runners

- When a dependency reports both syntax/build and semantic validation errors
  through one collection, classify them before translating diagnostics. Syntax
  errors belong to the engine's parse-error channel; stable semantic error
  codes belong to the mapped rule bridge. Do not emit both for one underlying
  failure, and do not infer a rule id from localized message text.
- Pin the dependency version and record every deliberate mapping gap. If the
  dependency exposes a rule id but no stable diagnostic code, keep the public
  entry only when configuration compatibility is required and document that it
  is inactive until a structured adapter exists.
- For a family of rule entries backed by one runner, register one entry per
  configured id and run focused fixtures with the specific id enabled. Avoid a
  synthetic umbrella rule: the engine must stamp the actual rule id so
  enable/disable filtering and fixture ownership are exercised.
- A shared runner that appends sibling sources must preserve the current
  source's original byte range and report only diagnostics owned by that
  source. Add a cross-file fixture when sibling context affects validation.

### Schema category versus schema precondition

- Keep `category = "schema"` and `requires_schema` metadata separate: the
  category controls which SDL/CST inputs the rule visits, while
  `requires_schema` controls whether the engine skips it when no compiled
  schema is available. A schema-category rule with `requires_schema = false`
  still needs `kind = "schema"` fixtures so its SDL source is dispatched, but
  its fixtures must not add a redundant `schema = ...` merely to force
  execution.
- When a schema fixture needs helper SDL to satisfy the loader (for example,
  because the upstream snippet references a type that is irrelevant to the
  rule), append the smallest self-contained helper definitions after the
  upstream snippet. Do not prepend or interleave helpers, because that changes
  the line/column offsets used for parity. Record this source-ownership
  decision in the spec when it is material to the rule's behavior.

For shared helper specs without a rule harness, use a checked-in SDL fixture
loaded with `include_str!` and unit-test the public helpers directly. Cover
valid and structurally incomplete types, cross-type resolution, and the
option-driven naming behavior; do not rely on an empty rule fixture manifest as
evidence that a helper is tested.

For shared helpers that inspect a local schema definition, make the source
ownership decision explicit in the helper's module documentation and tests:
use the source AST when the helper must distinguish a definition from an
extension, and use the merged schema only when extension-contributed members
are intentionally part of the contract. Return borrowed AST members where
possible; do not clone fields or expose compiler-owned nodes through owned
facades. If the helper has no rule harness, its checked-in fixture should
exercise both the positive and negative predicate paths and any future-proof
accessor promised by the public API.

## Completion and merge handoff

Before staging, run `git diff --check`, `git status --short`, and inspect the
complete diff including the moved spec and README index. Stage only the files
belonging to the spec. After the PR is merged, return the checkout to the
default branch and refresh it with `git switch main` followed by
`git pull --ff-only origin main`; verify the final status and report any
pre-existing changes rather than deleting them.

When opening the PR, include the implementation branch, spec/rule scope,
upstream parity source and pinned revision, focused test, full validation
commands, and any deliberate scope difference. After merging, refresh `main`
and verify `git status -sb`; report any remaining changes instead of cleaning
them up implicitly.

## Reporter implementation contract

Reporters are adapters over `ProjectLintResult`, not alternate lint engines.
Keep output-specific serialization structs private to the reporter and leave
the core diagnostic model independent of presentation formats. A reporter must:

- resolve line, column, and end locations through the result's `SourceFile`
  index; never reread paths from disk, because fixtures and generated inputs
  may not exist on the filesystem;
- define deterministic file grouping, diagnostic ordering, and serialized key
  order so snapshots and parity diffs are stable across runs and projects;
- explicitly handle empty results, suppressed severities, zero-length spans,
  and missing source entries without panicking; use the format's documented
  fallback for data that cannot be resolved;
- propagate writer and serialization failures as `io::Error` rather than
  silently truncating output; and
- test both the format's human-readable and compact/machine modes where they
  exist, including a parsed shape assertion in addition to a byte-stable
  snapshot.

When a format claims upstream compatibility, record the exact field names,
severity mapping, coordinate convention, and path policy in its spec and test
those details directly. Do not infer compatibility from a generic
`serde::Serialize` derive on `Diagnostic`.

For schema-backed machine formats, keep the emitted contract and its vendored
schema in the same focused change. The schema should be strict for the fields
the reporter emits, while tests should validate both a representative
multi-diagnostic document and the empty/suppressed case against it. Record the
schema draft, tool/version policy, URI/path normalization, and fallback used
when a diagnostic's `SourceFile` is missing. If the format has a standard
schema, pin its immutable source or clearly label a deliberately reduced
subset; never call a self-authored subset the full standard.

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

## Options struct convention

Use `#[serde(rename_all = "camelCase")]` on options structs so JSON keys like `"maxDepth"` map to `max_depth`:

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    allowed: Vec<String>,
}
```

Read options in the `handler` method or in `finalize`:

```rust
fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
    let opts: Opts = ctx.option().unwrap_or_default();
    Box::new(Handler { opts })
}
```

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

### Operation-side schema + sibling rules

For rules that inspect typed executable ASTs (especially rules with both
`requires_schema` and `requires_siblings`), do the semantic walk in
`Handler::finalize` and keep source ownership explicit:

- Iterate `siblings.operations()` and `siblings.fragments_all()`, filtering each
  definition by `definition.source.path() == ctx.file.path()` before reporting.
  This prevents every per-file handler from duplicating diagnostics.
- Use fragment spreads only to answer whether a field is selected; walk each
  fragment definition independently when it is the current source so nested
  violations have a stable owner.
- `apollo_compiler::executable::SelectionSet` has no source span. Preserve the
  owning field/fragment node location and map it to the selection-set opening
  brace when parity requires the GraphQL AST selection-set location.
- In operation fixtures, use `sibling_documents = [...]` for helper fragments.
  The `documents` config field is reserved for inline operation documents in
  `kind = "schema"` fixtures. Sibling files are real lint inputs, so expected
  diagnostics must include their source-owned reports.

This pattern also applies when an upstream rule is implemented as an ESLint
visitor over one primary file: make the Rust engine's per-source behavior
explicit in the spec and expected fixtures instead of silently relying on
cross-file traversal side effects.

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

### Checking if a type is a scalar (built-in or custom)

Built-in scalars (`String`, `Int`, `Float`, `Boolean`, `ID`) are implicit in the GraphQL spec and may not appear in `schema.types`. Check them explicitly before falling back to the schema:

```rust
fn is_scalar_type(type_name: &str, schema: &apollo_compiler::Schema) -> bool {
    match type_name {
        "String" | "Int" | "Float" | "Boolean" | "ID" => return true,
        _ => {}
    }
    match schema.types.get(type_name) {
        Some(ext_type) => matches!(ext_type, apollo_compiler::schema::ExtendedType::Scalar(_)),
        None => false,
    }
}
```

## `NAMED_TYPE` subscription pattern (spec-037 pattern)

When a rule needs to inspect type **references** (not definitions), subscribe to `kinds = "NAMED_TYPE"`. The `on_node` fires for every named type reference (return types, argument types, etc.), with `node.name` giving the type name and `node.span` pointing at the type identifier.

### Walking up from NAMED_TYPE to find the containing field definition

To distinguish return-type references from argument-type references, walk the parent chain and return `None` if you encounter `INPUT_VALUE_DEFINITION` (argument):

```rust
fn find_field_def<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent?;
    loop {
        if current.kind == SyntaxKind::INPUT_VALUE_DEFINITION {
            return None; // skip argument type references
        }
        if current.kind == SyntaxKind::FIELD_DEFINITION {
            return Some(current);
        }
        current = current.parent?;
    }
}
```

Then from the `FIELD_DEFINITION`, walk further up to find the containing type definition:

```rust
fn find_type_def<'a>(field_def: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = field_def.parent?;
    loop {
        if matches!(current.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return Some(current);
        }
        current = current.parent?;
    }
}
```

## Test file template

New file: `crates/rglint-rules/tests/rule_<snake_case>.rs`

```rust
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

All fixture files must use the **full suffix** — the harness matches via `name.ends_with(suffix)`, so `config.toml` does NOT match `.config.toml`, and `expected.json` does NOT match `.expected.json`. Always use the `NN.`-prefixed form: `01.config.toml`, `01.expected.json`, `01.graphql`. Avoid bare filenames like `config.toml`, `expected.json`, or `graphql`.

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

### BFS reachability for schema rules (spec-036 pattern)

When computing type reachability via BFS/DFS over the compiled schema, follow ALL edges:

```rust
match ext_type {
    ExtendedType::Object(obj) => {
        // Follow interface implementations
        for iface in &obj.implements_interfaces { /* add to queue */ }
        for field in obj.fields.values() {
            // Follow field return types
            if let Some(base) = resolve_base_type_name(&field.ty) { /* add */ }
            // Follow field argument types (input objects, enums, scalars)
            for arg in &field.arguments {
                if let Some(base) = resolve_base_type_name(&arg.ty) { /* add */ }
            }
        }
    }
    ExtendedType::Interface(iface) => {
        // Follow interface-to-interface implementations
        for iface_name in &iface.implements_interfaces { /* add */ }
        for field in iface.fields.values() {
            if let Some(base) = resolve_base_type_name(&field.ty) { /* add */ }
            for arg in &field.arguments { /* add arg types */ }
        }
        // Follow implementers: objects AND interfaces implementing this interface
        for (_name, other_type) in &schema.types {
            let implements = match other_type {
                ExtendedType::Object(obj) => Some(&obj.implements_interfaces),
                ExtendedType::Interface(iface2) => Some(&iface2.implements_interfaces),
                _ => None,
            };
            if let Some(impls) = implements {
                if impls.iter().any(|i| i.as_str() == type_name) { /* add */ }
            }
        }
    }
    ExtendedType::Union(union) => {
        for member in &union.members { /* add */ }
    }
    // Scalar, Enum, InputObject are terminal
}
```

Also follow directive definition argument types to find referenced types:

```rust
for (_dir_name, dir_def) in &schema.directive_definitions {
    for arg in &dir_def.arguments {
        if let Some(base) = resolve_base_type_name(&arg.ty) { /* add */ }
    }
}
```

### Directive unreachable reporting logic

Directives are only reported as unreachable when the reachable set is empty (no root types found). When at least one user-defined type is reachable, skip directive reporting:

```rust
fn has_user_reachable_types(reachable: &HashSet<String>, schema: &apollo_compiler::Schema) -> bool {
    for type_name in reachable {
        if type_name.starts_with("__") { continue; }
        match type_name.as_str() {
            "String" | "Int" | "Float" | "Boolean" | "ID" => continue,
            _ => {}
        }
        if schema.types.contains_key(type_name.as_str()) { return true; }
    }
    false
}
```

Directive messages use `"Directive \`X\` is unreachable."` (no "type" in the message), while type messages use `"Scalar type \`X\` is unreachable."`, etc.

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

### Reporter source ownership

Reporters consume `ProjectLintResult`, so every result must retain the
`SourceFile` handles needed to render its diagnostics. Do not make reporters
re-read diagnostic paths from disk: inline fixtures, generated sources, and
deduplicated documents may not have a readable path or may have source text
that differs from the current filesystem. Keep the source index keyed by the
same path stored in `Diagnostic::file`, and use the engine's source mapping
for line and column output. A reporter must remain total for empty results,
zero-length spans, missing source entries, and writer errors; these cases
should produce stable text or a returned I/O error, never a panic.

For line-oriented workflow commands such as the GitHub annotations reporter,
separate property escaping from message escaping. Escape command delimiters
before writing user-controlled paths or messages, truncate before escaping, and
never allow a raw carriage return or newline into an annotation command. Keep
the coordinate convention explicit (including whether end coordinates are
exclusive), use a documented relative-path root, and test literal `%`, CR/LF,
commas, colons, multi-line spans, Unicode truncation, and long messages. Summary
output must be opt-in and its counts must exclude `Severity::Off` diagnostics.

## Fix application and staged entry points

Fixers are adapters over `ProjectLintResult`, just like reporters are. They
must use the result's retained `SourceFile` text for byte-range validation and
must not reread a diagnostic path to calculate an edit. Group suggestions by
physical file, sort deterministically, keep the lowest-offset non-overlapping
edits, and apply accepted edits rightmost-first. Reject out-of-bounds or
non-UTF-8-boundary ranges with an error; do not silently clamp user-facing
fixes. `--fix` v1 is limited to files in `Project.documents`, so schema source
files remain untouched even when a mixed rule has suggestions. Dry-run must
share the same conflict and iteration logic without filesystem writes, and
no-op or self-recreating fixes must be bounded by an explicit pass cap.

When a spec lists a later CLI or integration spec as a dependency, implement
the testable core seam owned by the current spec and record the wiring boundary
in the spec/architecture notes. Do not add a speculative CLI entry point or
duplicate config parsing merely to exercise a core API early; the owning entry
point spec should consume the stable public interface.

## CLI and process-boundary conventions

The binary is a thin adapter over the public config, resolver, engine, fixer,
and reporter APIs. Keep command-line parsing in `crates/rglint/src/cli.rs`,
numeric status mapping in `exit.rs`, and `main.rs` limited to parsing plus
`std::process::exit`; do not duplicate linting or config parsing in `main`.

Treat stdout and stderr as separate interfaces: diagnostics and fix diffs go
to stdout, while configuration, usage, progress, and internal errors go to
stderr. `--quiet` suppresses progress and human-readable summaries but must
not suppress machine-readable diagnostics. Every new flag needs a parser test
and, when it changes process behavior, an integration test that asserts both
the status code and the affected stream.

CLI tests must cover the no-config path, explicit `--config` path resolution,
`--max-warnings`, `--init` refusing to overwrite an existing file, and
`--fix-dry-run` leaving source files unchanged. Resolve positional paths to
stable physical inputs before handing them to `ProjectResolver`; keep config
relative paths relative until the resolver receives the config directory.

## Parallel engine and cache invariants

Per-file parallelism belongs at the engine boundary: keep one multiplexed rule
walk per source file, share only immutable schema/sibling context, and use a
scoped Rayon pool rather than mutating Rayon’s global pool. `--jobs` must reject
zero and every flag change needs a parser test. The WASM build must retain the
same API validation while taking a serial path.

Worker closures must catch panics at the file boundary and return an
`internal-error` diagnostic attributed to the affected source. Do not allow a
panic to poison a shared cache lock or abort unrelated files. Cache reads must
return owned snapshots (never lock guards), lock poisoning must recover, and
any engine-integrated cache key must include the rule/config/schema/sibling
execution context in addition to source content. Add a contention test and a
repeat-run byte-stability test whenever the dispatch or cache implementation
changes.

Performance timing harnesses belong to the benchmark spec that owns their
framework and corpus. A correctness spec may add focused smoke/contended tests,
but must not add a second ad-hoc benchmark framework or make a flaky wall-clock
threshold part of the normal test suite.

## Branch / PR workflow

- Branch name: `spec-NNN`
- Commit message: `spec-NNN: <rule-id>` (matches existing pattern)
- Push, then `gh pr create --title "spec-NNN: <rule-id>" --body "<description>"`
- Merge via `gh pr merge --squash`
- After merge, run `git switch main`, `git pull --ff-only origin main`, and
  `git status -sb`; report any remaining changes rather than hiding them.

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
