//! The runner: build a [`LintEngine`] + inline [`Project`] from a
//! [`FixtureCase`], lint it, and assert parity against the case's
//! `expected.json` (or zero diagnostics for `valid` cases) — spec-014.
//!
//! ## What the runner does
//!
//! 1. Builds a [`Project`] inline from the case (no filesystem beyond an
//!    optional `schema_path`): schema from `case.schema` / `case.schema_path`,
//!    the `.graphql` source as the lone operation document
//!    ([`DocKind::Operations`]) or as the schema ([`DocKind::Schema`]).
//! 2. Runs `engine.lint(&project)` to collect the engine's emitted
//!    diagnostics.
//! 3. Projects each diagnostic to the parity fields `(rule, line, column,
//!    message)` via [`project_actual`][crate::project_actual] using the
//!    project's [`SourceFile`] handles to resolve byte spans.
//! 4. Compares via [`Comparator`][crate::Comparator]: count + slot-by-slot
//!    `(rule, line, column)` and, unless `loose_message`, the verbatim message.
//!    On mismatch, returns a [`HarnessError::Parity`] carrying a
//!    `pretty_assertions`-rendered diff.
//!
//! ## The `rglint_test_suite!` macro
//!
//! [`rglint_test_suite!`] discovers every case directory under
//! `rules-fixtures/<rule-id>/{valid,invalid}/` (relative to the *consumer
//! crate's* `CARGO_MANIFEST_DIR`) and runs each through the runner with a
//! freshly-built engine enabled for `<rule-id>`. One `#[test]` is generated per
//! suite; on failure the test reports each offending case's id + diff so
//! individual case failures are visible in `cargo test` output.
//!
//! Spec-014's spec text asks for "one `#[test]` per case"; compile-time
//! directory enumeration requires a build-script (out of scope here, and
//! unneeded before spec-015's `xtask port-fixture` even produces fixtures). The
//! runtime-walking approach this macro takes keeps the interface identical from
//! a rule author's perspective (`rglint_test_suite!("no-foo")`), and produces
//! per-case failure detail — the property that matters in practice.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pretty_assertions::StrComparison;
use rglint_core::{
    DocumentLoader, DocumentSpec, LintEngine, LoadedDocuments, Project, ProjectConfig, RuleConfig,
    RulesConfig, SchemaLoader, SchemaSpec, Severity, Siblings, SourceFile,
};

use crate::expected::{find_source, project_actual, ActualError, Comparator, ParityDiff};
use crate::fixture::{load_fixture, DocKind, FixtureCase};

/// Build a [`LintEngine`] enabled for a single `rule_id` at `Severity::Error`
/// with the given `options`. The harness always lints a fixture with the rule at
/// `Error` severity — parity asserts `rule`/`message`/`location`, never
/// `severity` (`graphql-eslint` fixtures do not constrain severity).
pub fn engine_for(
    rule_id: &str,
    options: serde_json::Value,
) -> Result<LintEngine, rglint_core::LintEngineError> {
    LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: rule_id.to_owned(),
            severity: Severity::Error,
            options,
        }],
    })
}

/// Failures the runner can surface while building the inline project or linting.
#[derive(Debug, thiserror::Error)]
pub enum BuildProjectError {
    /// The case's `schema_path` pointed at a file that could not be read.
    #[error("failed to read schema_path `{path}`: {source}")]
    SchemaPathIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The schema loader rejected the case's schema (inline or path).
    #[error("schema load failed: {0}")]
    SchemaLoad(#[from] rglint_core::SchemaLoadError),
    /// The document loader rejected the case's source.
    #[error("document load failed: {0}")]
    DocumentLoad(#[from] rglint_core::DocumentLoadError),
}

/// Outcome of a successful [`run_fixture`] call: the projected actual errors,
/// for tests that want to inspect them (e.g. the snapshot helper).
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The actual diagnostics projected to parity records, in engine order.
    pub actual: Vec<ActualError>,
}

/// Errors [`run_fixture`] can surface. [`HarnessError::Parity`] carries the
/// structured [`ParityDiff`] in its message; `pretty_assertions` is used to
/// render the diff so a reader sees the familiar `--- expected / +++ actual`
/// shape.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// The inline [`Project`] could not be built (schema/document load).
    #[error(transparent)]
    Build(#[from] BuildProjectError),
    /// The engine itself errored (e.g. an unknown rule id).
    #[error("lint engine failed: {0}")]
    Engine(#[from] rglint_core::LintEngineError),
    /// Parity mismatch (count or per-slot). The rendered diff is in the
    /// message and in the carried [`ParityDiff`].
    #[error("{diff}")]
    Parity {
        /// The structured diff (los + summary). `to_string()` of the error is
        /// `diff.render()`.
        diff: ParityDiff,
    },
    /// A case directory could not be loaded by [`load_fixture`][crate::load_fixture].
    /// Used by [`run_suite`] when enumerating a suite.
    #[error("failed to load fixture `{path}`: {message}")]
    FixtureLoad { path: PathBuf, message: String },
}

/// Lint the case with `engine` and assert parity.
///
/// Builds the inline [`Project`] from `case`, runs the engine, projects the
/// emitted diagnostics, and compares them to `case.expected` (or asserts zero
/// diagnostics for `valid` cases). On mismatch returns
/// [`HarnessError::Parity`] with a `pretty_assertions`-rendered diff; on a clean
/// run returns [`RunOutcome`] with the projected actuals.
pub fn run_fixture(case: &FixtureCase, engine: &LintEngine) -> Result<RunOutcome, HarnessError> {
    let project = build_project(case)?;
    let result = engine.lint(&project)?;

    // Gather every SourceFile the project knows about (schema sources + each
    // operation document source) so any diagnostic resolves its (line, column).
    let sources = collect_sources(&project);
    let actual: Vec<ActualError> = result
        .all
        .iter()
        .map(|d| project_actual(d, find_source(&sources, &d.file)))
        .collect();

    let comparator = Comparator::new(&case.expected, case.loose_message);
    let outcome = RunOutcome {
        actual: actual.clone(),
    };
    if case.valid {
        comparator
            .compare_valid(&actual)
            .map_err(|diff| parity_err(&diff, &case.expected, &actual))?;
    } else {
        comparator
            .compare(&actual)
            .map_err(|diff| parity_err(&diff, &case.expected, &actual))?;
    }
    Ok(outcome)
}

/// Wrap a [`ParityDiff`] into [`HarnessError::Parity`], layering a
/// `pretty_assertions` `--- expected / +++ actual` body on top of the diff's
/// own lines so the failure message reads cleanly in `cargo test` output.
fn parity_err(
    diff: &ParityDiff,
    expected: &[crate::ExpectedError],
    actual: &[ActualError],
) -> HarnessError {
    // Build a compact JSON-ish projection for the StrComparison so the rendered
    // diff has the familiar +/- shape alongside the structured diff lines.
    let exp_str = format!("{expected:#?}");
    let act_str = format!("{actual:#?}");
    let comparison = StrComparison::new(&exp_str, &act_str);
    let mut msg = diff.render();
    msg.push_str("\npretty_assertions diff (expected / actual):\n");
    msg.push_str(&comparison.to_string());
    HarnessError::Parity {
        diff: ParityDiff::from_lines(msg.lines().map(|l| l.to_owned()).collect()),
    }
}

/// Build the inline [`Project`] for a case: schema from `case.schema` / path,
/// the source as the lone document (`Operations` kind) or as the schema
/// (`Schema` kind, no documents).
pub fn build_project(case: &FixtureCase) -> Result<Project, BuildProjectError> {
    let schema_loader = SchemaLoader::new();
    let doc_loader = DocumentLoader::new();
    build_project_with(&schema_loader, &doc_loader, case)
}

/// Same as [`build_project`] but lets the caller supply a (cached) loader pair.
pub fn build_project_with(
    schema_loader: &SchemaLoader,
    doc_loader: &DocumentLoader,
    case: &FixtureCase,
) -> Result<Project, BuildProjectError> {
    // base dir is irrelevant for Inline specs; only schema_path uses disk and
    // is resolved to absolute by load_fixture, so resolve it against "".
    let base = Path::new("");

    let (schema, documents) = match case.kind {
        DocKind::Operations => {
            let schema = match (&case.schema, &case.schema_path) {
                (Some(inline), _) => {
                    Some(schema_loader.load(&SchemaSpec::Inline(inline.clone()), base)?)
                }
                (None, Some(path)) => {
                    let _ = std::fs::read_to_string(path).map_err(|source| {
                        BuildProjectError::SchemaPathIo {
                            path: path.clone(),
                            source,
                        }
                    })?;
                    Some(schema_loader.load(&SchemaSpec::File(path.clone()), base)?)
                }
                (None, None) => None,
            };
            // Always load via DocumentSpec::Files so the SourceFile retains the
            // real path on disk (needed for rules like match-document-filename
            // that inspect the file name/extension). DocumentSpec::Inline
            // assigns the synthetic path <inline> which loses that information.
            let mut paths = Vec::with_capacity(1 + case.sibling_documents.len());
            paths.push(case.source_path.clone());
            paths.extend(case.sibling_documents.iter().cloned());
            let documents = doc_loader.load(
                &DocumentSpec::Files(paths),
                base,
                schema.as_deref().map(|ls| &ls.compiler),
            )?;
            (schema, documents)
        }
        DocKind::Schema => {
            // The source itself is the schema; optionally load sibling
            // operations from the inline `documents` field.
            let schema = Some(
                schema_loader.load(&SchemaSpec::Inline(case.source.clone()), base)?,
            );
            let documents = if let Some(doc_str) = &case.documents {
                // Split the documents into individual operation documents so
                // an anonymous query doesn't conflict with named operations.
                let tmp_dir = std::env::temp_dir().join("rglint-documents");
                let _ = std::fs::create_dir_all(&tmp_dir);
                let parts: Vec<&str> = doc_str
                    .split("\n\n")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut combined = empty_documents();
                for (i, part) in parts.iter().enumerate() {
                    let tmp_path = tmp_dir.join(format!("documents_{}_{}.graphql", case.id, i));
                    std::fs::write(&tmp_path, part).map_err(|source| {
                        BuildProjectError::SchemaPathIo {
                            path: tmp_path.clone(),
                            source,
                        }
                    })?;
                    let loaded = doc_loader.load(
                        &DocumentSpec::Files(vec![tmp_path]),
                        base,
                        schema.as_deref().map(|ls| &ls.compiler),
                    )?;
                    let offset = combined.docs.len();
                    combined.docs.extend(loaded.docs);
                    for (path, idx) in loaded.by_file {
                        combined.by_file.insert(path, idx + offset);
                    }
                }
                combined
            } else {
                empty_documents()
            };
            (schema, documents)
        }
    };

    let siblings = Siblings::from_documents(&documents);

    Ok(Project {
        config: ProjectConfig {
            name: format!("fixture:{}", case.id),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema,
        documents,
        siblings,
    })
}

fn empty_documents() -> LoadedDocuments {
    LoadedDocuments {
        docs: Vec::new(),
        by_file: std::collections::HashMap::new(),
    }
}

/// Collect every [`SourceFile`] the project can attribute diagnostics to, so
/// [`project_actual`] can resolve a diagnostic's span to `(line, column)`
/// regardless of which file the diagnostic points at.
fn collect_sources(project: &Project) -> Vec<Arc<SourceFile>> {
    let mut out = Vec::new();
    if let Some(schema) = project.schema.as_deref() {
        out.extend(schema.sources.iter().cloned());
    }
    for doc in &project.documents.docs {
        out.push(doc.source.clone());
    }
    out
}

/// Walk `root/valid/` and `root/invalid/`, running each case directory through
/// the runner with an engine enabled for `rule_id`. Returns a list of
/// `(case_id, HarnessError)` for every case that failed (empty on full pass).
///
/// This is the runtime helper [`rglint_test_suite!`] expands to; exported so
/// rule authors wanting custom suite setup (e.g. a shared options default) can
/// call it directly.
pub fn run_suite(rule_id: &str, root: &Path) -> Vec<(String, HarnessError)> {
    let mut failures = Vec::new();
    for sub in ["valid", "invalid"] {
        let subdir = root.join(sub);
        if !subdir.is_dir() {
            continue;
        }
        let mut cases: Vec<PathBuf> = Vec::new();
        if let Ok(rd) = fs::read_dir(&subdir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    cases.push(p);
                }
            }
        }
        cases.sort();
        for case_dir in cases {
            let case = match load_fixture(&case_dir) {
                Ok(c) => c,
                Err(e) => {
                    failures.push((
                        case_dir.display().to_string(),
                        HarnessError::FixtureLoad {
                            path: case_dir,
                            message: format!("{e}"),
                        },
                    ));
                    continue;
                }
            };
            let engine = match engine_for(rule_id, case.options.clone()) {
                Ok(e) => e,
                Err(e) => {
                    failures.push((case.id.clone(), HarnessError::Engine(e)));
                    continue;
                }
            };
            if let Err(e) = run_fixture(&case, &engine) {
                failures.push((case.id, e));
            }
        }
    }
    failures
}

/// The test-suite macro. Invoke as `rglint_test_suite!("no-anonymous-operations")`
/// in a rule crate's test module; it expands to a single `#[test]` that walks
/// `rules-fixtures/<rule-id>/{valid,invalid}/` (relative to the crate's
/// `CARGO_MANIFEST_DIR`) and asserts every case passes parity. On failure the
/// test panics with a combined per-case report (case id + diff) so individual
/// case failures are visible.
///
/// A second, optional argument overrides the fixture root (useful when the
/// suite lives somewhere other than `<manifest>/rules-fixtures`):
///
/// ```ignore
/// rglint_test_suite!("my-rule", root = "tests/my-rule-fixtures");
/// ```
#[macro_export]
macro_rules! rglint_test_suite {
    ($rule_id:literal) => {
        $crate::rglint_test_suite!($rule_id, root = "rules-fixtures");
    };
    ($rule_id:literal, root = $root:literal) => {
        #[test]
        fn rglint_test_suite() {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join($root)
                .join(std::path::Path::new($rule_id));
            let failures = rglint_test_harness::run_suite($rule_id, &root);
            if !failures.is_empty() {
                let mut msg = format!(
                    "rglint_test_suite!({}) had {} failing case(s):\n",
                    $rule_id,
                    failures.len()
                );
                for (id, err) in &failures {
                    msg.push_str(&format!("  - {id}: {err}\n"));
                }
                panic!("{msg}");
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Handler, RuleContext};
    use rglint_derive::Rule as DeriveRule;
    use std::fs;
    use std::path::PathBuf;

    /// A stand-in no-op rule registered into the test binary's `ALL_RULES`
    /// via `#[derive(Rule)]`. Its handler emits nothing, so:
    ///
    /// - valid cases (no expected) compare 0 == 0 → pass,
    /// - broken sources still emit `parse-error` diagnostics from the
    ///   document loader, which the harness surfaces as a parity mismatch.
    ///
    /// This is exactly the substrate the harness needs to test its plumbing
    /// without depending on a real rule (none exist until spec-016).
    #[derive(DeriveRule)]
    #[rule(id = "__rg_harness_test_rule", category = "operations")]
    struct HarnessTestRule;

    impl HarnessTestRule {
        fn handler(&self, _ctx: &mut RuleContext) -> std::boxed::Box<dyn Handler> {
            std::boxed::Box::new(NoopHandler)
        }
    }

    struct NoopHandler;
    impl Handler for NoopHandler {}

    fn write_case(root: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = root.join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (fname, contents) in files {
            fs::write(dir.join(fname), contents).unwrap();
        }
        dir
    }

    fn tmp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("rglint-harness-runner-tests")
            .join(label);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn run_fixture_reports_parse_error_on_broken_source() {
        let root = tmp_root("parse_error");
        let dir = write_case(
            &root,
            "broken",
            &[
                ("01.graphql", "query { x "), // malformed
                ("01.expected.json", r#"{"errors":[]}"#),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        // expected=[] but the engine will emit parse-error(s) → parity failure
        // (count mismatch). We assert the failure is informative. Use the
        // registered no-op rule so the engine builds (parse-errors still fire
        // from the document loader).
        let engine = engine_for("__rg_harness_test_rule", serde_json::Value::Null).unwrap();
        let err = run_fixture(&case, &engine).expect_err("broken source -> parity err");
        assert!(matches!(err, HarnessError::Parity { .. }), "got {err:?}");
        let msg = format!("{err}");
        assert!(
            msg.contains("count") || msg.contains("unexpected"),
            "msg: {msg}"
        );
    }

    #[test]
    fn valid_clean_case_passes_with_empty_engine() {
        let root = tmp_root("valid_clean");
        let dir = write_case(&root, "clean", &[("01.graphql", "query { x }")]);
        let case = load_fixture(&dir).unwrap();
        let engine = engine_for("__rg_harness_test_rule", serde_json::Value::Null).unwrap();
        let outcome = run_fixture(&case, &engine).expect("clean valid -> ok");
        assert!(outcome.actual.is_empty());
    }

    #[test]
    fn run_suite_reports_no_failures_for_clean_dir() {
        let root = tmp_root("run_suite_clean");
        let inv = root.join("invalid");
        fs::create_dir_all(&inv).unwrap();
        let dir = write_case(&inv, "case", &[("01.graphql", "query { x }")]);
        let _ = dir;
        let failures = run_suite("__rg_harness_test_rule", &root);
        assert!(
            failures.is_empty(),
            "valid case + no-op rule -> no failures"
        );
    }

    #[test]
    fn run_suite_reports_failure_for_broken_case() {
        let root = tmp_root("run_suite_broken");
        let inv = root.join("invalid");
        fs::create_dir_all(&inv).unwrap();
        write_case(
            &inv,
            "broken",
            &[
                ("01.graphql", "query { x "),
                ("01.expected.json", r#"{"errors":[]}"#),
            ],
        );
        let failures = run_suite("__rg_harness_test_rule", &root);
        assert_eq!(failures.len(), 1, "one broken case -> one failure");
        assert!(failures[0].0.contains("broken"));
        assert!(format!("{}", failures[0].1).contains("count"));
    }

    #[test]
    fn build_project_schema_kind_loads_source_as_schema() {
        let root = tmp_root("schema_kind");
        let dir = write_case(
            &root,
            "schema_case",
            &[
                ("01.graphql", "type Query { x: Int }"),
                ("01.config.toml", "kind = \"schema\"\n"),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        let project = build_project(&case).expect("schema kind project builds");
        assert!(project.schema.is_some(), "source loaded as schema");
        assert!(project.documents.docs.is_empty(), "no operation documents");
    }

    #[test]
    fn build_project_operations_kind_with_inline_schema() {
        let root = tmp_root("ops_inline_schema");
        let dir = write_case(
            &root,
            "ops",
            &[
                ("01.graphql", "query { x }"),
                ("01.config.toml", "schema = \"type Query { x: Int }\"\n"),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        let project = build_project(&case).expect("ops kind with inline schema builds");
        assert!(project.schema.is_some());
        assert_eq!(project.documents.docs.len(), 1);
    }
}
