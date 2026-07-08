# AGENTS.md — conventions for implementing specs

## Spec lifecycle

1. Next unimplemented spec = lowest-numbered `spec-NNN.md` still in `specs/` (not in `specs/implemented/`). Check `specs/README.md` for the status index.
2. Read the spec, understand dependencies (they state which prior specs must be done).
3. Implement per the spec's Deliverables.
4. Move `specs/spec-NNN.md` → `specs/implemented/spec-NNN.md`.
5. Update `specs/README.md`: fix the link path (add `implemented/` prefix) and change status to `[x]`.
6. Build + test before committing.

## Rule implementation template

New rule file: `crates/rglint-rules/src/operations/<snake_case>.rs`

```rust
//! `<rule-id>` (spec-NNN).

use rglint_core::{Handler, RuleContext};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(id = "<rule-id>", category = "operations")]
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
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        // add assertion for requires_schema / requires_siblings as needed
    }
}
```

Register in `crates/rglint-rules/src/operations/mod.rs`: add `pub mod <snake_case>;`

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

## Fixture layout

Each rule gets a tree under `rules-fixtures/<rule-id>/`:

```
rules-fixtures/<rule-id>/
  manifest.json          # metadata (update valid_count / invalid_count / cases)
  valid/
    01/
      01.graphql         # source under lint
      01.sibling.graphql # sibling document (for requires_siblings rules)
      01.config.toml     # sibling_documents = ["01.sibling.graphql"]
  invalid/
    01/
      01.graphql
      01.sibling.graphql
      01.config.toml
      01.expected.json   # { "errors": [{ "rule": "...", "message": "...", "line": 1, "column": 0 }] }
```

Config fields: `schema`, `schema_path`, `kind` (operations/schema), `loose_message`, `[options]`, `sibling_documents`.

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
