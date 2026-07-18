# Spec-028: naming-convention

> Plan reference: §5 Phase 2 ("naming-convention — biggest single rule ~579 lines"), §3 (`crates/rglint-rules/src/schema/naming_convention.rs`), §8 (risk)

## Goal

Port `naming-convention` — the largest single rule. Enforces identifier styles
per node kind with per-kind options, forbidden prefixes/suffixes, regex
overrides, and custom messages. Built on `shared/case.rs` (spec-022).

## Source

`packages/plugin/src/rules/naming-convention/index.ts` (~579 LOC) +
`case-packages.ts` (spec-022 ports the case part).

## Scope

**In scope:**

- Rule id `naming-convention`, category `Schema`.
- Options (full shape from TS `configOptions`):
  - Per-kind config: `{ style: CaseStyle, forbiddenPrefixes: [String],
    forbiddenSuffixes: [String], requiredPrefixes: [String],
    requiredSuffixes: [String], forbiddenPatterns: [String],
    ignorePattern: String }` keyed by kind
    (`types`, `FieldDefinition`, `EnumValueDefinition`,
    `InputObjectTypeDefinition`, `InterfaceTypeDefinition`,
    `UnionTypeDefinition`, `ScalarTypeDefinition`, `DirectiveDefinition`,
    `Argument`, `Variable`, `Enum`, `Operation`, `Fragment`, ...).
  - Top-level shortcuts: `"types": "PascalCase"` (string → style only).
- For each in-scope node, validate against its kind's config; report with the
  exact graphql-eslint message template + a suggestion to rename to the
  converted form.
- `hasSuggestions: true`.

**Out of scope:**

- Case helper internals (spec-022).

## Dependencies

- spec-022 (case.rs — hard prerequisite).
- spec-008, spec-009, spec-011, spec-012, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/naming_convention.rs`.
- `rules-fixtures/naming-convention/` (largest fixture set — verify all cases
  ported).
- `tests/rule_naming_convention.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts {
    types: KindConfig,
    field_definition: KindConfig,
    // ... one field per supported kind
    #[serde(default)] ignore_pattern: Option<String>,
}
#[derive(Default, Deserialize)]
struct KindConfig {
    style: Option<CaseStyle>,
    #[serde(default)] forbidden_prefixes: Vec<String>,
    #[serde(default)] forbidden_suffixes: Vec<String>,
    #[serde(default)] required_prefixes: Vec<String>,
    #[serde(default)] required_suffixes: Vec<String>,
    #[serde(default)] forbidden_patterns: Vec<String>,
}
```

## Behavior

- A kind absent from options → that kind is not checked.
- `ignorePattern` — node names matching it are skipped globally.
- Message templates (port verbatim from TS): prefix/suffix/pattern/style
  violations each have distinct messages naming the node, the kind, and the
  expected style.
- Suggestion: `Fix::Replace` of the name token with `convert_case(name,
  style, acronyms)` plus any required prefix/suffix. Acronyms option
  supported.

## Testing

- `rglint_test_suite!("naming-convention")` — **message + location + suggestion
  parity**; this is the rule most likely to diverge, so every fixture must pass.
- PLAN §8 mitigation: "property test that any `naming-convention` config from
  `rules-fixtures/naming-convention/` produces byte-identical messages." Add
  it here.
- Snapshot of suggestion output for one prefix and one style case.

## Risks / Notes

- §8 risk: this is the biggest rule. Port methodically: scaffold the option
  struct, port one kind end-to-end (with fixtures green), then iterate kinds.
  Resist refactoring `case.rs` mid-port — if a helper is missing, add it and
  update spec-022.
- Verify `acronyms` option semantics in TS before implementing (may be
  per-kind or global).
