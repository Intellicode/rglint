//! GitHub Actions workflow-command annotations.

use std::cmp::Ordering;
use std::io::{self, Write};
use std::path::Path;

use rglint_core::{Diagnostic, Location, ProjectLintResult, Severity, SourceFile};

use super::Reporter;

const MAX_MESSAGE_CHARS: usize = 1_000;

/// Reporter for GitHub Actions annotations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GithubReporter {
    /// Whether to append a grouped error/warning summary.
    pub summary: bool,
}

impl GithubReporter {
    /// Construct a reporter with the requested summary behavior.
    pub const fn new(summary: bool) -> Self {
        Self { summary }
    }
}

impl Default for GithubReporter {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Reporter for GithubReporter {
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> io::Result<()> {
        let workspace_root = std::env::current_dir().ok();
        let mut annotations = Vec::new();
        let mut errors = 0;
        let mut warnings = 0;

        for result in results {
            for diagnostic in &result.all {
                match diagnostic.severity {
                    Severity::Error => errors += 1,
                    Severity::Warn => warnings += 1,
                    Severity::Off => continue,
                }

                let source = result
                    .sources
                    .get(&diagnostic.file)
                    .map(|source| source.as_ref());
                annotations.push(Annotation::from_diagnostic(
                    diagnostic,
                    source,
                    workspace_root.as_deref(),
                ));
            }
        }

        annotations.sort_by(Annotation::cmp);
        for annotation in annotations {
            writeln!(out, "{annotation}")?;
        }

        if self.summary {
            let total = errors + warnings;
            writeln!(out, "::group::Summary")?;
            writeln!(
                out,
                "{total} problems ({errors} errors, {warnings} warnings)"
            )?;
            writeln!(out, "::endgroup::")?;
        }

        Ok(())
    }
}

struct Annotation {
    kind: &'static str,
    file: String,
    location: Location,
    rule_id: String,
    message: String,
}

impl Annotation {
    fn from_diagnostic(
        diagnostic: &Diagnostic,
        source: Option<&SourceFile>,
        workspace_root: Option<&Path>,
    ) -> Self {
        let (location, path) = source
            .map(|source| {
                (
                    source.location(diagnostic.span),
                    relative_path(source.path(), workspace_root),
                )
            })
            .unwrap_or_else(|| {
                (
                    Location {
                        line: 1,
                        column: 1,
                        end_line: 1,
                        end_column: 1,
                    },
                    relative_path(&diagnostic.file, workspace_root),
                )
            });

        Self {
            kind: match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warn | Severity::Off => "warning",
            },
            file: escape_property(&path),
            location,
            rule_id: diagnostic.rule_id.clone(),
            message: truncate_message(&diagnostic.message),
        }
    }

    fn cmp(left: &Self, right: &Self) -> Ordering {
        left.file
            .cmp(&right.file)
            .then_with(|| left.location.line.cmp(&right.location.line))
            .then_with(|| left.location.column.cmp(&right.location.column))
            .then_with(|| left.rule_id.cmp(&right.rule_id))
            .then_with(|| left.message.cmp(&right.message))
    }
}

impl std::fmt::Display for Annotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "::{} file={},line={},col={}",
            self.kind, self.file, self.location.line, self.location.column
        )?;
        if self.location.line != self.location.end_line {
            write!(
                f,
                ",endLine={},endColumn={}",
                self.location.end_line, self.location.end_column
            )?;
        }
        write!(
            f,
            "::{}",
            escape_message(&format!("{}: {}", self.rule_id, self.message))
        )
    }
}

fn relative_path(path: &Path, workspace_root: Option<&Path>) -> String {
    workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Escape workflow-command properties, including delimiters that would alter
/// the annotation command's metadata.
fn escape_property(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

/// Escape the message channel after truncation so literal percent sequences
/// cannot be interpreted as workflow-command escapes.
fn escape_message(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn truncate_message(message: &str) -> String {
    let mut chars = message.chars();
    let truncated: String = chars.by_ref().take(MAX_MESSAGE_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{DiagnosticBuilder, Span};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn source(path: &str) -> Arc<SourceFile> {
        // The spans below are pinned to LF byte offsets. Git may check this
        // fixture out with CRLF on Windows, so normalize it before indexing.
        let text = include_str!("fixtures/github-multi.graphql").replace("\r\n", "\n");
        SourceFile::new(path.into(), text)
    }

    fn result(sources: Vec<Arc<SourceFile>>, diagnostics: Vec<Diagnostic>) -> ProjectLintResult {
        let sources = sources
            .into_iter()
            .map(|source| (source.path().to_path_buf(), source))
            .collect();
        ProjectLintResult {
            project_name: "test".to_owned(),
            by_file: HashMap::new(),
            all: diagnostics,
            sources,
        }
    }

    fn render(reporter: GithubReporter, results: &[ProjectLintResult]) -> String {
        let mut output = Vec::new();
        reporter.render(results, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn snapshot_matches_annotations_and_summary() {
        let first = source("z.graphql");
        let second = source("a.graphql");
        let first_error = DiagnosticBuilder::new(
            "no-hero",
            first.path().to_path_buf(),
            Span::new(14, 16),
            "Hero is 100% discouraged\nkeep out",
        )
        .severity(Severity::Error)
        .finish();
        let second_warning = DiagnosticBuilder::new(
            "no-villain",
            second.path().to_path_buf(),
            Span::new(25, 7),
            "Villain is discouraged",
        )
        .finish();

        insta::assert_snapshot!(render(
            GithubReporter::new(true),
            &[result(
                vec![first, second],
                vec![first_error, second_warning]
            )]
        ));
    }

    #[test]
    fn missing_sources_and_suppressed_diagnostics_are_total() {
        let path = PathBuf::from("missing.graphql");
        let suppressed =
            DiagnosticBuilder::new("off-rule", path.clone(), Span::new(40, 2), "Suppressed")
                .severity(Severity::Off)
                .finish();
        let missing = DiagnosticBuilder::new(
            "missing-source",
            path,
            Span::new(40, 2),
            "Source was not retained",
        )
        .severity(Severity::Error)
        .finish();
        assert_eq!(
            render(
                GithubReporter::default(),
                &[result(Vec::new(), vec![suppressed, missing])]
            ),
            "::error file=missing.graphql,line=1,col=1::missing-source: Source was not retained\n"
        );
    }

    #[test]
    fn truncates_long_messages_without_splitting_utf8() {
        let message = "é".repeat(MAX_MESSAGE_CHARS + 1);
        let truncated = truncate_message(&message);
        assert_eq!(truncated.chars().count(), MAX_MESSAGE_CHARS + 1);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn escapes_workflow_command_property_and_message_delimiters() {
        assert_eq!(
            escape_property("folder,with:delimiters%\r\n.graphql"),
            "folder%2Cwith%3Adelimiters%25%0D%0A.graphql"
        );
        assert_eq!(escape_message("100%\r\nready"), "100%25%0D%0Aready");
    }

    #[test]
    fn writer_errors_are_propagated() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = GithubReporter::new(true).render(&[], &mut FailingWriter);
        assert_eq!(error.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }
}
