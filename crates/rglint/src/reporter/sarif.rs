//! SARIF 2.1.0 reporter.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

use rglint_core::{Diagnostic, ProjectLintResult, Severity, SourceFile};
use serde::Serialize;

use super::Reporter;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const HELP_URI_BASE: &str = "https://rglint.dev/rules/";

/// Reporter for SARIF 2.1.0 consumers such as GitHub code scanning.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SarifReporter;

impl SarifReporter {
    /// Construct a SARIF reporter.
    pub const fn new() -> Self {
        Self
    }
}

impl Reporter for SarifReporter {
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> io::Result<()> {
        let report = SarifLog::from_results(results);
        serde_json::to_writer_pretty(&mut *out, &report).map_err(io::Error::from)?;
        writeln!(out)
    }
}

#[derive(Debug, Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Debug, Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize)]
struct SarifRule {
    id: String,
    name: String,
    #[serde(rename = "helpUri")]
    help_uri: String,
}

#[derive(Debug, Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: String,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Debug, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Debug, Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Debug, Serialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: usize,
    #[serde(rename = "startColumn")]
    start_column: usize,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ResultSortKey {
    uri: String,
    line: usize,
    column: usize,
    rule_id: String,
    message: String,
}

impl SarifLog {
    fn from_results(results: &[ProjectLintResult]) -> Self {
        let mut sarif_results = Vec::new();

        for result in results {
            for diagnostic in &result.all {
                if diagnostic.severity == Severity::Off {
                    continue;
                }

                let source = result
                    .sources
                    .get(&diagnostic.file)
                    .map(|source| source.as_ref());
                let uri = path_to_uri(&diagnostic.file);
                let (line, column) = start_location(diagnostic, source);
                sarif_results.push((
                    ResultSortKey {
                        uri: uri.clone(),
                        line,
                        column,
                        rule_id: diagnostic.rule_id.clone(),
                        message: diagnostic.message.clone(),
                    },
                    SarifResult {
                        rule_id: diagnostic.rule_id.clone(),
                        level: level(diagnostic.severity),
                        message: SarifMessage {
                            text: diagnostic.message.clone(),
                        },
                        locations: vec![SarifLocation {
                            physical_location: SarifPhysicalLocation {
                                artifact_location: SarifArtifactLocation { uri },
                                region: SarifRegion {
                                    start_line: line,
                                    start_column: column,
                                },
                            },
                        }],
                    },
                ));
            }
        }

        sarif_results.sort_by(|left, right| left.0.cmp(&right.0));
        let rules = sarif_results
            .iter()
            .map(|(_, result)| result.rule_id.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| SarifRule {
                id: id.to_owned(),
                name: id.to_owned(),
                help_uri: format!("{HELP_URI_BASE}{id}"),
            })
            .collect();
        let results = sarif_results
            .into_iter()
            .map(|(_, result)| result)
            .collect();

        Self {
            schema: SARIF_SCHEMA,
            version: SARIF_VERSION,
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "rglint",
                        version: env!("CARGO_PKG_VERSION"),
                        rules,
                    },
                },
                results,
            }],
        }
    }
}

fn start_location(diagnostic: &Diagnostic, source: Option<&SourceFile>) -> (usize, usize) {
    source
        .map(|source| {
            let location = source.location(diagnostic.span);
            (location.line, location.column)
        })
        // SARIF regions are 1-based. This fallback keeps diagnostics whose
        // source was not retained renderable without rereading the filesystem.
        .unwrap_or((1, 1))
}

fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Off => "none",
    }
}

fn path_to_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::with_capacity(normalized.len());
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':' | b'@')
        {
            uri.push(byte as char);
        } else {
            uri.push('%');
            uri.push(HEX[(byte >> 4) as usize] as char);
            uri.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    uri
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{DiagnosticBuilder, Span};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn source(path: &str, text: &str) -> Arc<SourceFile> {
        SourceFile::new(path.into(), text.to_owned())
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

    fn render(results: &[ProjectLintResult]) -> String {
        let mut output = Vec::new();
        SarifReporter::new().render(results, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn snapshot_matches_sarif_shape_and_locations() {
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

        insta::assert_snapshot!(render(&[
            result(vec![first, second], vec![error, warning],)
        ]));
    }

    #[test]
    fn output_validates_against_vendored_schema() {
        let source = source("schema.graphql", "type Query {\n  hero: String\n}\n");
        let diagnostic = DiagnosticBuilder::new(
            "no-hero",
            source.path().to_path_buf(),
            Span::new(16, 4),
            "Hero is not allowed",
        )
        .finish();
        let value: serde_json::Value =
            serde_json::from_str(&render(&[result(vec![source], vec![diagnostic])])).unwrap();
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../schemas/sarif-schema-2.1.0.json")).unwrap();
        let validator = jsonschema::JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(&schema)
            .unwrap();
        let errors = validator
            .validate(&value)
            .err()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "SARIF schema errors: {errors:#?}");
    }

    #[test]
    fn empty_suppressed_and_missing_sources_are_total() {
        let path = std::path::PathBuf::from("missing.graphql");
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
        .finish();
        let value: serde_json::Value = serde_json::from_str(&render(&[
            ProjectLintResult::default(),
            result(Vec::new(), vec![suppressed, missing]),
        ]))
        .unwrap();
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 1);
        assert_eq!(value["runs"][0]["results"][0]["level"], "warning");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]
                ["startLine"],
            1
        );
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]
                ["startColumn"],
            1
        );
    }

    #[test]
    fn diagnostics_are_sorted_independently_of_input_order() {
        let first = source("z.graphql", "query Z { hero }\n");
        let second = source("a.graphql", "query A { hero }\n");
        let first_diagnostic =
            DiagnosticBuilder::new("z-rule", first.path().to_path_buf(), Span::new(10, 4), "z")
                .finish();
        let second_diagnostic =
            DiagnosticBuilder::new("a-rule", second.path().to_path_buf(), Span::new(10, 4), "a")
                .finish();
        let value: serde_json::Value = serde_json::from_str(&render(&[
            result(vec![first], vec![first_diagnostic]),
            result(vec![second], vec![second_diagnostic]),
        ]))
        .unwrap();
        let results = value["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["ruleId"], "a-rule");
        assert_eq!(results[1]["ruleId"], "z-rule");
    }

    #[test]
    fn paths_are_normalized_and_uri_encoded() {
        assert_eq!(
            path_to_uri(Path::new("folder with space\\file#1.graphql")),
            "folder%20with%20space/file%231.graphql"
        );
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

        let error = SarifReporter::new().render(&[], &mut FailingWriter);
        assert_eq!(error.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }
}
