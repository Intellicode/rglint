# Spec-065: Benchmarks (criterion)

> Plan reference: §3 (`benches/`), §5 Phase 9, §6.2 (Performance layer), §8 (perf-regression risk)

## Goal

Set up `criterion` benchmarks to detect ≥10% performance regressions vs a
pinned baseline, per PLAN §6.2 / §8. Covers parser throughput, full-document
lint throughput, and per-rule micro-benchmarks over real-world corpora.

## Scope

**In scope:**

- `benches/parser.rs` — parse a corpus of schemas/operations vs an
  apollo-parser-only baseline (isolates engine overhead).
- `benches/linters.rs` — full `LintEngine::lint` over a corpus with the
  `recommended` preset (spec-063).
- `benches/corpora/` — vendored, license-clean schemas shaped like large
  real-world APIs (GitHub, Shopify, etc. — pick 2-3 representative corpora;
  record provenance and license decisions beside the inputs).
- Per-rule micro-bench: a deterministic loop registers one Criterion benchmark
  per rule in the recommended preset over the largest checked-in corpus. This
  keeps the benchmark list linked to the live preset instead of duplicating its
  rule ids in a macro.
- CI integration: `cargo bench --no-run` (compile-only in CI per PLAN §9);
  full benches run on a scheduled nightly job and post a PR comment with
  regressions.
- Regression gate: a `benches/baseline.json` (criterion's `--save-baseline
  pinned`); a script compares `--bench` vs baseline and fails if any group
  regresses >10%.

**Out of scope:**

- Memory benchmarks (criterion doesn't cover; defer to a heap-profiling
  stretch).
- WASM benchmarks.

## Dependencies

- spec-011 (engine), spec-063 (preset), spec-001 (workspace).

## Deliverables

- `benches/{parser,linters}.rs`.
- `benches/corpora/*` (vendored + license-noted).
- `benches/baseline.json` (committed; regenerated on deliberate re-pin).
- `benches/compare.sh` (regression gate script).
- `xtask bench` convenience wrapper (optional).

## Interface / API

```
cargo bench --bench parser
cargo bench --bench linters
./benches/compare.sh   # exits non-zero on >10% regression vs baseline
```

## Behavior

- Criterion group names are stable (`parse/shopify-schema`,
  `lint/recommended-github-schema`, and `rule/<rule-id>`).
- `compare.sh` uses `critcmp` (or criterion's JSON output) to compare
  median ns/iter; ±10% tolerance.
- Corpora files are read-only inputs; benches don't mutate them.

## Testing

- The benchmark *is* the test; correctness gate is `cargo bench --no-run`
  passing in CI.
- `compare.sh` self-test: run twice on the same code → exit 0.

## Risks / Notes

- §8 risk: "Performance regression hidden by complexity." Per-rule benches
  isolate the culprit rule; the full-lint bench catches cross-cutting
  regressions. Pin the baseline on a known-good commit before enabling the
  gate.
- Noisy CI runners: criterion gathers enough samples to be robust; the 10%
  threshold avoids flapping. Run the gate on a dedicated (non-shared) runner
  if available.

## Implementation note

The checked-in GitHub- and Shopify-shaped corpora are first-party benchmark
inputs rather than copied service introspection dumps. This deliberate scope
difference keeps the benchmark reproducible offline and avoids introducing
unclear third-party redistribution terms while preserving the schema size,
connection patterns, and operation shapes that exercise the engine's hot paths.
