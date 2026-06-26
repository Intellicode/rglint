# Spec-056: JSON-schema option validation

> Plan reference: §3 (`crates/rglint-config/src/validate.rs`, `crates/rglint-core::RuleMeta::option_schema`), §2 (`jsonschema`), §8 (option-defaults risk)

## Goal

Validate each enabled rule's `options` against that rule's
`RuleMeta::option_schema` (a JSON-schema `Validator`) at config load time,
producing actionable config errors before any linting runs. Mirrors
graphql-eslint's JSON-schema meta-validation.

## Scope

**In scope:**

- `rglint-config::validate` — `validate_rule_options(meta: &RuleMeta,
  options: &serde_json::Value) -> Result<()>`.
- Uses `jsonschema::Validator` (draft 2020-12) stored in `RuleMeta`.
- On failure, produces a `ConfigError` naming the rule, the JSON-schema path
  of the offending field, and a human message.
- Default-options injection: when `options` is `{}` (empty) and
  `RuleMeta::default_options` is set, substitute defaults before validating
  (so a rule with required options still works when the user omits them).
- Wiring: `Config::validate(&[&RuleEntry]) -> Result<()>` runs validation for
  every enabled rule; called by the CLI before `LintEngine::new`.

**Out of scope:**

- Building the per-rule `option_schema` (each rule spec owns its schema; this
  spec only consumes them).
- Config file parsing (spec-054).

## Dependencies

- spec-008 (RuleMeta + option_schema).
- spec-054 (Config — the options source).

## Deliverables

- `crates/rglint-config/src/validate.rs`.
- Unit tests: a rule with `option_schema` requiring `{maxDepth: integer}` —
  valid options pass; `{"maxDepth": "x"}` fails with a path-bearing error;
  missing options + `default_options` present → defaults applied + pass.

## Interface / API

```rust
pub struct ConfigError {
    pub rule_id: String,
    pub schema_path: String,   // e.g. "/maxDepth"
    pub message: String,
}

pub fn validate_rule_options(meta: &RuleMeta, options: &serde_json::Value) -> Result<(), Vec<ConfigError>>;
pub fn apply_defaults(meta: &RuleMeta, options: &mut serde_json::Value);
impl Config {
    pub fn validate(&self, rules: &[&'static RuleEntry]) -> Result<()>;
}
```

## Behavior

- `option_schema = None` → always valid (rule takes freeform options).
- Validation errors are batched (all reported at once, not fail-fast) so the
  user sees every problem in one pass.
- Default injection is a shallow merge: `default_options` provides keys
  missing from user `options`; user keys win.

## Testing

- See Deliverables unit tests.
- A rule with no `option_schema` → `validate_rule_options` returns `Ok(())`.

## Risks / Notes

- §8 risk: "JSON-schema option validation differs from TS-typed defaults" —
  the `apply_defaults` step + extracting `meta.docs.configOptions` in spec-015
  (written to `rules-fixtures/<id>/defaults.json`) together close the gap.
  This spec consumes `default_options` from `RuleMeta`; populating that field
  is each rule spec's job.
