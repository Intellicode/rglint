//! `insta` snapshot helper: render a slice of [`Diagnostic`]s over their
//! source as a `^^^`-caret diagram (the format the `pretty` reporter uses,
//! spec-057). spec-014's deliverable is the harness scaffolding; the
//! full-blown pretty reporter lives in spec-057 and builds on this.
//!
//! The rendered snapshot is a single string containing the source text, with
//! each diagnostic's offending line followed by a caret line whose `^^^` run
//! covers the span's columns, and a trailing `> {rule}: {message}` annotation.
//! Multi-diagnostics are annotated in source order.

use rglint_core::{Diagnostic, SourceFile};

/// Render `diagnostics` over `source` and `insta`-assert the result as a
/// snapshot named after the caller's test (insta picks the `.snap` path from
/// the test's module path automatically).
///
/// The format mirrors the `pretty` reporter (spec-057): for each diagnostic,
/// the offending line is printed followed by a caret line whose `^^^` run
/// covers the span's columns, then a `> {rule}: {message}` line. Diagnostics
/// are emitted in source order (line, then column) so the snapshot is stable
/// across runs.
///
/// Diagnostics whose `file` does not match `source.path()` are rendered with a
/// bare `> {rule}: {message}` line (no caret) at the end of the snapshot, so a
/// mismatched attribution is still visible rather than silently dropped.
pub fn assert_diagnostic_snapshot(diagnostics: &[Diagnostic], source: &SourceFile) {
    let rendered = render_snapshot(diagnostics, source);
    insta::assert_snapshot!(rendered);
}

/// Render the same string [`assert_diagnostic_snapshot`] would write into the
/// `.snap`. Exposed so spec-057's pretty reporter can reuse the exact format
/// and so tests can assert specifics without going through `insta`.
pub fn render_snapshot(diagnostics: &[Diagnostic], source: &SourceFile) -> String {
    // Split diagnostics into on-source (file matches) and off-source.
    let mut on_src: Vec<&Diagnostic> = Vec::new();
    let mut off_src: Vec<&Diagnostic> = Vec::new();
    for d in diagnostics {
        if d.file == source.path() {
            on_src.push(d);
        } else {
            off_src.push(d);
        }
    }
    // Sort on-source diagnostics by (line, column) for stable output.
    on_src.sort_by_key(|d| {
        let (line, col, _end_line, _end_col) = source.location_eslint(d.span);
        (line, col)
    });

    let source_text = source.source();
    let lines: Vec<&str> = source_text.split_inclusive('\n').collect();
    // If the source has no trailing newline split_inclusive yields the last
    // line without one; that's fine for indexing.

    let mut out = String::new();
    for d in &on_src {
        let (line, col, end_line, end_col) = source.location_eslint(d.span);
        let line_idx = line.saturating_sub(1);
        let culprit = lines.get(line_idx).copied().unwrap_or("");
        // Strip a trailing newline for the printed line; we add our own.
        let culprit_trim = culprit.trim_end_matches('\n');
        out.push_str(culprit_trim);
        out.push('\n');

        // Build the caret line. If the span is multi-line, draw carets across
        // each covered line.
        if end_line == line {
            // Same line: `col`..`end_col` (0-based, exclusive end).
            let start = col.min(end_col);
            let end = end_col.max(col);
            let mut caret = String::new();
            for _ in 0..start {
                caret.push(' ');
            }
            for _ in start..end {
                caret.push('^');
            }
            if caret.len() == start {
                // Zero-length span: no `^` was emitted (start == end), so
                // rebuild the caret line as `col` spaces + a single `^` so the
                // location is visible rather than an all-spaces line.
                caret.clear();
                for _ in 0..col {
                    caret.push(' ');
                }
                caret.push('^');
            }
            out.push_str(&caret);
        } else {
            // Multi-line: caret from `col` to EOL on this line; subsequent lines
            // get full-line carets.
            let mut caret = String::new();
            for _ in 0..col {
                caret.push(' ');
            }
            let line_len = culprit_trim.chars().count();
            // Use char count for the caret width (matches what the reader sees);
            // col is byte-based but for ASCII (the overwhelming majority of
            // fixtures) this is identical.
            for _ in col..line_len {
                caret.push('^');
            }
            out.push_str(&caret);
            // middle lines (strictly between line and end_line) get full carets.
            for mid in (line_idx + 1)..end_line.saturating_sub(1) {
                if let Some(ml) = lines.get(mid) {
                    let ml_trim = ml.trim_end_matches('\n');
                    if !ml_trim.is_empty() {
                        out.push('\n');
                        let mut c = String::new();
                        for _ in 0..ml_trim.chars().count() {
                            c.push('^');
                        }
                        out.push_str(&c);
                    }
                }
            }
            // end line: carets up to end_col.
            if let Some(_el) = lines.get(end_line.saturating_sub(1)) {
                out.push('\n');
                let mut c = String::new();
                for _ in 0..end_col {
                    c.push('^');
                }
                out.push_str(&c);
            }
        }
        out.push('\n');
        out.push_str(&format!("> {}: {}", d.rule_id, d.message));
        out.push('\n');
    }

    for d in &off_src {
        out.push_str(&format!(
            "> {} (in {}): {}",
            d.rule_id,
            d.file.display(),
            d.message
        ));
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{DiagnosticBuilder, Span};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn src_file(text: &str) -> Arc<SourceFile> {
        SourceFile::new(PathBuf::from("test.graphql"), text.to_owned())
    }

    #[test]
    fn renders_single_line_caret_and_message() {
        let src = src_file("query { hero }\n");
        // span covers `hero` (cols 8..12 on line 1; 0-based).
        let diag = DiagnosticBuilder::new(
            "no-anonymous-operations",
            PathBuf::from("test.graphql"),
            Span::new(8, 4),
            "Anonymous operation",
        )
        .finish();
        let rendered = render_snapshot(std::slice::from_ref(&diag), &src);
        let expected =
            "query { hero }\n        ^^^^\n> no-anonymous-operations: Anonymous operation\n";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn renders_zero_length_span_as_single_caret() {
        let src = src_file("foo bar baz\n");
        // 0-length span at col 4 (between "foo " and "bar").
        let diag =
            DiagnosticBuilder::new("r", PathBuf::from("test.graphql"), Span::new(4, 0), "boom")
                .finish();
        let rendered = render_snapshot(std::slice::from_ref(&diag), &src);
        assert_eq!(rendered, "foo bar baz\n    ^\n> r: boom\n");
    }

    #[test]
    fn sorts_diagnostics_by_line_then_column() {
        let src = src_file("a b\nc d\n");
        let d1 = DiagnosticBuilder::new(
            "r1",
            PathBuf::from("test.graphql"),
            Span::new(6, 1),
            "second",
        )
        .finish();
        let d0 = DiagnosticBuilder::new(
            "r0",
            PathBuf::from("test.graphql"),
            Span::new(0, 1),
            "first",
        )
        .finish();
        let rendered = render_snapshot(&[d1, d0], &src);
        // "first" (line 1) should come before "second" (line 2) despite input order.
        let first_idx = rendered.find("> r0: first").unwrap();
        let second_idx = rendered.find("> r1: second").unwrap();
        assert!(first_idx < second_idx);
    }

    #[test]
    fn off_source_diagnostics_render_with_file_annotation() {
        let src = src_file("foo\n");
        let diag = DiagnosticBuilder::new(
            "r",
            PathBuf::from("other.graphql"),
            Span::new(0, 1),
            "elsewhere",
        )
        .finish();
        let rendered = render_snapshot(std::slice::from_ref(&diag), &src);
        assert!(rendered.contains("> r (in other.graphql): elsewhere"));
    }
}
