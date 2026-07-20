//! Parse a fixture case directory into an in-memory [`FixtureCase`].
//!
//! ## On-disk layout
//!
//! The spec sketches fixture triplets as
//! `rules-fixtures/<rule-id>/{valid,invalid}/NN.{graphql,config.toml,expected.json}`.
//! We model each **case** as its own directory (so `NN` is the dir name), which
//! lets a case carry arbitrarily-named extras (a separate `schema.graphqls`,
//! a `.opts.json` snapshot, …) without colliding with the triplet convention.
//! Concretely a case directory contains:
//!
//! ```text
//! rules-fixtures/<rule-id>/invalid/01/
//!   *.graphql         # required: the source under lint
//!   *.config.toml     # optional: schema + options + loose_message + kind
//!   *.expected.json   # optional: PLAN §6.1 `{ "errors": [...] }`
//! ```
//!
//! The harness finds each part by suffix (`*.graphql`, `*.config.toml`,
//! `*.expected.json`), so the conventional `01.graphql` / `01.config.toml` /
//! `01.expected.json` names work, as do plain `graphql` / `config.toml` /
//! `expected.json`. A directory with `*.expected.json` (or any expected errors
//! inline) is an **invalid** case; a directory without one is a **valid** case
//! (the runner asserts zero diagnostics).
//!
//! ## `config.toml` shape
//!
//! ```toml
//! # Inline SDL schema (optional). Omit schema entirely for schema-less cases.
//! schema = "type Query { x: Int }"
//! # OR a path relative to the case dir (optional):
//! # schema_path = "schema.graphqls"
//!
//! # What kind of source the `.graphql` file is. Default "operations".
//! # Set "schema" for schema-rule fixtures (the source is loaded as the schema;
//! # no operation documents are loaded).
//! kind = "operations"
//!
//! # Compare rule + location only, never the message (spec-053). Default false.
//! loose_message = true
//!
//! [options]            # optional: rule options as a TOML table → JSON
//! maxDepth = 5
//! ```
//!
//! The rule **id** is *not* in `config.toml` — the [`rglint_test_suite!`][crate::rglint_test_suite]
//! macro supplies it (one suite per rule), so all cases in a suite enable the
//! same rule.
//!
//! ## `expected.json` shape
//!
//! ```json
//! { "errors": [
//!   { "rule": "no-anonymous-operations",
//!     "message": "Anonymous operation",
//!     "line": 1,
//!     "column": 0 }
//! ] }
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use crate::expected::ExpectedError;

/// Which kind of GraphQL source the fixture's `.graphql` file holds.
///
/// Operation rules lint a document; schema rules lint SDL. The runner uses this
/// to decide whether the source becomes the project's lone operation document
/// ([`DocKind::Operations`]) or its inline schema ([`DocKind::Schema`], with no
/// operation documents loaded).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DocKind {
    /// The `.graphql` source is an operation document (queries / mutations /
    /// fragments). This is the default.
    #[default]
    Operations,
    /// The `.graphql` source is SDL and should be linted as the project's
    /// schema. No operation documents are loaded; schema rules fire on it.
    Schema,
}

impl DocKind {
    /// Parse from the `kind = "..."` TOML string. Unknown values fall back to
    /// [`DocKind::Operations`] (the default) rather than erroring, so a typo in
    /// a fixture's `kind` degrades to "the common case" instead of failing the
    /// whole suite.
    pub fn from_kebab(s: &str) -> Self {
        match s {
            "schema" => DocKind::Schema,
            _ => DocKind::Operations,
        }
    }
}

/// The optional per-case configuration, parsed from `<case>.config.toml`.
///
/// All fields have defaults that make a minimum-viable `config.toml` empty
/// (or the file absent entirely): no schema, operations-kind, strict message
/// parity, no rule options.
#[derive(Clone, Debug, Default)]
pub struct FixtureConfig {
    /// Inline SDL, when `schema = "..."` was set.
    pub schema: Option<String>,
    /// A path to SDL relative to the case dir, when `schema_path = "..."` was
    /// set. Resolved to an absolute path by [`load_fixture`] so the runner can
    /// hand it straight to the schema loader.
    pub schema_path: Option<PathBuf>,
    /// What kind the `.graphql` source is.
    pub kind: DocKind,
    /// Skip the message comparison (spec-053 `loose_message`).
    pub loose_message: bool,
    /// Rule options, converted from the TOML `[options]` table to a JSON value.
    /// Defaults to `serde_json::Value::Null` when no `[options]` is present.
    pub options: serde_json::Value,
    /// Additional sibling operation documents (relative paths in the case dir,
    /// e.g. `["01.sibling.graphql"]`) loaded alongside the main `.graphql`
    /// source so `requires_siblings` rules (spec-017 onwards) can be exercised
    /// with multiple documents in one fixture case. Defaults to empty.
    pub sibling_documents: Vec<String>,
    /// Inline operation document(s) as a single string, from `documents = "..."`.
    /// When set (and `kind = "schema"`), the main `.graphql` source is loaded as
    /// the schema and these inline documents become the sibling operations.
    /// Defaults to `None`.
    pub documents: Option<String>,
}

/// One in-memory fixture case, ready to be passed to
/// [`run_fixture`][crate::run_fixture].
///
/// `id` is the case directory's file name (e.g. `"01"`), so test failures and
/// macro-generated case names carry a stable identifier. `valid` is `true` when
/// no `expected.json` was present (the runner asserts zero diagnostics).
#[derive(Clone, Debug)]
pub struct FixtureCase {
    /// The case directory's file name (e.g. `"01"`).
    pub id: String,
    /// The absolute path to the case directory.
    pub dir: PathBuf,
    /// The on-disk path of the main `.graphql` source under lint. Equivalent
    /// to [`Self::source`] when no sibling documents are configured; always
    /// present so the runner can hand it to [`DocumentSpec::Files`] when
    /// `sibling_documents` is nonempty (see [`Self::sibling_documents`]).
    pub source_path: PathBuf,
    /// The source under lint (the `.graphql` file's contents).
    pub source: String,
    /// Additional sibling operation documents, as **absolute** paths
    /// (resolved from [`FixtureConfig::sibling_documents`]). Empty unless
    /// the case's `config.toml` declared `sibling_documents = [...]`. When
    /// nonempty, the runner builds the project from these paths **plus**
    /// [`Self::source_path`] via [`DocumentSpec::Files`] so a
    /// `requires_siblings` rule sees a multi-document bundle.
    pub sibling_documents: Vec<PathBuf>,
    /// Inline SDL schema, if `schema = "..."` was set in `config.toml`.
    pub schema: Option<String>,
    /// A path to a schema file relative to the case dir, if `schema_path` was
    /// set. Mutually exclusive with [`Self::schema`].
    pub schema_path: Option<PathBuf>,
    /// What kind the `.graphql` source [`Self::source`] is.
    pub kind: DocKind,
    /// Rule options (JSON, from the `[options]` TOML table).
    pub options: serde_json::Value,
    /// Inline operation document(s) as a single string, from `documents = "..."`.
    /// When set (and `kind = "schema"`), the main `.graphql` source is loaded as
    /// the schema and these inline documents become the sibling operations.
    pub documents: Option<String>,
    /// Compare rule + location only (spec-053 `loose_message`).
    pub loose_message: bool,
    /// The expected parity errors. Empty for `valid` cases.
    pub expected: Vec<ExpectedError>,
    /// `true` when no `expected.json` was present — the runner asserts the
    /// engine emits zero diagnostics.
    pub valid: bool,
}

/// Errors [`load_fixture`] can surface while reading a case directory.
#[derive(Debug, thiserror::Error)]
pub enum FixtureLoadError {
    /// The case directory does not exist or is not a directory.
    #[error("fixture directory `{dir}` does not exist or is not a directory")]
    MissingDir { dir: PathBuf },
    /// No `.graphql` or `.gql` source file was found in the case directory.
    #[error("no `*.graphql` or `*.gql` source file found in fixture directory `{dir}`")]
    NoSource { dir: PathBuf },
    /// Multiple `.graphql` source files were found; the harness expects one.
    #[error("multiple `*.graphql` files in fixture directory `{dir}`: {files:?}")]
    ManySources { dir: PathBuf, files: Vec<PathBuf> },
    /// Reading a file from disk failed.
    #[error("failed to read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// `config.toml` failed to parse.
    #[error("failed to parse config `{path}`: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    /// `expected.json` failed to parse.
    #[error("failed to parse expected `{path}`: {source}")]
    ExpectedParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// `expected.json`'s top-level shape wasn't `{ "errors": [...] }`.
    #[error("expected `{path}`: top-level object missing an `errors` array")]
    ExpectedShape { path: PathBuf },
    /// A sibling document declared in `sibling_documents = [...]` doesn't
    /// exist on disk. Surfaces before source discovery so a typo in the
    /// path is reported with the missing path rather than confused for the
    /// "this is actually the main source" case.
    #[error("sibling document `{path}` declared in config.toml does not exist")]
    SiblingMissing { path: PathBuf },
    /// Multiple `*.graphql` files remain after excluding the declared
    /// sibling documents — the harness needs exactly one main source.
    #[error(
        "multiple main `*.graphql` files in fixture directory `{dir}` after excluding siblings: {files:?}"
    )]
    ManySourcesExcluding { dir: PathBuf, files: Vec<PathBuf> },
}

/// Parse the case directory at `dir` into a [`FixtureCase`].
///
/// See the [module docs][self] for the on-disk layout, the `config.toml` shape,
/// and the `expected.json` shape.
pub fn load_fixture(dir: &Path) -> Result<FixtureCase, FixtureLoadError> {
    if !dir.is_dir() {
        return Err(FixtureLoadError::MissingDir {
            dir: dir.to_path_buf(),
        });
    }

    let id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| dir.display().to_string());

    // Parse config first so we can resolve `sibling_documents` to absolute
    // paths *before* discovering the main source — sibling files (e.g.
    // `01.sibling.graphql`) live in the same case dir and would otherwise
    // trip the "exactly one `*.graphql`" rule in `find_one_suffix`. The
    // `.config.toml` / `.expected.json` suffixes don't collide with `.graphql`
    // so this reorder is safe.
    let config_path = find_opt_suffix(dir, ".config.toml");
    let config = match config_path {
        Some(path) => {
            let text = fs::read_to_string(&path).map_err(|source| FixtureLoadError::Io {
                path: path.clone(),
                source,
            })?;
            parse_config(&text, path, dir)?
        }
        None => FixtureConfig::default(),
    };
    let sibling_documents: Vec<PathBuf> = config
        .sibling_documents
        .iter()
        .map(|s| dir.join(s))
        .collect();
    for p in &sibling_documents {
        if !p.is_file() {
            return Err(FixtureLoadError::SiblingMissing { path: p.clone() });
        }
    }

    // Find the main source: exactly one `*.graphql` or `*.gql` *excluding*
    // sibling files. Try `.graphql` first (the conventional extension), then
    // `.gql` (used by `match-document-filename` fixtures and the original
    // graphql-eslint tests).
    let source_path = find_one_suffix_excluding(dir, ".graphql", &sibling_documents)
        .or_else(|_| find_one_suffix_excluding(dir, ".gql", &sibling_documents))?;
    let source = fs::read_to_string(&source_path).map_err(|source| FixtureLoadError::Io {
        path: source_path.clone(),
        source,
    })?;

    // Optional expected.json. Absent ⇒ valid case.
    let expected_path = find_opt_suffix(dir, ".expected.json");
    let (expected, valid) = match expected_path {
        Some(path) => {
            let text = fs::read_to_string(&path).map_err(|source| FixtureLoadError::Io {
                path: path.clone(),
                source,
            })?;
            // Parse to a Value first so we can enforce the `{ "errors": [...] }`
            // shape *before* field-deserializing each error: this distinguishes
            // a missing/mistyped `errors` key (ExpectedShape) from a malformed
            // entry (ExpectedParse), which the slot error messages care about.
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|source| FixtureLoadError::ExpectedParse {
                    path: path.clone(),
                    source,
                })?;
            let errors_val = v
                .get("errors")
                .filter(|e| e.is_array())
                .ok_or_else(|| FixtureLoadError::ExpectedShape { path: path.clone() })?;
            let errors_arr = errors_val.as_array().expect("checked above");
            let mut errors = Vec::with_capacity(errors_arr.len());
            for (i, entry) in errors_arr.iter().enumerate() {
                match serde_json::from_value::<ExpectedError>(entry.clone()) {
                    Ok(e) => errors.push(e),
                    Err(source) => {
                        // Annotate the entry index so a malformed entry is easy
                        // to locate in the fixture; reuse serde's custom-error.
                        use serde::de::Error as _;
                        return Err(FixtureLoadError::ExpectedParse {
                            path: path.clone(),
                            source: serde_json::Error::custom(format!("errors[{i}]: {source}")),
                        });
                    }
                }
            }
            (errors, false)
        }
        None => (Vec::new(), true),
    };

    Ok(FixtureCase {
        id,
        dir: dir.to_path_buf(),
        source_path,
        source,
        sibling_documents,
        schema: config.schema,
        schema_path: config.schema_path,
        kind: config.kind,
        options: config.options,
        documents: config.documents,
        loose_message: config.loose_message,
        expected,
        valid,
    })
}

/// Find exactly one file in `dir` whose name ends with `suffix`. Errors on
/// missing/multiple.
fn find_one_suffix(dir: &Path, suffix: &str) -> Result<PathBuf, FixtureLoadError> {
    let hits = list_suffix(dir, suffix);
    match hits.len() {
        0 => Err(FixtureLoadError::NoSource {
            dir: dir.to_path_buf(),
        }),
        1 => Ok(hits[0].clone()),
        _ => Err(FixtureLoadError::ManySources {
            dir: dir.to_path_buf(),
            files: hits,
        }),
    }
}

/// Like [`find_one_suffix`] but excludes any path listed in `exclude` (used to
/// keep sibling documents out of the "exactly one main source" check). Errors
/// return a distinct [`FixtureLoadError::ManySourcesExcluding`] when more than
/// one candidate remains after exclusion, so a fixture author can tell from
/// the message that sibling exclusion was applied.
fn find_one_suffix_excluding(
    dir: &Path,
    suffix: &str,
    exclude: &[PathBuf],
) -> Result<PathBuf, FixtureLoadError> {
    let mut hits = list_suffix(dir, suffix);
    hits.retain(|p| !exclude.contains(p));
    match hits.len() {
        0 => Err(FixtureLoadError::NoSource {
            dir: dir.to_path_buf(),
        }),
        1 => Ok(hits[0].clone()),
        _ => Err(FixtureLoadError::ManySourcesExcluding {
            dir: dir.to_path_buf(),
            files: hits,
        }),
    }
}

/// Find an optional file in `dir` whose name ends with `suffix`. `None` if no
/// match. Errors if multiple match — a case dir should have at most one of each
/// part; treat duplicates as a config mistake rather than picking arbitrarily.
fn find_opt_suffix(dir: &Path, suffix: &str) -> Option<PathBuf> {
    let hits = list_suffix(dir, suffix);
    if hits.len() == 1 {
        Some(hits[0].clone())
    } else {
        None
    }
}

fn list_suffix(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    // Use the full filename's end, not the dot-extension, so
                    // `.config.toml` and `.expected.json` (compound suffixes)
                    // match unambiguously.
                    if name.ends_with(suffix) {
                        hits.push(p);
                    }
                }
            }
        }
    }
    hits.sort();
    hits
}

/// Parse `config.toml` text into a [`FixtureConfig`]. `case_dir` is used to
/// resolve `schema_path` to an absolute path.
fn parse_config(
    text: &str,
    path: PathBuf,
    case_dir: &Path,
) -> Result<FixtureConfig, FixtureLoadError> {
    // `toml::de::Error`'s `custom` constructor comes from `serde::de::Error`;
    // bring it into scope locally so we can synthesize a "root must be a
    // table" message without a dedicated error variant.
    use serde::de::Error as _;

    let raw: toml::Value =
        toml::from_str(text).map_err(|source| FixtureLoadError::ConfigParse {
            path: path.clone(),
            source,
        })?;
    let table = raw
        .as_table()
        .ok_or_else(|| FixtureLoadError::ConfigParse {
            path: path.clone(),
            source: toml::de::Error::custom("config root must be a table"),
        })?;

    let schema = table
        .get("schema")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let schema_path = table
        .get("schema_path")
        .and_then(|v| v.as_str())
        .map(|s| case_dir.join(s));
    let kind = table
        .get("kind")
        .and_then(|v| v.as_str())
        .map(DocKind::from_kebab)
        .unwrap_or_default();
    let loose_message = table
        .get("loose_message")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let options = table
        .get("options")
        .map(toml_to_json)
        .unwrap_or(serde_json::Value::Null);

    let sibling_documents = table
        .get("sibling_documents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str().map(|s| s.to_owned()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let documents = table
        .get("documents")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    Ok(FixtureConfig {
        schema,
        schema_path,
        kind,
        loose_message,
        options,
        sibling_documents,
        documents,
    })
}

/// Convert a `toml::Value` (typically the `[options]` table) into a
/// `serde_json::Value`, preserving strings/numbers/bools/arrays/tables. Done by
/// round-tripping through JSON text so we don't depend on a toml↔json bridge
/// crate. Tables become JSON objects; arrays stay arrays.
fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(i),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in t {
                obj.insert(k.clone(), toml_to_json(val));
            }
            serde_json::Value::Object(obj)
        }
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn write_case(name: &str, files: &[(&str, &str)]) -> PathBuf {
        // Write under the OS tmp dir (not the crate dir) so the test artifacts
        // never leak into the workspace tree (which is not gitignored under
        // `crates/*/target/`).
        let root = std::env::temp_dir()
            .join("rglint-harness-fixture-tests")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (fname, contents) in files {
            fs::write(root.join(fname), contents).unwrap();
        }
        root
    }

    #[test]
    fn loads_minimal_valid_case() {
        let dir = write_case("minimal_valid", &[("01.graphql", "query { x }")]);
        let case = load_fixture(&dir).expect("load minimal valid");
        assert_eq!(case.id, "minimal_valid");
        assert!(case.valid, "no expected.json => valid case");
        assert!(case.expected.is_empty());
        assert_eq!(case.source, "query { x }");
        assert!(case.schema.is_none());
        assert_eq!(case.kind, DocKind::Operations);
        assert!(case.options.is_null());
        assert!(!case.loose_message);
    }

    #[test]
    fn loads_invalid_case_with_expected_json() {
        let dir = write_case(
            "invalid_with_expected",
            &[
                ("01.graphql", "query { x }"),
                (
                    "01.expected.json",
                    r#"{"errors":[{"rule":"no-anonymous-operations","message":"Anonymous operation","line":1,"column":0}]}"#,
                ),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        assert!(!case.valid);
        assert_eq!(case.expected.len(), 1);
        let e = &case.expected[0];
        assert_eq!(e.rule, "no-anonymous-operations");
        assert_eq!(e.line, 1);
        assert_eq!(e.column, 0);
    }

    #[test]
    fn parses_config_toml_schema_options_kind() {
        let dir = write_case(
            "with_config",
            &[
                ("01.graphql", "type Query { x: Int }"),
                (
                    "01.config.toml",
                    "kind = \"schema\"\nloose_message = true\nschema = \"type Query { x: Int }\"\n[options]\nmaxDepth = 5\n",
                ),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        assert_eq!(case.kind, DocKind::Schema);
        assert!(case.loose_message);
        assert_eq!(case.schema.as_deref(), Some("type Query { x: Int }"));
        assert_eq!(case.options["maxDepth"], serde_json::json!(5));
    }

    #[test]
    fn schema_path_resolves_relative_to_case_dir() {
        let dir = write_case(
            "with_schema_path",
            &[
                ("01.graphql", "query { x }"),
                ("01.config.toml", "schema_path = \"schema.graphqls\"\n"),
                ("schema.graphqls", "type Query { x: Int }"),
            ],
        );
        let case = load_fixture(&dir).unwrap();
        let sp = case.schema_path.expect("schema_path set");
        assert!(sp.ends_with("schema.graphqls"));
        assert!(sp.is_absolute() || sp.starts_with(&dir));
    }

    #[test]
    fn missing_dir_is_an_error() {
        let err = load_fixture(Path::new("/nonexistent/rglint-fixture-missing"))
            .expect_err("missing dir");
        assert!(format!("{err}").contains("does not exist"));
    }

    #[test]
    fn no_source_is_an_error() {
        let dir = write_case("no_source", &[("anything.txt", "hello")]);
        let err = load_fixture(&dir).expect_err("no source");
        assert!(format!("{err}").contains("no `*.graphql`"));
    }

    #[test]
    fn malformed_expected_json_is_an_error() {
        let dir = write_case(
            "bad_expected",
            &[
                ("01.graphql", "query { x }"),
                ("01.expected.json", "{not json"),
            ],
        );
        let err = load_fixture(&dir).expect_err("bad expected");
        assert!(format!("{err}").contains("parse expected"));
    }

    #[test]
    fn expected_without_errors_array_is_shape_error() {
        let dir = write_case(
            "bad_shape",
            &[
                ("01.graphql", "query { x }"),
                ("01.expected.json", r#"{"foo": 1}"#),
            ],
        );
        let err = load_fixture(&dir).expect_err("bad shape");
        assert!(format!("{err}").contains("missing an `errors`"));
    }
}
