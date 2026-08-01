# Spec-053: rglint-graphql-spec bridge

> Plan reference: §5 Phase 7, §3 (`crates/rglint-graphql-spec/`), §8 (message-divergence risk), §6.1 (`loose_message`)

## Goal

Bridge `apollo_compiler::validate` output into rglint diagnostics, exposing
the ~28 graphql-js spec validation rules as rglint rule entries. Spec
validation is **not** reimplemented (PLAN §1 principle 5) — apollo-compiler
already implements it; we map each validation error to a rglint rule id +
location, and surface as diagnostics.

## Source

`graphql-hive/graphql-eslint` at immutable commit
`241936acfebef3e6201703e483776d3f952a6f0f`,
`packages/plugin/src/rules/graphql-js-validation.ts`.

## Scope

**In scope:**

- `rglint-graphql-spec` crate with:
  - `names.rs`: mapping `apollo_compiler::validate` error code → rglint rule
    id, e.g. `UndefinedField` → `fields-on-correct-type`. Register all 30
    upstream rule ids and map every Apollo 1.32 diagnostic with a stable
    structured error name.
  - `spec_rules.rs`: a single `Rule` that runs `apollo_compiler::validate` on
    the schema+document, iterates `ValidationErrors`, translates each into a
    `Diagnostic` (rule id from `names.rs`, span from the error's location,
    message from apollo-compiler).
  - `lib.rs`: `all_spec_rules() -> &'static [RuleEntry]` — one entry per
    mapped rule id, all backed by the same runner but filtered by id so the
    config can enable/disable individual spec rules.
- The bridge is `requires_schema: true` for schema-side spec rules and uses
  the document for executable-side rules.

**Out of scope:**

- Reimplementing graphql-js validation (apollo-compiler owns this).
- Exact message parity (PLAN §8: accept divergence; compare on rule-id +
  location only).
- Message-text inference for Apollo diagnostics. Six SDL rule ids remain
  registered but intentionally produce no bridge diagnostic because Apollo
  1.32 exposes no stable structured code for their schema-builder errors.

## Dependencies

- spec-004, spec-005 (loaders).
- spec-008 (Rule/RuleEntry — spec rules are registered like any rule).
- spec-011 (engine — the spec rules are just more `RuleEntry`s).
- spec-014 (harness — `loose_message: true` flag in fixtures for these rules).

## Deliverables

- `crates/rglint-graphql-spec/src/{lib,spec_rules,names}.rs`.
- `rules-fixtures/graphql-js-validation/` — generated fixtures use
  `loose_message: true` in `.config.toml`.
- `tests/conformance/graphql-js/known-divergences.md` — record every message
  mismatch discovered during fixture runs (PLAN §8 mitigation).
- `tests/rule_graphql_js_validation.rs`.

## Interface / API

```rust
// names.rs
pub fn rule_id_for(error: &apollo_compiler::validation::Diagnostic) -> Option<&'static str>;

// spec_rules.rs
pub struct SpecRule { id: &'static str }
impl Rule for SpecRule { /* runs validate, filters by id, reports */ }

// lib.rs
pub fn all_spec_rules() -> &'static [RuleEntry];
```

## Behavior

- One execution of Apollo validation produces all validation errors; the
  bridge dispatches each to its mapped rule id, so enabling only
  `fields-on-correct-type` filters to just those errors.
- Unmapped error codes (Apollo extras with no graphql-eslint id) are dropped
  with a debug log to avoid duplicate or synthetic diagnostics.
- `loose_message` in the harness (spec-014) makes the runner compare only
  rule-id + line + column for these rules, tolerating message text divergence.
- Executable semantic build diagnostics are not emitted as `parse-error` by the
  document loader when Apollo supplies a stable error name; this prevents the
  bridge from reporting a second diagnostic for the same validation failure.

## Testing

- `rglint_test_suite!("graphql-js-validation")` with `loose_message: true`.
- A `known-divergences.md` entry per fixture case whose message differs from
  graphql-eslint's (manual audit during port). The committed audit is at
  `tests/conformance/graphql-js/known-divergences.md`.

## Risks / Notes

- §8 risk: "apollo-compiler validation errors have slightly different
  messages than graphql-js." This is expected and **documented**, not treated
  as a bug — the divergence log is the artifact.
- Some graphql-eslint spec rule ids have no Apollo 1.32 counterpart with a
  stable structured name. The six retained-but-inactive SDL ids and the
  Apollo-to-graphql-eslint mapping are listed in `names.rs` and the divergence
  log; no message matching is used to guess a rule id.
