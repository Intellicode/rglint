# Spec-058: JSON reporter

> Plan reference: §3 (`crates/rglint/src/reporter/json.rs`), §1 ("Reporters")

## Goal

Implement the `json` reporter: emit diagnostics as a JSON array (or
eslint-style `--format json` object) suitable for machine consumption and the
parity harness (spec-069 diffs this against graphql-eslint's JSON output).

## Scope

**In scope:**

- `reporter::json` — serialize `&[ProjectLintResult]` to a JSON document
  matching eslint's `--format json` shape:
  ```json
  [
    { "filePath": "...", "messages": [
      { "ruleId": "...", "severity": 2, "message": "...", "line": 1, "column": 0, "endLine": 1, "endColumn": 5 }
    ], "errorCount": 1, "warningCount": 0 }
  ]
  ```
  (eslint uses `severity: 2` for error, `1` for warn — match this, not rglint's
  enum).
- Pretty (2-space) and compact modes (flag in spec-062).
- Stable key order (alphabetical or eslint-compatible) for diff stability.

**Out of scope:**

- SARIF (spec-059), GitHub annotations (spec-060).
- The pretty reporter (spec-057).

## Dependencies

- spec-002, spec-003 (Location/Diagnostic — for line/column mapping).
- spec-011 (ProjectLintResult).

## Deliverables

- `crates/rglint/src/reporter/json.rs`.
- Snapshot of the JSON output for a fixed fixture (used as the parity-harness
  contract).

The reporter owns a private JSON projection of diagnostics. It resolves
locations from each result's retained `SourceFile` index and uses deterministic
file/key ordering; it does not reread source paths from disk or serialize the
engine's internal diagnostic shape directly.

## Interface / API

```rust
pub struct JsonReporter { pub pretty: bool }
impl Reporter for JsonReporter { ... }
```

## Behavior

- `severity`: `Error → 2`, `Warn → 1`, `Off → 0` (off never reaches here).
- `column`/`endColumn` are 0-based (eslint convention — use
  `location_eslint`).
- Empty result → `[]` (not `{}`).
- One array entry per file with diagnostics; files with zero diagnostics are
  omitted (matches eslint).

## Testing

- Snapshot test asserting byte-stable JSON.
- Round-trip: parse the output back into `serde_json::Value`, assert the
  expected field set.

## Risks / Notes

- This output is the contract `xtask check-parity` (spec-069) diffs against
  graphql-eslint's `--format json`; lock the shape here and update spec-069's
  comparator to match.
