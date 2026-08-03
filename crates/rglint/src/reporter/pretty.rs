//! Human-readable miette reporter.

use std::fmt::{self, Display};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miette::{
    Diagnostic as MietteDiagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan,
    NamedSource, Severity as MietteSeverity, SourceCode,
};
use rglint_core::{Diagnostic, ProjectLintResult, Severity, SourceFile, Span};

use super::Reporter;

/// The default human-readable reporter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrettyReporter {
    /// Whether miette should use ANSI styling.
    pub color: bool,
    /// Whether to append the problems summary.
    pub summary: bool,
}

impl PrettyReporter {
    /// Construct a reporter with explicit color behavior.
    pub const fn new(color: bool) -> Self {
        Self {
            color,
            summary: true,
        }
    }

    /// Construct a reporter with explicit color and summary behavior.
    pub const fn new_with_summary(color: bool, summary: bool) -> Self {
        Self { color, summary }
    }

    fn handler(&self) -> GraphicalReportHandler {
        let theme = if self.color {
            GraphicalTheme::unicode()
        } else {
            GraphicalTheme::unicode_nocolor()
        };
        GraphicalReportHandler::new_themed(theme)
            .without_cause_chain()
            .with_urls(false)
            .without_primary_span_start()
    }
}

impl Default for PrettyReporter {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Reporter for PrettyReporter {
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> io::Result<()> {
        let mut errors = 0;
        let mut warnings = 0;
        let cwd = std::env::current_dir().ok();
        let handler = self.handler();

        for result in results {
            let mut by_file: Vec<(&PathBuf, Vec<&Diagnostic>)> = result
                .all
                .iter()
                .filter(|diagnostic| diagnostic.severity != Severity::Off)
                .fold(Vec::new(), |mut groups, diagnostic| {
                    if let Some((_, diagnostics)) = groups
                        .iter_mut()
                        .find(|(path, _)| path.as_path() == diagnostic.file.as_path())
                    {
                        diagnostics.push(diagnostic);
                    } else {
                        groups.push((&diagnostic.file, vec![diagnostic]));
                    }
                    match diagnostic.severity {
                        Severity::Error => errors += 1,
                        Severity::Warn => warnings += 1,
                        Severity::Off => {}
                    }
                    groups
                });

            // `all` is already stable-sorted by the engine; preserve that
            // order while making the file grouping explicit.
            for (path, diagnostics) in by_file.drain(..) {
                writeln!(out, "{}:", display_path(path, cwd.as_deref()))?;
                for diagnostic in diagnostics {
                    let source = result.sources.get(path).map(|source| source.as_ref());
                    let adapted = MietteDiagnosticAdapter::new(diagnostic, source, cwd.as_deref());
                    let mut rendered = String::new();
                    handler
                        .render_report(&mut rendered, &adapted)
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    out.write_all(rendered.as_bytes())?;
                    if !rendered.ends_with('\n') {
                        writeln!(out)?;
                    }
                }
            }
        }

        if self.summary {
            writeln!(
                out,
                "✖ {} problems ({} errors, {} warnings)",
                errors + warnings,
                errors,
                warnings
            )?;
        }
        Ok(())
    }
}

/// Adapt the engine's source-independent diagnostic into miette's rendering
/// protocol. The source is owned here so the adapter remains valid while the
/// graphical handler borrows it.
struct MietteDiagnosticAdapter {
    message: String,
    rule_id: String,
    severity: MietteSeverity,
    help: Option<String>,
    source: Option<NamedSource<String>>,
    span: Span,
}

impl MietteDiagnosticAdapter {
    fn new(diagnostic: &Diagnostic, source: Option<&SourceFile>, cwd: Option<&Path>) -> Self {
        let span = source
            .map(|source| clamp_span(source.source(), diagnostic.span))
            .unwrap_or(diagnostic.span);
        let source = source.map(|source| {
            NamedSource::new(display_path(source.path(), cwd), source.source().to_owned())
        });
        let help = (!diagnostic.suggestions.is_empty()).then(|| {
            diagnostic
                .suggestions
                .iter()
                .map(|suggestion| suggestion.desc.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        });
        Self {
            message: diagnostic.message.clone(),
            rule_id: diagnostic.rule_id.clone(),
            severity: match diagnostic.severity {
                Severity::Error => MietteSeverity::Error,
                Severity::Warn | Severity::Off => MietteSeverity::Warning,
            },
            help,
            source,
            span,
        }
    }
}

impl Display for MietteDiagnosticAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl fmt::Debug for MietteDiagnosticAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MietteDiagnosticAdapter")
            .field("rule_id", &self.rule_id)
            .field("message", &self.message)
            .finish()
    }
}

impl std::error::Error for MietteDiagnosticAdapter {}

impl MietteDiagnostic for MietteDiagnosticAdapter {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(self.rule_id.as_str()))
    }

    fn severity(&self) -> Option<MietteSeverity> {
        Some(self.severity)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.help
            .as_deref()
            .map(|help| Box::new(help) as Box<dyn Display + 'a>)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.source.as_ref().map(|source| source as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.source.as_ref().map(|_| {
            Box::new(std::iter::once(LabeledSpan::new_primary_with_span(
                None,
                (self.span.offset, self.span.len),
            ))) as Box<dyn Iterator<Item = LabeledSpan>>
        })
    }
}

fn display_path(path: &Path, cwd: Option<&Path>) -> String {
    if path.is_relative() {
        return path.display().to_string();
    }
    cwd.and_then(|cwd| path.strip_prefix(cwd).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn clamp_span(source: &str, span: Span) -> Span {
    let end = span.end().min(source.len());
    let mut start = span.offset.min(end);
    while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = end;
    while end > start && !source.is_char_boundary(end) {
        end -= 1;
    }
    Span::new(start, end.saturating_sub(start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{DiagnosticBuilder, Fix};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn source(path: &str, text: &str) -> Arc<SourceFile> {
        SourceFile::new(PathBuf::from(path), text.to_owned())
    }

    fn result(source: Arc<SourceFile>, diagnostics: Vec<Diagnostic>) -> ProjectLintResult {
        let mut sources = HashMap::new();
        sources.insert(source.path().to_path_buf(), source);
        ProjectLintResult {
            project_name: "test".to_owned(),
            by_file: HashMap::new(),
            all: diagnostics,
            sources,
        }
    }

    fn render(results: &[ProjectLintResult]) -> String {
        let mut output = Vec::new();
        PrettyReporter::new(false)
            .render(results, &mut output)
            .unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn snapshots_curated_diagnostic_shapes() {
        let source = source(
            "pretty.graphql",
            "query Example {\n  hero {\n    name\n  }\n}\n",
        );
        let single = DiagnosticBuilder::new(
            "no-name",
            source.path().to_path_buf(),
            Span::new(29, 4),
            "Name is not allowed",
        )
        .severity(Severity::Error)
        .finish();
        let suggestion = DiagnosticBuilder::new(
            "rename",
            source.path().to_path_buf(),
            Span::new(29, 4),
            "Use `fullName`",
        )
        .suggestion(
            "Rename the field",
            Fix::Replace {
                span: Span::new(29, 4),
                text: "fullName".to_owned(),
            },
        )
        .finish();
        let multi_line = DiagnosticBuilder::new(
            "balanced",
            source.path().to_path_buf(),
            Span::new(25, 13),
            "Selection must be balanced",
        )
        .severity(Severity::Error)
        .finish();
        insta::assert_snapshot!(
            "single-error-and-suggestion",
            render(&[result(source.clone(), vec![single, suggestion, multi_line]),])
        );
    }

    #[test]
    fn empty_and_zero_length_results_are_total() {
        let empty = ProjectLintResult::default();
        let source = source("empty.graphql", "query { hero }\n");
        let zero = DiagnosticBuilder::new(
            "at-point",
            source.path().to_path_buf(),
            Span::new(7, 0),
            "Expected a name",
        )
        .finish();
        let output = render(&[empty, result(source, vec![zero])]);
        assert!(output.contains("✖ 1 problems (0 errors, 1 warnings)"));
        assert!(output.contains("at-point"));
    }

    #[test]
    fn snapshots_multiple_files_and_summary_counts() {
        let first = source("first.graphql", "query First { hero }\n");
        let second = source("second.graphql", "query Second { villain }\n");
        let first_diag = DiagnosticBuilder::new(
            "first-rule",
            first.path().to_path_buf(),
            Span::new(14, 4),
            "First field is discouraged",
        )
        .finish();
        let second_diag = DiagnosticBuilder::new(
            "second-rule",
            second.path().to_path_buf(),
            Span::new(15, 7),
            "Second field is invalid",
        )
        .severity(Severity::Error)
        .finish();
        insta::assert_snapshot!(
            "multiple-files",
            render(&[
                result(first, vec![first_diag]),
                result(second, vec![second_diag]),
            ])
        );
    }

    #[test]
    fn relative_path_is_used_for_sources_inside_cwd() {
        let source_path = std::env::current_dir().unwrap().join("nested.graphql");
        let source = source(source_path.to_str().unwrap(), "query { hero }\n");
        let diagnostic = DiagnosticBuilder::new(
            "rule",
            source.path().to_path_buf(),
            Span::new(8, 4),
            "bad field",
        )
        .finish();
        let output = render(&[result(source, vec![diagnostic])]);
        assert!(output.contains("nested.graphql:"));
        assert!(!output.contains(&format!("{}:", source_path.display())));
    }
}
