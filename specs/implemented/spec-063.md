# Spec-063: Default recommended config preset

> Plan reference: §5 Phase 8 ("default-recommended config preset (mirror configs/*.ts)"), §3 (`crates/rglint-config`), §10

## Goal

Ship a built-in `recommended` (and `all`) config preset mirroring
graphql-eslint's `packages/plugin/src/configs/*.ts` so users get a sensible
default rule set without enumerating every rule.

## Source

Pinned upstream snapshot: `f0f200ef0b030cb8a905bbcb32fe346b87cc2e24`.

`packages/plugin/src/configs/{schema-recommended,schema-all,operations-recommended,operations-all,schema-relay,index}.ts` at that revision.

The upstream source is authoritative for the preset contents. In particular,
`schema-all` extends `schema-recommended`, `operations-all` extends
`operations-recommended`, and `schema-relay` contains only the four Relay
rules; the latter does not extend the schema-recommended preset.

## Scope

**In scope:**

- `rglint-config::preset` module exposing:
  - `recommended()` — the union of `schema-recommended` + `operations-recommended`.
  - `all()` — `schema-all` + `operations-all` (every rule as `error`).
  - `schema_recommended()`, `operations_recommended()`, `schema_relay()`,
    `schema_all()`, `operations_all()` (granular presets).
- Each preset is a `Config` with `rules` populated by rule id + default
  severity, matching the TS source exactly.
- A `extends: "recommended"` (or `["schema-recommended", "operations-recommended"]`) key in `.rglintrc` pulls in a preset; user `rules` override preset severities/options. The normalized config retains only the resolved rules, not the source `extends` spelling.
- `rglint --init` writes a `.rglintrc.toml` with `extends = "recommended"`
  (spec-062).

**Out of scope:**

- Custom user presets (out of v1; they write their own config).

## Dependencies

- spec-054 (Config).
- All rule specs (the preset references their ids + defaults — coordinate so
  preset rule ids match `RuleMeta::id`).

## Deliverables

- `crates/rglint-config/src/preset.rs`.
- A test asserting the preset's rule-id set + severities **exactly** matches
  the TS `configs/*.ts` (parse the TS at test time via a snapshot, or pin a
  static expected map — prefer a static map snapshot to avoid TS-dep in
  tests).

## Interface / API

```rust
pub fn recommended() -> Config;
pub fn all() -> Config;
pub fn schema_recommended() -> Config;
pub fn operations_recommended() -> Config;
pub fn schema_relay() -> Config;
pub fn schema_all() -> Config;
pub fn operations_all() -> Config;

// Config merge for `extends`
impl Config {
    pub fn extend_with(&mut self, preset: Config); // preset is base; self overrides
}
```

## Behavior

- `extends` resolution: built-in composite presets resolve their documented
  parent presets transitively and detect cycles in the resolver. User config
  may name built-in presets only; custom user presets remain out of scope.
- Merge: rule in both → user's wins; rule only in preset → preset's; rule
  only in user → user's.
- `ignore` and `format` merge: union / user-wins respectively.

## Testing

- Snapshot of each preset's `(rule_id, severity)` set; fail if it diverges
  from the TS source (pin the expected set at port time; update deliberately).
- `extends` cycle detection → error.

## Risks / Notes

- This spec is the single source of truth for which rules are "on by default";
  coordinate with spec-062's `--init` output. Pin the expected map from the
  TS source during port and commit it as a snapshot to catch drift.
