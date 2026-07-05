# Spec-011: LintEngine orchestration

> Plan reference: §3 (`crates/rglint-core/src/engine.rs`), §1 ("Lint Engine", "Visitor Pipeline")

## Goal

Implement `LintEngine` — the orchestrator that, given a `Project`, runs all
enabled rules over all documents, walking each document's AST once and
multiplexing to subscribed rule handlers, then collecting diagnostics. This is
the single entry point the CLI (spec-062) calls.

## Scope

**In scope:**

- `LintEngine` struct holding the compiled rule registry + config (which rules
  enabled, severities, options).
- Per-document pipeline:
  1. Emit `parse-error` diagnostics from the loader (specs 004/005).
  2. For each enabled rule, build a `RuleContext` + `Handler` via
     `Rule::create`.
  3. Walk the `apollo_compiler` AST/CST once, calling `Handler::on_node` for
     each node whose `SyntaxKind` is in **any** rule's `interested_kinds`
     (pre-filtered set). Pass parent where available.
  4. After the walk, call `Handler::finalize(ctx)` for each rule.
  5. Drain `ctx.take_diagnostics()` into the project's result.
- `LintResult` per file: `Vec<Diagnostic>`.
- `LintEngine::lint(project: &Project) -> Result<ProjectLintResult>`.
- Severity filtering: drop `Severity::Off` diagnostics.
- Ordering: diagnostics sorted by (file, line, column, rule_id) for stable
  output.

**Out of scope:**

- Parallelism (spec-064 adds rayon; this spec is single-threaded but the
  engine is `Send + Sync` so rayon drops in).
- `--fix` (spec-061).
- Reporting (specs 057-060).

## Dependencies

- spec-004, spec-005 (loaders — engine receives already-loaded `Project`).
- spec-006 (Siblings).
- spec-007 (Project).
- spec-008 (Rule, RuleEntry, all_rules()).
- spec-009 (RuleContext).
- spec-010 (Selector — engine compiles rule selectors into the per-kind
  subscription; a rule's `interested_kinds` may be derived from its selector).
- spec-012 (node_name — used by some handlers).

## Deliverables

- `crates/rglint-core/src/engine.rs`.
- Integration test: run a 2-rule config over a fixture, assert diagnostics
  match hand-computed expected.

## Interface / API

```rust
pub struct LintEngine {
    rules: Vec<EnabledRule>,   // (RuleEntry, severity, options)
}

pub struct EnabledRule {
    pub entry: &'static RuleEntry,
    pub severity: Severity,
    pub options: serde_json::Value,
}

pub struct ProjectLintResult {
    pub project_name: String,
    pub by_file: AHashMap<PathBuf, Vec<Diagnostic>>,
    pub all: Vec<Diagnostic>,   // sorted
}

impl LintEngine {
    pub fn new(config: &RulesConfig) -> Result<Self>; // resolves rule ids -> entries
    pub fn lint(&self, project: &Project) -> Result<ProjectLintResult>;
}
```

## Behavior

- A rule with `requires_schema: true` and a project with `schema: None` is
  **skipped** (not an error) — matches graphql-eslint.
- A rule with `requires_siblings: true` and `siblings` present uses it; without
  siblings it skips.
- The walk is a single recursive descent over the CST; at each node, the
  engine iterates only the handlers whose `interested_kinds` contains the
  node's `SyntaxKind`.
- Handlers are dropped (resources freed) once `finalize` returns.
- Empty document (no operations/fragments) → no rule runs, zero diagnostics.

## Testing

- Fixture `tests/fixtures/engine/two-rules/`: schema with an `ID!` field and an
  anonymous query; enable `no-anonymous-operations` + `strict-id-in-types`;
  assert exactly the two expected diagnostics, sorted correctly.
- `Severity::Off` rule produces no diagnostics.
- `requires_schema` rule skips on schema-less project.

## Risks / Notes

- Borrow checker: `RuleContext` borrows `&Project` fields; `Handler` borrows
  `&mut RuleContext`. Walking the AST (which borrows `&ExecutableDocument`)
  while holding `&mut RuleContext` is the tricky part — design handlers to
  receive node data by value (copy what they need) or use arena indices. Spike
  in this spec; the chosen pattern becomes the template for all rules.
