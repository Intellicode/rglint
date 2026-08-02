# Spec-056: JSON-schema option validation

> Plan reference: §3 (`crates/rglint-config/src/validate.rs`, `crates/rglint-core::RuleMeta::option_schema`), §2 (`jsonschema`), §8 (option-defaults risk)

## Goal

Validate each enabled rule's `options` against that rule's
`RuleMeta::option_schema` (the repository's lazy `jsonschema::JSONSchema`,
compiled as Draft 2020-12) at config validation time,
producing actionable config errors before any linting runs. Mirrors
graphql-eslint's JSON-schema meta-validation.

The repository already has a public `rglint-config::ConfigError` enum for
file I/O, parse, and normalization failures, so option failures use its
`InvalidRuleOptions` variant with a public per-error payload rather than
replacing that established API with a new `ConfigError` struct.

## Scope

**In scope:**

- `rglint-config::validate` — `validate_rule_options(meta: &RuleMeta,
  options: &serde_json::Value) -> Result<(), Vec<RuleOptionError>>`.
- Uses the Draft 2020-12 `jsonschema::JSONSchema` stored in `RuleMeta`.
- On failure, produces `RuleOptionError` values naming the rule, the JSON
  Pointer of the offending option, and a human message.
- Default-options injection: when `options` is `{}` (empty) and
  `RuleMeta::default_options` is set, substitute defaults before validating
  (so a rule with required options still works when the user omits them).
- Wiring: `Config::validate(&[&RuleEntry]) -> Result<(), ConfigError>` runs
  validation for every non-disabled, known rule and batches all failures in
  `ConfigError::InvalidRuleOptions`. The CLI does not exist until spec-062;
  that spec must call this method before `LintEngine::new`.

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
pub struct RuleOptionError {
    pub rule_id: String,
    pub schema_path: String,   // e.g. "/maxDepth"
    pub message: String,
}

pub fn validate_rule_options(meta: &RuleMeta, options: &serde_json::Value) -> Result<(), Vec<RuleOptionError>>;
pub fn apply_defaults(meta: &RuleMeta, options: &mut serde_json::Value);
impl Config {
    pub fn validate(&self, rules: &[&RuleEntry]) -> Result<(), ConfigError>;
}
```

## Behavior

- `option_schema = None` → always valid (rule takes freeform options).
- Validation errors are batched (all reported at once, not fail-fast) so the
  user sees every problem in one pass. Unknown rule ids are skipped here for
  forward compatibility and remain the engine registry's responsibility.
- Rules configured as `off` are not enabled and are not option-validated.
- Default injection is a shallow merge: `default_options` provides keys
  missing from user `options`; user keys win.

## Testing

- See Deliverables unit tests.
- A rule with no `option_schema` → `validate_rule_options` returns `Ok(())`.
- `Config::validate` batches known-rule failures and ignores disabled and
  unknown rule ids.

## Risks / Notes

- §8 risk: "JSON-schema option validation differs from TS-typed defaults" —
  the `apply_defaults` step + extracting `meta.docs.configOptions` in spec-015
  (written to `rules-fixtures/<id>/defaults.json`) together close the gap.
  This spec consumes `default_options` from `RuleMeta`; populating that field
  is each rule spec's job.
- The CLI integration is deliberately deferred to spec-062 because no CLI
  entry point exists in the current workspace.
