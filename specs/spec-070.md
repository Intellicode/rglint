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
  `target/coverage/cobertura.xml` and fails if workspace coverage < 85% or
  any `rglint-rules` module < 90% (PLAN §6.7).
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

- `tests/invariants.rs`.
- `tests/negative_paths.rs` (rstest-parametric).
- `scripts/coverage-gate.sh`.
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

- Coverage gate reads `cobertura.xml` line-rate attributes; compares against
  thresholds; prints per-crate breakdown; exits non-zero on failure.
- Invariant tests use the test harness (spec-014) to build configs + fixtures
  inline.
- Negative-path: each rule gets a known-bad input (a list maintained in
  `tests/negative_paths.json` keyed by rule id); the test loads it, runs the
  engine, asserts `!panicked && diagnostics.len() >= 1`.

## Testing

- These *are* the tests; correctness = they pass + the gate script exits 0
  on a green commit.

## Risks / Notes

- The 85% floor may be unattainable early; start with a 60% floor and raise
  incrementally, recording the target in `scripts/coverage-gate.sh`. Document
  the ratchet in `docs/contributing.md`.
- `rstest` parametrization over all rules: if a rule lacks a bad-input entry,
  the test is skipped with a warning (don't fail on missing negative-input
  data — that's a coverage-gap, not a regression).
