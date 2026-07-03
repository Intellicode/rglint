# Spec-009: RuleContext

> Plan reference: §3 (`crates/rglint-core/src/context.rs`), §4.2

## Goal

Implement `RuleContext` — the per-rule, per-document handle that rules receive
in `create()` and `Handler::finalize()`. Mirrors `GraphQLESLintRuleContext`:
gives a rule access to the source file, schema, siblings, options, and the
`report()` sink. Owns the diagnostics buffer it writes into (the engine
collects it after the walk).

## Scope

**In scope:**

- `RuleContext<'a>` struct (§4.2) with fields: `file`, `schema`, `siblings`,
  `options`, `project`, private `diagnostics: Vec<Diagnostic>`.
- `report(DiagnosticBuilder)` — pushes a `Diagnostic` rooted at this file/rule.
- `source_code() -> &SourceFile`.
- `require_schema(rule_id) -> Result<&Schema>` — returns `Err` if `schema` is
  `None` (the engine should have skipped the rule, but this is a defense).
- `require_operations(rule_id) -> Result<&Siblings>`.
- `options()` typed accessor helpers: `option<T: DeserializeOwned>(&self) ->
  Result<T>` for rules to read their config.
- `take_diagnostics() -> Vec<Diagnostic>` — engine drains after the walk.
- `node_name(node) -> String` helper passthrough (spec-012).

**Out of scope:**

- The engine orchestration (spec-011).
- Rule trait (spec-008).

## Dependencies

- spec-002 (SourceFile, Span).
- spec-003 (Diagnostic, DiagnosticBuilder, Severity).
- spec-004 (LoadedSchema — `Schema` type).
- spec-006 (Siblings).
- spec-007 (ProjectConfig).
- spec-008 (Rule trait — `create` signature references `RuleContext`).
- spec-012 (node_name helper — soft; can stub here and refactor in 012).

## Deliverables

- `crates/rglint-core/src/context.rs`.
- Unit tests: `report()` accumulates; `require_schema` errors when None;
  `option::<MyOpts>()` deserializes a JSON value.

## Interface / API

```rust
pub struct RuleContext<'a> {
    pub file: &'a SourceFile,
    pub schema: Option<&'a apollo_compiler::Schema>,
    pub siblings: Option<&'a Siblings>,
    pub project: &'a ProjectConfig,
    options: &'a serde_json::Value,
    diagnostics: Vec<Diagnostic>,
    rule_id: &'static str,
    severity: Severity,
}

impl<'a> RuleContext<'a> {
    pub fn report(&mut self, b: DiagnosticBuilder);
    pub fn source_code(&self) -> &SourceFile { self.file }
    pub fn require_schema(&self, rule_id: &str) -> Result<&apollo_compiler::Schema>;
    pub fn require_operations(&self, rule_id: &str) -> Result<&Siblings>;
    pub fn option<T: DeserializeOwned>(&self) -> Result<T>;
    pub fn options_raw(&self) -> &serde_json::Value;
    pub fn node_name(&self, node: &Node) -> String; // delegates to spec-012
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic>;
    pub fn rule_id(&self) -> &'static str;
    pub fn severity(&self) -> Severity;
}
```

## Behavior

- `report()` stamps `rule_id`, `file`, and `severity` (from config, not from
  the builder) onto the `Diagnostic`; the builder's severity is ignored unless
  the rule explicitly overrides (rare).
- `option::<T>()` deserializes `options` into the rule's strongly-typed options
  struct; on failure returns an error the engine converts to a config-error
  diagnostic.
- `take_diagnostics` is called once by the engine after `finalize()`.

## Testing

- Construct a `RuleContext` with a JSON option `{"maxDepth": 7}`, assert
  `option::<DepthOpts>().unwrap().max_depth == 7`.
- `require_schema("x")` on a context with `schema: None` returns `Err` whose
  message names the rule.
- `report()` 3 times → `take_diagnostics().len() == 3`, all carrying the
  context's `rule_id` and `file`.

## Risks / Notes

- The `rule_id`/`severity` are set by the engine when constructing the context
  per-rule-per-file (spec-011), not by the rule itself — rules never know their
  own configured severity (matches eslint semantics).
