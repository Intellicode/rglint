//! Interoperability with the common `graphql-config` file formats.
//!
//! This module keeps the file-facing GraphQL config model separate from
//! [`crate::schema::Config`]. GraphQL config has project maps,
//! `include`/`exclude`, and legacy filename conventions; the normalized
//! engine config only contains named projects and local path specs. Relative
//! paths remain relative to the config file until project resolution.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::schema::{Config, ConfigError, DocumentSpecRaw, ProjectConfigRaw, SchemaSpecRaw};

const GRAPHQL_CONFIG_NAMES: [&str; 7] = [
    ".graphqlrc",
    ".graphqlrc.yml",
    ".graphqlrc.yaml",
    ".graphqlrc.json",
    ".graphqlconfig",
    ".graphqlconfig.yml",
    ".graphqlconfig.json",
];

/// Search upward from `start` for a GraphQL config file.
pub fn discover_graphql_config(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        for name in GRAPHQL_CONFIG_NAMES {
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

/// Load and normalize one `.graphqlrc`/`.graphqlconfig` file.
pub fn load_graphql_config(path: &Path) -> Result<Config, ConfigError> {
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw = parse_raw(path, &source)?;
    normalize(path, raw)
}

fn parse_raw(path: &Path, source: &str) -> Result<RawGraphqlConfig, ConfigError> {
    let is_json = path.extension().and_then(|ext| ext.to_str()) == Some("json")
        || (path.extension().is_none() && source.trim_start().starts_with(['{', '[']));

    if is_json {
        serde_json::from_str(source).map_err(|error| ConfigError::Parse {
            path: path.to_path_buf(),
            line: error.line(),
            column: error.column(),
            message: error.to_string(),
        })
    } else {
        serde_yaml::from_str(source).map_err(|error| {
            let (line, column) = error
                .location()
                .map(|location| (location.line(), location.column()))
                .unwrap_or((1, 1));
            ConfigError::Parse {
                path: path.to_path_buf(),
                line,
                column,
                message: error.to_string(),
            }
        })
    }
}

fn normalize(path: &Path, raw: RawGraphqlConfig) -> Result<Config, ConfigError> {
    let projects = match raw.projects {
        Some(projects) => projects
            .into_iter()
            .map(|(name, project)| normalize_project(path, name, project))
            .collect::<Result<Vec<_>, _>>()?,
        None => normalize_top_level(path, raw.schema, raw.documents)?,
    };

    Ok(Config {
        projects,
        rules: Default::default(),
        ignore: Vec::new(),
        format: Default::default(),
    })
}

fn normalize_top_level(
    path: &Path,
    schema: Option<GraphqlSpec>,
    documents: Option<GraphqlSpec>,
) -> Result<Vec<ProjectConfigRaw>, ConfigError> {
    let mut names = BTreeSet::new();
    add_project_names(&mut names, schema.as_ref());
    add_project_names(&mut names, documents.as_ref());

    if names.is_empty() {
        return Ok(vec![normalize_project(
            path,
            "default".to_owned(),
            RawGraphqlProject {
                schema,
                documents,
                include: None,
                exclude: None,
            },
        )?]);
    }

    names
        .into_iter()
        .map(|name| {
            normalize_project(
                path,
                name.clone(),
                RawGraphqlProject {
                    schema: schema.as_ref().and_then(|spec| spec.for_project(&name)),
                    documents: documents.as_ref().and_then(|spec| spec.for_project(&name)),
                    include: None,
                    exclude: None,
                },
            )
        })
        .collect()
}

fn add_project_names(names: &mut BTreeSet<String>, spec: Option<&GraphqlSpec>) {
    if let Some(GraphqlSpec::ProjectMap(projects)) = spec {
        names.extend(projects.keys().cloned());
    }
}

fn normalize_project(
    path: &Path,
    name: String,
    project: RawGraphqlProject,
) -> Result<ProjectConfigRaw, ConfigError> {
    let schema = project
        .schema
        .as_ref()
        .and_then(|spec| spec.for_project(&name));
    let schema = match schema.as_ref() {
        Some(spec) => schema_spec(path, &name, spec)?,
        None => None,
    };

    let documents = project
        .documents
        .as_ref()
        .and_then(|spec| spec.for_project(&name));
    let documents = match documents.as_ref() {
        Some(spec) => Some(document_spec(path, &name, spec)?),
        None => project
            .include
            .as_ref()
            .map(StringOrStrings::to_document_spec)
            .transpose()?,
    };

    Ok(ProjectConfigRaw {
        name,
        schema,
        documents,
        ignore: project
            .exclude
            .map(StringOrStrings::into_vec)
            .unwrap_or_default(),
        rules: None,
    })
}

fn schema_spec(
    path: &Path,
    project: &str,
    spec: &GraphqlSpec,
) -> Result<Option<SchemaSpecRaw>, ConfigError> {
    match spec {
        GraphqlSpec::Single(value) => {
            reject_remote_schema(path, project, value)?;
            Ok(Some(SchemaSpecRaw::Single(value.clone())))
        }
        GraphqlSpec::Multiple(values) => {
            for value in values {
                reject_remote_schema(path, project, value)?;
            }
            Ok(Some(SchemaSpecRaw::Multiple(values.clone())))
        }
        GraphqlSpec::ProjectMap(values) => {
            let Some(value) = values.get(project) else {
                return Ok(None);
            };
            reject_remote_schema(path, project, value)?;
            Ok(Some(SchemaSpecRaw::Single(value.clone())))
        }
        GraphqlSpec::Http(value) => Err(ConfigError::UnsupportedRemoteSchema {
            path: path.to_path_buf(),
            project: project.to_owned(),
            url: value.http.clone(),
        }),
    }
}

fn document_spec(
    path: &Path,
    project: &str,
    spec: &GraphqlSpec,
) -> Result<DocumentSpecRaw, ConfigError> {
    match spec {
        GraphqlSpec::Single(value) => Ok(DocumentSpecRaw::Single(value.clone())),
        GraphqlSpec::Multiple(values) => Ok(DocumentSpecRaw::Multiple(values.clone())),
        GraphqlSpec::ProjectMap(values) => {
            let Some(value) = values.get(project) else {
                return Ok(DocumentSpecRaw::Multiple(Vec::new()));
            };
            Ok(DocumentSpecRaw::Single(value.clone()))
        }
        GraphqlSpec::Http(value) => Err(ConfigError::UnsupportedRemoteSchema {
            path: path.to_path_buf(),
            project: project.to_owned(),
            url: value.http.clone(),
        }),
    }
}

fn reject_remote_schema(path: &Path, project: &str, value: &str) -> Result<(), ConfigError> {
    if value.starts_with("http://") || value.starts_with("https://") {
        return Err(ConfigError::UnsupportedRemoteSchema {
            path: path.to_path_buf(),
            project: project.to_owned(),
            url: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawGraphqlConfig {
    schema: Option<GraphqlSpec>,
    documents: Option<GraphqlSpec>,
    projects: Option<BTreeMap<String, RawGraphqlProject>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawGraphqlProject {
    schema: Option<GraphqlSpec>,
    documents: Option<GraphqlSpec>,
    include: Option<StringOrStrings>,
    exclude: Option<StringOrStrings>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum GraphqlSpec {
    Single(String),
    Multiple(Vec<String>),
    Http(HttpSchema),
    ProjectMap(BTreeMap<String, String>),
}

impl GraphqlSpec {
    fn for_project(&self, project: &str) -> Option<Self> {
        match self {
            Self::ProjectMap(values) => values.get(project).cloned().map(Self::Single),
            _ => Some(self.clone()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct HttpSchema {
    http: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StringOrStrings {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrStrings {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(value) => vec![value],
            Self::Multiple(values) => values,
        }
    }

    fn to_document_spec(&self) -> Result<DocumentSpecRaw, ConfigError> {
        Ok(match self {
            Self::Single(value) => DocumentSpecRaw::Single(value.clone()),
            Self::Multiple(values) => DocumentSpecRaw::Multiple(values.clone()),
        })
    }
}
