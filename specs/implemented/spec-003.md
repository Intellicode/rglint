# Spec-003: Diagnostics model

> Plan reference: §3 (`crates/rglint-core/src/diagnostics.rs`), §4.5

## Goal

Define the in-engine diagnostic data model: `Diagnostic`, `Severity`,
`Suggestion`, `Fix`. This is what rules produce via `RuleContext::report` and
what reporters consume. Kept independent of `miette` (miette is a *renderer*,
not the model — spec-057 adapts `Diagnostic` → `miette::Report`).

## Scope

**In scope:**

- `Severity` enum: `Off`, `Warn`, `Error` (mirrors eslint).
- `Diagnostic` struct: rule_id, file, span, message, severity, suggestions,
  data (JSON for rule-specific payload, e.g. deprecated reason).
- `Suggestion`: description + `Fix`.
- `Fix` enum: `Replace { span, text }`, `Insert { offset, text }`,
  `Remove { span }`.
- `DiagnosticBuilder` for fluent construction inside rules.
- `Diagnostic` is `Clone + Send + Sync + serde::Serialize/Deserialize` (for
  JSON reporter).

**Out of scope:**

- `miette` rendering (spec-057).
- `RuleContext` (spec-009).

## Dependencies

- spec-002 (Span, Location, SourceFile).

## Deliverables

- `crates/rglint-core/src/diagnostics.rs`.
- Unit tests: serialize a `Diagnostic` to JSON, round-trip.

## Interface / API

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Severity { Off, Warn, Error }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule_id: String,
    pub file: PathBuf,
    pub span: Span,
    pub message: String,
    pub severity: Severity,
    pub suggestions: Vec<Suggestion>,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub desc: String,
    pub fix: Fix,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Fix {
    Replace { span: Span, text: String },
    Insert { offset: usize, text: String },
    Remove { span: Span },
}

pub struct DiagnosticBuilder { /* private */ }
impl DiagnosticBuilder {
    pub fn new(rule_id: &str, file: PathBuf, span: Span, message: impl Into<String>) -> Self;
    pub fn severity(mut self, s: Severity) -> Self;
    pub fn suggestion(mut self, desc: impl Into<String>, fix: Fix) -> Self;
    pub fn data(mut self, v: serde_json::Value) -> Self;
    pub fn finish(self) -> Diagnostic;
}
```

## Behavior

- `Diagnostic::span` always carries a real span; if a rule has no node, it uses
  a zero-length span at file offset 0 (and a `TODO` note for column parity).
- `Severity::Off` diagnostics are filtered out by the engine before reporting
  (engine concern, spec-011), but the model permits them so config can downgrade.
- `Fix::Replace` text may be empty (equivalent to `Remove`).

## Testing

- Round-trip JSON serialization asserts field names match `01.expected.json`
  schema from PLAN.md §6.1:
  `{ "rule": ..., "message": ..., "line": ..., "column": ... }` — provide a
  `Diagnostic::to_parity_json(source: &SourceFile)` helper here or in spec-014.
  (Decide: put `to_parity_json` in the test harness, spec-014, to keep core
  test-agnostic. This spec only provides `Serialize`.)

## Risks / Notes

- Keep `Diagnostic` free of `miette` types so the core compiles without miette
  in WASM builds later (PLAN §2 mentions WASM as later phase).
