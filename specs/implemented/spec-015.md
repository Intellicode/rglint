# Spec-015: xtask port-fixture

> Plan reference: §3 (`xtask/src/port_fixture.rs`), §5 Phase 0.4, §6.1

## Goal

Implement the `xtask port-fixture` subcommand that reads
`packages/plugin/src/rules/<rule>/index.test.ts`, extracts the `{ valid: [...],
invalid: [...] }` test cases, and emits one `rules-fixtures/<rule-id>/{valid,invalid}/NN.{graphql,config.toml,expected.json}` triplet per case. This is the
bridge that turns the TS oracle into Rust-runnable fixtures.

## Scope

**In scope:**

- `xtask port-fixture --rule <id>` (or `--all`) subcommand.
- TS source parser: a pragmatic regex/line-based extractor (not a full TS
  parser) that finds `ruleTester.run({ valid: [...], invalid: [...] }, ...)`
  blocks. Accept imperfection — PLAN §5 Phase 0.4 says "Not perfectly; manual
  cleanup expected."
- For each `valid` case: emit `NN.graphql` (the `code` field) + `NN.config.toml`
  (`schema` + `options`).
- For each `invalid` case: same + `NN.expected.json` with the `errors` array
  (`message`, `line`, `column`; rule id filled from the test's rule).
- Schema extraction: graphql-eslint fixtures embed schema as a template
  literal; copy verbatim into the `.config.toml`'s `schema = """..."""`.
- Options extraction: copy the `options` object as TOML-ish (emit as a JSON
  blob inside `options = ...` using `serde_json` → pretty TOML via `toml` crate
  where possible; fall back to `options_json = "..."` string if conversion
  fails).
- A `manifest.json` per rule recording the source `.ts` file + line range, so
  re-runs are idempotent and humans can trace.

**Out of scope:**

- Running the fixtures (that's the harness, spec-014).
- Fixing imperfect extractions by hand (that's each rule spec's job during port).

## Dependencies

- spec-001 (xtask crate exists).
- spec-014 (fixture format must match what the harness reads — coordinate the
  `.config.toml` schema with this spec; lock the format here).

## Deliverables

- `xtask/src/port_fixture.rs`.
- `rules-fixtures/<rule-id>/` directories generated for **all 34 rules** on
  first `--all` run (committed to the repo).
- `rules-fixtures/<rule-id>/manifest.json` per rule.

## Interface / API

CLI:

```
xtask port-fixture --rule no-anonymous-operations
xtask port-fixture --all
xtask port-fixture --all --force   # overwrite existing
```

`01.config.toml` format (locked here):

```toml
schema = """
type Query { ... }
"""
options = { maxDepth = 7 }
loose_message = false   # set true for graphql-js spec rules (spec-053)
```

`01.expected.json`:

```json
{ "errors": [ { "rule": "no-anonymous-operations", "message": "...", "line": 2, "column": 0 } ] }
```

## Behavior

- Idempotent: re-running without `--force` skips cases whose source hash
  matches `manifest.json`.
- Missing `code` field → skip case + log; missing `errors` on an invalid case
  → emit empty `expected.json` and flag in a `port-fixture.log`.
- The extractor handles both `ruleTester.run(RULE, { valid, invalid })` and
  the object-literal-first form `ruleTester.run({ valid, invalid }, RULE)`.

## Testing

- Run `xtask port-fixture --rule no-anonymous-operations`; assert
  `rules-fixtures/no-anonymous-operations/` contains ≥ the number of cases in
  the TS file, each with a parseable `.config.toml`.
- Re-run without `--force` → no file writes.
- A deliberately hand-edited `manifest.json` with a wrong hash → re-run
  regenerates.

## Risks / Notes

- PLAN §8 risk: "JSON-schema option validation differs from TS-typed defaults"
  — also extract `meta.docs.configOptions` defaults here and write them to
  `rules-fixtures/<rule-id>/defaults.json` so the harness can fill missing
  options. Best-effort; not all rules have `configOptions`.
- The TS extractor is throwaway quality; document its known limitations in
  `xtask/src/port_fixture.rs` header so future maintainers don't trust it
  beyond its scope.
