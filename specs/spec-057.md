# Spec-057: Pretty reporter (miette)

> Plan reference: §3 (`crates/rglint/src/reporter/pretty.rs`), §4.5 ("miette renders"), §1 ("Reporters")

## Goal

Implement the default `pretty` reporter: human-friendly diagnostic output with
source snippets and `^^^` carets, powered by `miette`. This is the format
`insta` snapshots (spec-014) pin, so it doubles as the canonical rendering.

## Scope

**In scope:**

- `reporter::pretty` — convert each `Diagnostic` into a `miette::Diagnostic`
  impl carrying: message, severity label, `LabeledSpan` (from `Diagnostic.span` +
  `SourceFile`), rule id as `code`, suggestions as `help` text.
- Render to a `String` (or `Write`) using `miette::GraphicalReportHandler` (or
  `GraphicalTheme`).
- Aggregate per-file output with a file header and a summary line
  (`✖ N problems (M errors, W warnings)` — eslint-style; confirm exact
  wording/format).
- Exit-code helper lives in spec-062; this reporter only renders.

**Out of scope:**

- JSON/SARIF/GitHub reporters (specs 058-060).
- Color/TTY detection (delegate to miette's default; add a `--no-color` flag
  in spec-062 that sets `miette`'s color disabled).

## Dependencies

- spec-002 (SourceFile — needed to build `LabeledSpan` with source text).
- spec-003 (Diagnostic).
- spec-011 (ProjectLintResult — the input).

## Deliverables

- `crates/rglint/src/reporter/pretty.rs`.
- `crates/rglint/src/reporter/mod.rs` (Reporter trait: `render(result, out:
  &mut dyn Write)`).
- Snapshot tests (insta) for a curated set of diagnostics covering:
  single-error, multi-error same file, multi-file, a suggestion, an error
  spanning multiple lines.

## Interface / API

```rust
pub trait Reporter {
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> Result<()>;
}

pub struct PrettyReporter { color: bool }
impl Reporter for PrettyReporter { ... }
```

## Behavior

- Source snippet shows the offending line(s) with a caret underline matching
  the span's column range.
- File paths rendered relative to CWD (absolute if outside).
- Summary counts errors vs warnings by `Severity`.
- `color: false` → plain text (for CI logs / piped output).

## Testing

- Insta snapshots (the canonical reference; spec-014's snapshot helper
  reuses this renderer).
- Property: rendering never panics on an empty result or a zero-length span.

## Risks / Notes

- miette's `fancy-no-backtrace` feature (PLAN §2) keeps output lean; verify it
  renders source snippets without a backtrace section.
- miette label count limits — if a diagnostic has many labels, cap to the
  first N and note "+K more".
