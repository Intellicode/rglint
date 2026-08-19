# Spec-070: Coverage gate + cross-cutting invariant tests

> Plan reference: §6.6 (Cross-cutting invariants), §6.7 (Coverage gate), §6.5 (Negative-path)

## Goal

Two coupled concerns: (a) enforce the 85% line-coverage floor via tarpaulin
(PLAN §6.7), and (b) implement the cross-cutting invariant tests + negative-
path coverage that don't belong to any single rule (PLAN §6.5, §6.6).

## Scope

**In scope — coverage gate:**

- A `xtask coverage` (or `scripts/coverage.sh`) wrapper:
  `cargo tarpaulin --workspace --out xml --output-dir target/coverage`.
- A gate script `scripts/coverage-gate.sh` that parses
  `target/coverage/cobertura.xml`, targets 85% workspace coverage and 90% for
  each `rglint-rules` module, and enforces reviewed ratchets until those
  targets are reached (PLAN §6.7).
- CI wiring lives in spec-067; this spec owns the gate logic + thresholds.

**In scope — invariant tests:**

- `tests/invariants.rs` covering PLAN §6.6:
  1. Disabling a rule via config suppresses all its diagnostics.
  2. Parse errors yield `parse-error` diagnostics and abort rule execution
     for that file (other files still lint).
  3. A workspace with N schemas produces N independent lint passes (no
     cross-project contamination).
  4. `Severity::Off` rule produces zero diagnostics.
  5. `requires_schema` rule self-skips on a schema-less project.
  6. A rule's `option_schema` rejects malformed options at load (spec-056).
- Negative-path coverage (PLAN §6.5): for every rule, a parametric test
  (`rstest`) feeds a deliberately malformed input and asserts ≥1 diagnostic
  with no panic. A `macro_rules! negative_path!(rule_id)` enumerates rules.

**Out of scope:**

- Per-rule unit tests (each rule spec owns those).
- The tarpaulin CI job (spec-067).

## Dependencies

- spec-011 (engine — invariants exercise it).
- spec-054 (config — disable rule test).
- spec-056 (option validation invariant).
- spec-007 (multi-project invariant).
- All rule specs (negative-path enumerates them).

## Deliverables

- `crates/rglint/tests/invariants.rs`.
- `crates/rglint/tests/negative_paths.rs` (registry-driven macro
  parametrization; the workspace does not otherwise depend on `rstest`).
- `scripts/coverage-gate.sh`.
- `scripts/coverage-baseline.json` (reviewed module ratchets below the target).
- `xtask/src/coverage.rs` (thin wrapper) — optional.

## Interface / API

```rust
// tests/invariants.rs
#[test] fn disabling_rule_suppresses_diagnostics() { ... }
#[test] fn parse_errors_yield_parse_error_diagnostic() { ... }
#[test] fn multi_project_isolation() { ... }
// ...

// tests/negative_paths.rs
rstest::fixture! fn all_rules() -> Vec<&'static str> { ... }
#[rstest] fn no_panic_on_malformed_input(#[from all_rules] rule: &str) { ... }
```

## Behavior

- Coverage gate reads `cobertura.xml` source-line hits, compares the workspace
  and per-rule-module rates against their thresholds, prints the complete
  module breakdown, and exits non-zero on failure. Pre-existing modules below
  the 90% target use reviewed, checked-in ratchets that reject regressions in
  both covered-line count and rate; collector attribution exemptions pin their
  exact executable-line count and require review whenever it changes. CI
  publishes the passing Cobertura report as a required GitHub artifact.
- Invariant tests use the test harness (spec-014) to build configs + fixtures
  inline.
- Negative-path: each registered rule gets the shared malformed operation
  input; the test loads it, runs the engine, asserts no panic and at least one
  `parse-error` diagnostic. This deliberately tests the engine's common
  malformed-input boundary rather than pretending every rule has a distinct
  semantic malformed corpus.

## Testing

- These *are* the tests; correctness = they pass + the gate script exits 0
  on a green commit.

## Risks / Notes

- The 85% floor may be unattainable early; this implementation starts with a
  60% workspace floor and raises it incrementally, recording the target in
  `scripts/coverage-gate.sh` and `docs/contributing.md`. The per-module
  `rglint-rules` floor remains 90%.
- `rstest` parametrization over all rules: if a rule lacks a bad-input entry,
  the test is skipped with a warning (don't fail on missing negative-input
  data — that's a coverage-gap, not a regression).
