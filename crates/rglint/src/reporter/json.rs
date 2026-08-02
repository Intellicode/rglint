//! Machine-readable ESLint-compatible JSON reporter.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;

use rglint_core::{Diagnostic, ProjectLintResult, Severity, SourceFile};
use serde::Serialize;

use super::Reporter;

/// A JSON reporter compatible with the shape emitted by ESLint's `json`
/// formatter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonReporter {
    /// Whether to indent the JSON document with two spaces.
    pub pretty: bool,
}

impl JsonReporter {
    /// Construct a reporter with the requested formatting mode.
    pub const fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Reporter for JsonReporter {
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> io::Result<()> {
        let mut files: BTreeMap<PathBuf, Vec<JsonDiagnostic>> = BTreeMap::new();

        for result in results {
            for diagnostic in &result.all {
                if diagnostic.severity == Severity::Off {
                    continue;
                }
                let source = result
                    .sources
                    .get(&diagnostic.file)
                    .map(|source| source.as_ref());
                files
                    .entry(diagnostic.file.clone())
                    .or_default()
                    .push(JsonDiagnostic::from_diagnostic(diagnostic, source));
            }
        }

        let output: Vec<JsonFile> = files
            .into_iter()
            .map(|(file_path, messages)| JsonFile::new(file_path, messages))
            .collect();

        if self.pretty {
            serde_json::to_writer_pretty(&mut *out, &output).map_err(io::Error::other)?;
        } else {
            serde_json::to_writer(&mut *out, &output).map_err(io::Error::other)?;
        }
        writeln!(out)
    }
}

#[derive(Debug, Serialize)]
struct JsonFile {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<JsonDiagnostic>,
    #[serde(rename = "errorCount")]
    error_count: usize,
    #[serde(rename = "warningCount")]
    warning_count: usize,
}

impl JsonFile {
    fn new(file_path: PathBuf, messages: Vec<JsonDiagnostic>) -> Self {
        let error_count = messages
            .iter()
            .filter(|message| message.severity == 2)
            .count();
        let warning_count = messages
            .iter()
            .filter(|message| message.severity == 1)
            .count();
        Self {
            file_path: file_path.to_string_lossy().into_owned(),
            messages,
            error_count,
            warning_count,
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonDiagnostic {
    #[serde(rename = "ruleId")]
    rule_id: String,
    severity: u8,
    message: String,
    line: usize,
    column: usize,
    #[serde(rename = "endLine")]
    end_line: usize,
    #[serde(rename = "endColumn")]
    end_column: usize,
}

impl JsonDiagnostic {
    fn from_diagnostic(diagnostic: &Diagnostic, source: Option<&SourceFile>) -> Self {
        let (line, column, end_line, end_column) = source
            .map(|source| source.location_eslint(diagnostic.span))
            .unwrap_or((1, 0, 1, 0));
        Self {
            rule_id: diagnostic.rule_id.clone(),
            severity: match diagnostic.severity {
                Severity::Error => 2,
                Severity::Warn => 1,
                Severity::Off => 0,
            },
            message: diagnostic.message.clone(),
            line,
            column,
            end_line,
            end_column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{DiagnosticBuilder, Span};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn source(path: &str, text: &str) -> Arc<SourceFile> {
        SourceFile::new(PathBuf::from(path), text.to_owned())
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

    fn render(reporter: JsonReporter, results: &[ProjectLintResult]) -> String {
        let mut output = Vec::new();
        reporter.render(results, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn snapshot_matches_eslint_shape_and_locations() {
        let first = source("01.graphql", "query Example {\n  hero\n}\n");
        let second = source("02.graphql", "query Other {\n  villain\n}\n");
        let error = DiagnosticBuilder::new(
            "no-hero",
            first.path().to_path_buf(),
            Span::new(18, 4),
            "Hero is not allowed",
        )
        .severity(Severity::Error)
        .finish();
        let warning = DiagnosticBuilder::new(
            "no-villain",
            second.path().to_path_buf(),
            Span::new(18, 7),
            "Villain is discouraged",
        )
        .finish();

        insta::assert_snapshot!(render(
            JsonReporter::new(true),
            &[result(vec![first, second], vec![error, warning])]
        ));
    }

    #[test]
    fn compact_output_round_trips_with_expected_field_sets() {
        let source = source("round-trip.graphql", "query Example { hero }\n");
        let diagnostic = DiagnosticBuilder::new(
            "no-hero",
            source.path().to_path_buf(),
            Span::new(16, 4),
            "Hero is not allowed",
        )
        .severity(Severity::Error)
        .finish();
        let output = render(
            JsonReporter::new(false),
            &[result(vec![source], vec![diagnostic])],
        );
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        let file = &value[0];
        let message = &file["messages"][0];
        assert_eq!(file["filePath"], "round-trip.graphql");
        assert_eq!(file["errorCount"], 1);
        assert_eq!(file["warningCount"], 0);
        assert_eq!(message["severity"], 2);
        assert_eq!(message["line"], 1);
        assert_eq!(message["column"], 16);
        assert_eq!(message["endLine"], 1);
        assert_eq!(message["endColumn"], 20);
        let fields = message
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            "ruleId",
            "severity",
            "message",
            "line",
            "column",
            "endLine",
            "endColumn",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(fields, expected);
    }

    #[test]
    fn empty_and_suppressed_results_render_as_empty_array() {
        let source = source("suppressed.graphql", "query Example { hero }\n");
        let diagnostic = DiagnosticBuilder::new(
            "disabled-rule",
            source.path().to_path_buf(),
            Span::new(16, 4),
            "Disabled",
        )
        .severity(Severity::Off)
        .finish();
        assert_eq!(
            render(JsonReporter::new(false), &[ProjectLintResult::default()]),
            "[]\n"
        );
        assert_eq!(
            render(
                JsonReporter::new(false),
                &[result(vec![source], vec![diagnostic])]
            ),
            "[]\n"
        );
    }
}
