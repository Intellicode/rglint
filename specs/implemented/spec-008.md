# Spec-008: Rule trait, RuleMeta & registry

> Plan reference: §3 (`crates/rglint-rules/src/lib.rs`, `meta.rs`, `crates/rglint-derive/`), §4.1, §1 ("Rule Registry")

## Goal

Define the `Rule` and `Handler` traits, the `RuleMeta` descriptor, the rule
`Category`, and the registry that aggregates all built-in rules into a static
array dispatchable by `SyntaxKind`. Also implement the `rglint-derive`
proc-macro crate (`#[derive(Rule)]`) that auto-registers a rule struct so
`rglint-rules::all_rules()` is generated without manual list maintenance.

## Scope

**In scope:**

- `Rule` trait (§4.1): `meta()` + `create(&self, ctx) -> Box<dyn Handler>`.
- `Handler` trait: `on_node(&mut self, node, parent)` + `finalize(&mut self, ctx)`.
- `RuleMeta` (§4.1): id, category, severity, docs, `option_schema`
  (`Option<jsonschema::Validator>`), default_options, requires_schema,
  requires_siblings, deprecated, replaced_by, has_suggestions.
- `Category` enum: `Schema`, `Operations`, `Other` (mirrors graphql-eslint).
- `RuleEntry` = `{ meta: &'static RuleMeta, factory: fn() -> Box<dyn Rule>,
  interested_kinds: &'static [SyntaxKind] }` — the registry stores these.
- `rglint-rules::all_rules() -> &'static [RuleEntry]` built via
  `inventory` submissions OR a `rglint-derive`-generated array. **Decision:
  use `rglint-derive` static array** (no `inventory` runtime cost; simpler for
  WASM).
- `rglint-derive::rule` proc-macro: `#[derive(Rule)]` on a unit struct emits:
  - `impl Rule for ...` forwarding to a `create` method.
  - A submission into a `pub const RULES: &[RuleEntry]` aggregated across the
    crate via `linkme` (distributed slice) — preferred over `inventory` for
    no-init-order issues.

**Out of scope:**

- `RuleContext` body (spec-009).
- The engine's visitor dispatch (spec-011) — this spec only defines the data
  the registry holds.
- Individual rule implementations (specs 016+).

## Dependencies

- spec-002 (Span — referenced by Handler signatures indirectly).
- spec-003 (Severity — used by RuleMeta).
- spec-001 (proc-macro crate exists).

## Deliverables

- `crates/rglint-core/src/rule.rs` — `Rule`, `Handler`, `RuleMeta`, `Category`,
  `RuleEntry` (core so `rglint-graphql-spec` and `rglint-rules` both depend on
  it, not on each other).
- `crates/rglint-derive/src/lib.rs` + `rule_derive.rs`.
- `crates/rglint-rules/src/lib.rs` — `all_rules()` aggregating via `linkme`.
- `crates/rglint-rules/src/meta.rs` — helper constructors for `RuleMeta`.
- Doctest + unit test: a dummy rule with `#[derive(Rule)]` appears in
  `all_rules()`.

## Interface / API

```rust
// rglint-core/src/rule.rs
pub trait Rule: Send + Sync + 'static {
    fn meta(&self) -> &'static RuleMeta;
    fn create<'s>(&'s self, ctx: &'s mut RuleContext) -> Box<dyn Handler + 's>;
}

pub trait Handler {
    fn on_node(&mut self, _node: &Node<'_>, _parent: Option<&Node<'_>>) {}
    fn finalize(&mut self, _ctx: &mut RuleContext) {}
}

pub struct RuleMeta { /* §4.1 fields */ }
pub enum Category { Schema, Operations, Other }

pub struct RuleEntry {
    pub meta: &'static RuleMeta,
    pub factory: fn() -> Box<dyn Rule>,
    pub interested_kinds: &'static [SyntaxKind],
}
```

```rust
// rglint-derive
#[proc_macro_derive(Rule, attributes(rule))]
pub fn rule_derive(input: TokenStream) -> TokenStream { /* ... */ }
// Usage:
#[derive(Rule)]
#[rule(id = "no-anonymous-operations", category = "operations")]
struct NoAnonymousOperations;
```

## Behavior

- `all_rules()` is a `const`-friendly static slice; iteration is zero-cost.
- `interested_kinds` lets the engine (spec-011) skip walking a rule's handler
  for AST kinds the rule doesn't care about.
- `RuleMeta::option_schema` is built `OnceCell`-lazy because
  `jsonschema::Validator` construction is non-const.

## Testing

- Derive-macro test: a `#[derive(Rule)]` struct in `rglint-rules` appears in
  `all_rules()` with the correct `meta().id`.
- Negative: a rule without `#[derive(Rule)]` does not appear.

## Risks / Notes

- `linkme` requires a linker section; verify it works on the CI matrix
  (Linux/mac/Windows). Fallback: explicit `all_rules()` array literal in
  `rglint-rules/src/lib.rs` — simpler, no proc-macro. **Recommendation: start
  with the explicit array in spec-008 v1, add the derive in a later polish
  spec if maintenance pain emerges.** Note this decision in `ARCHITECTURE.md`.
