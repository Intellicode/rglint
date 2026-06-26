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
- Multi-line spans: use `line` + `col` of the start (GitHub annotations
  support `line` + `endLine` + `col` + `endColumn` — emit all when the span
  crosses lines).
- Escape message text per GitHub's workflow-command rules (`%0A` for newline,
  `%0D` for CR, `%25` for `%`).
- No file header / summary (GitHub renders annotations in the PR UI; console
  summary optional via a trailing count line, configurable).

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
  root; emit relative for portability).
- `col` is 1-based (GitHub convention).
- `summary: true` → append `::group::Summary` + counts + `::endgroup::`.

## Testing

- Snapshot asserting exact escape behavior (a message with `%`, newline).
- A multi-line span emits `endLine`/`endColumn`.

## Risks / Notes

- GitHub annotation command limit is 1 command per line; very long messages
  are fine (no hard limit, but keep readable). Truncate >1000 chars with `…`.
