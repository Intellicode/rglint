# Spec-014: Test harness (fixtures, snapshots, property tests)

> Plan reference: §3 (`crates/rglint-test-harness/`), §6 (Testing Strategy: layers, parity rules, snapshots, property, negative-path)

## Goal

Build the reusable test harness that drives rule parity against graphql-eslint
fixtures, plus the `insta` snapshot scaffolding and `proptest` property-test
helpers. This spec is what makes every subsequent rule spec verifiable against
the oracle.

## Scope

**In scope:**

- `rglint-test-harness::fixture` — parse a `rules-fixtures/<rule-id>/{valid,invalid}/NN.{graphql,config.toml,expected.json}` triplet into an in-memory `FixtureCase`.
  - `.graphql` = source under lint.
  - `.config.toml` = `schema` (inline or path) + `options` + `rule` severity.
  - `.expected.json` = `{ "errors": [{ rule, message, line, column }] }`
    (PLAN §6.1 format).
- `rglint-test-harness::expected` — `ExpectedError` struct with the parity
  fields (rule, message, line, column) and a `Comparator` that checks actual
  diagnostics against expected with the relaxed byte-offset rule (PLAN §6.3:
  compare line + column only, not raw offsets).
- `rglint-test-harness::runner` — `run_fixture(case, engine) -> Result<()>`
  that lints the case and asserts parity, producing a readable diff on
  mismatch (uses `pretty_assertions`).
- A macro `rglint_test_suite!(rule_id)` that discovers all fixtures under
  `rules-fixtures/<rule-id>/` and generates one `#[test]` per case.
- `insta` snapshot helper: `assert_diagnostic_snapshot(diagnostics, source)`
  producing a `.snap` showing the source with `^^^` carets + messages (the
  format `pretty` reporter uses, spec-057).
- `proptest` helpers: `prop_parse_roundtrip(src)` (parse → re-stringify →
  parse equality) and a rule-idempotence template.
- Negative-path helper: `assert_no_panic(malformed_src)` runs the engine and
  asserts it produces ≥1 diagnostic without panicking (PLAN §6.5).

**Out of scope:**

- The `xtask port-fixture` extractor (spec-015) — that *produces* the fixture
  files this harness *consumes*.
- Individual rule fixtures (each rule spec owns its `rules-fixtures/<id>/`).

## Dependencies

- spec-002, spec-003 (Diagnostic, Location).
- spec-011 (LintEngine — runner constructs an engine per case).
- spec-004, spec-005 (loaders — for cases with schema).

## Deliverables

- `crates/rglint-test-harness/src/{lib,fixture,expected,runner}.rs`.
- `crates/rglint-test-harness/src/snapshot.rs` (insta helper).
- `crates/rglint-test-harness/src/property.rs` (proptest helpers).
- A `tests/smoke.rs` proving the harness runs one hand-written fixture end-to-end.

## Interface / API

```rust
pub struct FixtureCase {
    pub id: String,
    pub source: String,
    pub schema: Option<String>,
    pub options: serde_json::Value,
    pub expected: Vec<ExpectedError>,
    pub valid: bool,
}

pub struct ExpectedError {
    pub rule: String,
    pub message: String,
    pub line: usize,
    pub column: usize, // 0-based, graphql-eslint style
}

pub fn load_fixture(dir: &Path) -> Result<FixtureCase>;
pub fn run_fixture(case: &FixtureCase, engine: &LintEngine) -> Result<()>;
#[macro_export] macro_rules! rglint_test_suite { ($rule_id:literal) => { ... } }

pub fn assert_diagnostic_snapshot(diagnostics: &[Diagnostic], source: &SourceFile);
pub fn prop_parse_roundtrip(src: &str) -> bool;
```

## Behavior

- `run_fixture` compares: error count, and for each error (matched by
  position order) the rule id, message **verbatim**, line (1-based), column
  (0-based via `location_eslint`). Mismatch → `pretty_assertions::assert_eq!`
  on a structured debug representation.
- `valid` cases assert zero diagnostics.
- Snapshot helper writes/updates `.snap` files alongside the test.
- Property helpers are `no_std`-compatible enough to run in CI fast mode.

## Testing

- `tests/smoke.rs`: a hand-rolled `no-anonymous-operations` invalid fixture
  (inline, not via xtask) → `run_fixture` passes; mutate the expected message
  → `run_fixture` fails with a diff (assert the failure mode is informative).
- Snapshot: a 2-diagnostic case produces a stable `.snap`.

## Risks / Notes

- Message verbatim parity is the strictest assertion; spec-053 (graphql-js spec
  rules) explicitly relaxes this — those fixtures use a `loose_message: true`
  flag in `.config.toml` so the runner compares rule+location only. Document
  the flag in the harness README.
