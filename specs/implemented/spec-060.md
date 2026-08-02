# Spec-060: GitHub annotations reporter

> Plan reference: §3 (`crates/rglint/src/reporter/github.rs`), §1 ("Reporters")

## Goal

Implement the `github` reporter: emit GitHub Actions workflow-command
annotations (`::error file=...,line=...,col=...::message`) so diagnostics
show up inline in PR diffs when rglint runs in a GitHub Actions workflow.

## Scope

**In scope:**

- `reporter::github` — for each `Diagnostic`, emit one line:
  ```
  ::error file=<path>,line=<line>,col=<col>::<rule_id>: <message>
  ```
  (`::warning` for `Severity::Warn`).
- Sort emitted annotations by normalized path, start location, rule id, and
  message so output is stable across result ordering.
- Multi-line spans: use `line` + `col` of the start (GitHub annotations
  support `line` + `endLine` + `col` + `endColumn` — emit all when the span
  crosses lines).
- Escape message text per GitHub's workflow-command rules (`%0A` for newline,
  `%0D` for CR, `%25` for `%`).
- Escape annotation properties (`%`, CR/LF, commas, and colons) so paths cannot
  change command metadata. Truncate messages at 1000 Unicode scalar values
  before escaping and append `…` when truncation occurs.
- No file header; GitHub renders annotations in the PR UI. A console summary is
  optional via a trailing count group and is configurable.

**Out of scope:**

- SARIF (spec-059 — for code-scanning upload).
- Pretty/JSON reporters.

## Dependencies

- spec-002, spec-003, spec-011.

## Deliverables

- `crates/rglint/src/reporter/github.rs`.
- Snapshot test for a multi-diagnostic fixture (assert exact annotation
  lines).

## Interface / API

```rust
pub struct GithubReporter { pub summary: bool }
impl Reporter for GithubReporter { ... }
```

## Behavior

- Paths relative to the workspace root (GitHub resolves relative to repo
  root; emit relative for portability). The reporter uses the current working
  directory as the workspace-root boundary and falls back to the original
  normalized path when it is outside that directory or unavailable.
- `col` and end coordinates are 1-based; end coordinates use the source span's
  exclusive end position and are emitted only for multi-line spans.
- Missing source entries use line 1, column 1 and still emit the diagnostic.
- `summary: true` → append `::group::Summary` + counts + `::endgroup::`.

## Testing

- Snapshot asserting exact escape behavior (a message with `%`, newline).
- A multi-line span emits `endLine`/`endColumn`.
- Unit coverage includes missing/suppressed diagnostics, Unicode-safe
  truncation, property escaping, and writer-error propagation.

## Risks / Notes

- GitHub annotation command limit is 1 command per line; very long messages
  are fine (no hard limit, but keep readable). Truncate beyond 1000 Unicode
  scalar values with `…`.

## Implementation notes

Implemented in `crates/rglint/src/reporter/github.rs` with the checked-in
`github-multi.graphql` source fixture and an insta snapshot. This reporter is a
pure adapter over `ProjectLintResult`; CLI format selection remains owned by
spec-062.
