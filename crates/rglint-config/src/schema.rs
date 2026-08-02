//! Serde schema, discovery, and normalization for `.rglintrc`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use ahash::AHashMap;
use rglint_core::{DocumentSpec, ProjectConfig, RulesConfig, SchemaSpec, Severity};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const CONFIG_NAMES: [&str; 4] = [
    ".rglintrc",
    ".rglintrc.json",
    ".rglintrc.toml",
    "rglint.config.json",
];

/// The output format selected by the configuration file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    /// Human-readable diagnostics.
    #[default]
    Pretty,
    /// JSON diagnostics.
    Json,
    /// SARIF diagnostics.
    Sarif,
    /// GitHub Actions annotations.
    Github,
}

impl Serialize for Format {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Format {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            "sarif" => Ok(Self::Sarif),
            "github" => Ok(Self::Github),
            _ => Err(serde::de::Error::custom(format!(
                "unknown format `{value}`; expected pretty, json, sarif, or github"
            ))),
        }
    }
}

impl Format {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
            Self::Sarif => "sarif",
            Self::Github => "github",
        }
    }
}

/// A schema path/glob as written in `.rglintrc`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaSpecRaw {
    /// One path or glob.
    Single(String),
    /// Several schema files.
    Multiple(Vec<String>),
}

/// An operation-document path/glob as written in `.rglintrc`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocumentSpecRaw {
    /// One path or glob.
    Single(String),
    /// Several paths or globs.
    Multiple(Vec<String>),
}

/// One normalized project from the `projects` map, or the synthesized default
/// project. `name` is the map key and is retained for the core resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConfigRaw {
    /// Project name.
    pub name: String,
    /// Schema path/glob, if configured.
    pub schema: Option<SchemaSpecRaw>,
    /// Operation-document path/glob, if configured.
    pub documents: Option<DocumentSpecRaw>,
    /// Project-specific ignore globs, after top-level ignores are prepended.
    pub ignore: Vec<String>,
}

impl ProjectConfigRaw {
    /// Convert this normalized project into the core resolver's description.
    pub fn to_core(&self) -> ProjectConfig {
        ProjectConfig {
            name: self.name.clone(),
            schema: self.schema.as_ref().map(SchemaSpecRaw::to_core),
            documents: self.documents.as_ref().map(DocumentSpecRaw::to_core),
            ignore: self.ignore.clone(),
        }
    }
}

impl SchemaSpecRaw {
    fn to_core(&self) -> SchemaSpec {
        match self {
            Self::Single(value) if has_glob_meta(value) => SchemaSpec::Glob(value.clone()),
            Self::Single(value) => SchemaSpec::File(PathBuf::from(value)),
            Self::Multiple(values) => SchemaSpec::Files(values.iter().map(PathBuf::from).collect()),
        }
    }
}

impl DocumentSpecRaw {
    fn to_core(&self) -> DocumentSpec {
        match self {
            Self::Single(value) if has_glob_meta(value) => DocumentSpec::Glob(value.clone()),
            Self::Single(value) => DocumentSpec::Files(vec![PathBuf::from(value)]),
            Self::Multiple(values) => DocumentSpec::Globs(values.clone()),
        }
    }
}

/// A fully normalized `.rglintrc` configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Projects in deterministic map-key order.
    pub projects: Vec<ProjectConfigRaw>,
    /// Rule id to resolved `(severity, options)`.
    pub rules: AHashMap<String, (Severity, serde_json::Value)>,
    /// Top-level ignores, retained separately for callers that need them.
    pub ignore: Vec<String>,
    /// Selected reporter format.
    pub format: Format,
}

impl Config {
    /// Convert the normalized rule map into the engine's configuration.
    pub fn rules_config(&self) -> RulesConfig {
        let mut ids: Vec<_> = self.rules.keys().collect();
        ids.sort_unstable();
        RulesConfig {
            rules: ids
                .into_iter()
                .map(|id| {
                    let (severity, options) = &self.rules[id];
                    rglint_core::RuleConfig {
                        id: id.clone(),
                        severity: *severity,
                        options: options.clone(),
                    }
                })
                .collect(),
        }
    }

    /// Convert normalized projects for [`rglint_core::ProjectResolver`].
    pub fn project_configs(&self) -> Vec<ProjectConfig> {
        self.projects
            .iter()
            .map(ProjectConfigRaw::to_core)
            .collect()
    }
}

impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let projects = self
            .projects
            .iter()
            .map(|project| {
                (
                    project.name.clone(),
                    RawProjectConfig {
                        schema: project.schema.clone(),
                        documents: project.documents.clone(),
                        ignore: local_ignore(&self.ignore, &project.ignore),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let rules = self
            .rules
            .iter()
            .map(|(id, (severity, options))| {
                (
                    id.clone(),
                    RuleConfig::Tuple(vec![
                        serde_json::Value::String(severity_to_str(*severity).to_owned()),
                        options.clone(),
                    ]),
                )
            })
            .collect::<BTreeMap<_, _>>();
        RawConfig {
            projects: Some(projects),
            schema: None,
            documents: None,
            rules,
            ignore: self.ignore.clone(),
            format: Some(self.format),
        }
        .serialize(serializer)
    }
}

/// Errors returned while discovering or parsing a configuration file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The requested config path does not exist or cannot be read.
    #[error("failed to read config `{path}`: {source}")]
    Io {
        /// Config path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file was syntactically invalid. Lines and columns are one-based.
    #[error("failed to parse config `{path}` at {line}:{column}: {message}")]
    Parse {
        /// Config path.
        path: PathBuf,
        /// One-based line.
        line: usize,
        /// One-based column.
        column: usize,
        /// Parser detail.
        message: String,
    },
    /// A rule setting used an unsupported severity or tuple shape.
    #[error("invalid configuration for rule `{rule}`: {message}")]
    InvalidRule {
        /// Rule id.
        rule: String,
        /// Validation detail.
        message: String,
    },
}

/// Search upward from `start` and return the closest config, checking names in
/// the documented precedence order at each directory.
pub fn discover(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        for name in CONFIG_NAMES {
            let candidate = current.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load and normalize one JSON or TOML config file.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw = parse_raw(path, &source)?;
    normalize(raw)
}

fn parse_raw(path: &Path, source: &str) -> Result<RawConfig, ConfigError> {
    let is_json = path.extension().and_then(|ext| ext.to_str()) == Some("json")
        || (path.extension().is_none() && source.trim_start().starts_with(['{', '[']));
    if is_json {
        warn_unknown_json_keys(path, source);
        serde_json::from_str(source).map_err(|error| ConfigError::Parse {
            path: path.to_path_buf(),
            line: error.line(),
            column: error.column(),
            message: error.to_string(),
        })
    } else {
        warn_unknown_toml_keys(path, source);
        toml::from_str(source).map_err(|error| {
            let offset = error.span().map(|span| span.start).unwrap_or(0);
            let (line, column) = line_column(source, offset);
            ConfigError::Parse {
                path: path.to_path_buf(),
                line,
                column,
                message: error.to_string(),
            }
        })
    }
}

fn normalize(raw: RawConfig) -> Result<Config, ConfigError> {
    let top_ignore = raw.ignore;
    let projects = match raw.projects {
        Some(projects) => projects
            .into_iter()
            .map(|(name, project)| ProjectConfigRaw {
                name,
                schema: project.schema,
                documents: project.documents,
                ignore: prepend_ignore(&top_ignore, project.ignore),
            })
            .collect(),
        None => vec![ProjectConfigRaw {
            name: "default".to_owned(),
            schema: raw.schema,
            documents: raw.documents,
            ignore: top_ignore.clone(),
        }],
    };
    let rules = raw
        .rules
        .into_iter()
        .map(|(id, setting)| parse_rule(&id, setting).map(|resolved| (id, resolved)))
        .collect::<Result<AHashMap<_, _>, _>>()?;
    Ok(Config {
        projects,
        rules,
        ignore: top_ignore,
        format: raw.format.unwrap_or_default(),
    })
}

fn parse_rule(id: &str, setting: RuleConfig) -> Result<(Severity, serde_json::Value), ConfigError> {
    let (severity, options) = match setting {
        RuleConfig::Severity(value) => (parse_severity(id, &value)?, empty_options()),
        RuleConfig::Tuple(values) => {
            if values.len() != 2 {
                return Err(ConfigError::InvalidRule {
                    rule: id.to_owned(),
                    message: "tuple form must contain severity and options".to_owned(),
                });
            }
            let severity = values[0].as_str().ok_or_else(|| ConfigError::InvalidRule {
                rule: id.to_owned(),
                message: "severity must be off, warn, or error".to_owned(),
            })?;
            if !values[1].is_object() {
                return Err(ConfigError::InvalidRule {
                    rule: id.to_owned(),
                    message: "tuple options must be an object".to_owned(),
                });
            }
            (parse_severity(id, severity)?, values[1].clone())
        }
    };
    Ok((severity, options))
}

fn parse_severity(id: &str, value: &str) -> Result<Severity, ConfigError> {
    match value {
        "off" => Ok(Severity::Off),
        "warn" => Ok(Severity::Warn),
        "error" => Ok(Severity::Error),
        _ => Err(ConfigError::InvalidRule {
            rule: id.to_owned(),
            message: format!("unknown severity `{value}`; expected off, warn, or error"),
        }),
    }
}

fn empty_options() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn severity_to_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Off => "off",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn prepend_ignore(top: &[String], local: Vec<String>) -> Vec<String> {
    top.iter().cloned().chain(local).collect()
}

fn local_ignore(top: &[String], combined: &[String]) -> Vec<String> {
    combined.strip_prefix(top).unwrap_or(combined).to_vec()
}

fn has_glob_meta(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '*' | '?' | '[' | '{'))
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .count()
        + 1;
    (line, column)
}

fn warn_unknown_json_keys(path: &Path, source: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    warn_unknown_keys(path, object.keys().map(String::as_str));
}

fn warn_unknown_toml_keys(path: &Path, source: &str) {
    let Ok(value) = source.parse::<toml::Value>() else {
        return;
    };
    let Some(table) = value.as_table() else {
        return;
    };
    warn_unknown_keys(path, table.keys().map(String::as_str));
}

fn warn_unknown_keys<'a>(path: &Path, keys: impl Iterator<Item = &'a str>) {
    const KNOWN: [&str; 6] = [
        "projects",
        "schema",
        "documents",
        "rules",
        "ignore",
        "format",
    ];
    for key in keys {
        if !KNOWN.contains(&key) {
            tracing::warn!(path = %path.display(), key, "unknown top-level config key ignored");
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct RawConfig {
    projects: Option<BTreeMap<String, RawProjectConfig>>,
    schema: Option<SchemaSpecRaw>,
    documents: Option<DocumentSpecRaw>,
    rules: BTreeMap<String, RuleConfig>,
    ignore: Vec<String>,
    format: Option<Format>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct RawProjectConfig {
    schema: Option<SchemaSpecRaw>,
    documents: Option<DocumentSpecRaw>,
    ignore: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RuleConfig {
    Severity(String),
    Tuple(Vec<serde_json::Value>),
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_json_and_normalizes_default_project() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".rglintrc.json");
        fs::write(
            &path,
            r#"{
              "schema": "schema/*.graphql",
              "documents": ["src/**/*.graphql", "tests/*.graphql"],
              "rules": {"no-deprecated": "error", "selection-set-depth": ["warn", {"maxDepth": 7}]},
              "ignore": ["**/generated/**"],
              "format": "json"
            }"#,
        )
        .unwrap();

        let config = load(&path).unwrap();
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].name, "default");
        assert_eq!(config.projects[0].ignore, vec!["**/generated/**"]);
        assert_eq!(config.format, Format::Json);
        assert_eq!(config.rules["no-deprecated"].0, Severity::Error);
        assert_eq!(config.rules["no-deprecated"].1, serde_json::json!({}));
        assert_eq!(
            config.rules["selection-set-depth"].1,
            serde_json::json!({"maxDepth": 7})
        );
        assert_eq!(config.project_configs()[0].name, "default");
    }

    #[test]
    fn loads_toml_projects_and_prepends_global_ignore() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".rglintrc.toml");
        fs::write(
            &path,
            r#"ignore = ["global/**"]
format = "sarif"

[projects.web]
schema = "schema.graphql"
documents = "src/**/*.graphql"
ignore = ["web-generated/**"]

[rules]
"naming-convention" = ["off", {}]
"#,
        )
        .unwrap();

        let config = load(&path).unwrap();
        assert_eq!(config.projects[0].name, "web");
        assert_eq!(
            config.projects[0].ignore,
            vec!["global/**", "web-generated/**"]
        );
        assert_eq!(config.format, Format::Sarif);
        assert_eq!(config.rules["naming-convention"].0, Severity::Off);
    }

    #[test]
    fn accepts_toml_rule_tuple_form_and_builds_engine_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".rglintrc.toml");
        fs::write(
            &path,
            r#"[rules]
"selection-set-depth" = ["warn", { maxDepth = 7 }]
"#,
        )
        .unwrap();

        let config = load(&path).unwrap();
        let rules = config.rules_config();
        assert_eq!(rules.rules.len(), 1);
        assert_eq!(rules.rules[0].severity, Severity::Warn);
        assert_eq!(rules.rules[0].options, serde_json::json!({"maxDepth": 7}));
    }

    #[test]
    fn discovery_prefers_closest_directory_then_name_precedence() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join(".rglintrc"), "format = 'json'").unwrap();
        fs::write(root.join("a/.rglintrc.toml"), "format = 'sarif'").unwrap();
        fs::write(root.join("a/.rglintrc.json"), r#"{"format":"json"}"#).unwrap();

        assert_eq!(discover(&nested), Some(root.join("a/.rglintrc.json")));
    }

    #[test]
    fn parse_errors_include_source_location() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".rglintrc.toml");
        fs::write(&path, "format = \"not-a-format\"\n").unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.to_string().contains("1:10"));
    }

    #[test]
    fn config_serialization_round_trips_through_json_and_toml() {
        let dir = tempdir().unwrap();
        let source = dir.path().join(".rglintrc.json");
        fs::write(
            &source,
            r#"{"projects":{"web":{"schema":"schema.graphql","documents":"src/*.graphql","ignore":["generated/**"]}},"rules":{"no-deprecated":["error",{}]},"ignore":["node_modules/**"],"format":"github"}"#,
        )
        .unwrap();
        let original = load(&source).unwrap();

        let json_path = dir.path().join("roundtrip.json");
        fs::write(&json_path, serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(load(&json_path).unwrap(), original);

        let toml_path = dir.path().join("roundtrip.toml");
        fs::write(&toml_path, toml::to_string(&original).unwrap()).unwrap();
        assert_eq!(load(&toml_path).unwrap(), original);
    }

    #[test]
    fn rejects_malformed_rule_tuples() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".rglintrc.json");
        fs::write(&path, r#"{"rules":{"rule":["warn",7]}}"#).unwrap();
        let error = load(&path).unwrap_err();
        assert!(error
            .to_string()
            .contains("tuple options must be an object"));
    }
}
