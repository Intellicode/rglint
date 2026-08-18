//! Node.js bridge for the rglint engine (spec-071).
//!
//! The bridge deliberately accepts source strings rather than filesystem paths.
//! This keeps the N-API surface portable and makes the same function usable by
//! eslint processors, editor integrations, and tests. The core engine is built
//! with its `napi` feature, which selects its deterministic serial execution
//! path and avoids the native Rayon dependency.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rglint_core::{
    DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, RuleConfig, RulesConfig,
    SchemaLoader, SchemaSpec, Severity, Siblings,
};
use serde::Deserialize;

/// The JSON input accepted by [`lint`]. It mirrors the public TypeScript
/// declaration while keeping rule options as arbitrary JSON.
#[derive(Debug, Deserialize)]
struct LintInput {
    #[serde(default)]
    schema: Option<String>,
    documents: Vec<String>,
    #[serde(default)]
    rules: HashMap<String, Vec<serde_json::Value>>,
}

/// One compact diagnostic returned to JavaScript.
#[napi(object)]
pub struct LintResult {
    /// Configured rule id.
    pub rule_id: String,
    /// Rule diagnostic message.
    pub message: String,
    /// One-based source line.
    pub line: i64,
    /// Zero-based source column, matching the JSON reporter contract.
    pub column: i64,
    /// Source identifier supplied to the bridge.
    pub file_path: String,
}

/// Lint inline schema and operation sources synchronously.
#[napi]
pub fn lint(env: Env, input: Object) -> Result<Vec<LintResult>> {
    let input: LintInput = env
        .from_js_value(input)
        .map_err(|error| Error::from_reason(format!("invalid lint input: {error}")))?;
    lint_inline(input).map_err(Error::from_reason)
}

/// Load a disk-backed config and return its normalized JSON representation.
/// Returning the serde representation preserves arbitrary rule option values
/// and keeps this API aligned with the file-facing `Config` model.
#[napi]
pub fn load_config(env: Env, path: String) -> Result<Unknown<'static>> {
    let config = rglint_config::load(Path::new(&path))
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let value = serde_json::to_value(config)
        .map_err(|error| Error::from_reason(format!("failed to serialize config: {error}")))?;
    env.to_js_value(&value)
        .map_err(|error| Error::from_reason(format!("failed to convert config: {error}")))
}

fn lint_inline(input: LintInput) -> std::result::Result<Vec<LintResult>, String> {
    // Both registries contribute to the public built-in preset and must stay
    // linked into the N-API binary just as they are for the CLI.
    let _ = rglint_rules::all_rules();
    let _ = rglint_graphql_spec::all_spec_rules();

    let LintInput {
        schema: schema_source,
        documents: document_sources,
        rules: rule_inputs,
    } = input;
    let rules = rule_inputs
        .into_iter()
        .map(|(id, setting)| parse_rule(&id, setting))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let engine = LintEngine::new(&RulesConfig { rules })
        .map_err(|error| format!("failed to build lint engine: {error}"))?;

    let schema_loader = SchemaLoader::new();
    let schema = schema_source
        .clone()
        .map(|source| schema_loader.load(&SchemaSpec::Inline(source), Path::new(".")))
        .transpose()
        .map_err(|error| format!("failed to load inline schema: {error}"))?;

    let document_files = document_sources
        .into_iter()
        .enumerate()
        .map(|(index, source)| (PathBuf::from(format!("<document-{index}>")), source))
        .collect::<Vec<_>>();
    let document_loader = DocumentLoader::new();
    let documents = document_loader
        .load_sources(
            &document_files,
            schema.as_deref().map(|loaded| &loaded.compiler),
        )
        .map_err(|error| format!("failed to load inline documents: {error}"))?;
    let siblings = Siblings::from_documents(&documents);
    let project = Project {
        config: ProjectConfig {
            name: "inline".to_owned(),
            schema: schema_source.map(SchemaSpec::Inline),
            documents: Some(DocumentSpec::Files(
                document_files
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect(),
            )),
            ignore: Vec::new(),
        },
        schema,
        documents,
        siblings,
    };

    let result = engine
        .lint(&project)
        .map_err(|error| format!("lint failed: {error}"))?;
    Ok(result
        .all
        .into_iter()
        .map(|diagnostic| {
            let source = result.sources.get(&diagnostic.file);
            let (line, column, _, _) = source
                .map(|source| source.location_eslint(diagnostic.span))
                .unwrap_or((1, 0, 1, 0));
            LintResult {
                rule_id: diagnostic.rule_id,
                message: diagnostic.message,
                line: line as i64,
                column: column as i64,
                file_path: diagnostic.file.to_string_lossy().into_owned(),
            }
        })
        .collect())
}

fn parse_rule(
    id: &str,
    setting: Vec<serde_json::Value>,
) -> std::result::Result<RuleConfig, String> {
    if setting.len() != 2 {
        return Err(format!("rule `{id}` must be a [severity, options] tuple"));
    }
    let severity = match setting[0].as_str() {
        Some("off") => Severity::Off,
        Some("warn") => Severity::Warn,
        Some("error") => Severity::Error,
        Some(value) => return Err(format!("rule `{id}` has unknown severity `{value}`")),
        None => return Err(format!("rule `{id}` severity must be a string")),
    };
    if !setting[1].is_object() {
        return Err(format!("rule `{id}` options must be an object"));
    }
    Ok(RuleConfig {
        id: id.to_owned(),
        severity,
        options: setting[1].clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lints_inline_sources_with_json_locations() {
        let result = lint_inline(LintInput {
            schema: None,
            documents: vec!["query { hero }".to_owned()],
            rules: HashMap::from([(
                "no-anonymous-operations".to_owned(),
                vec![serde_json::json!("error"), serde_json::json!({})],
            )]),
        })
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].rule_id, "no-anonymous-operations");
        assert_eq!(result[0].line, 1);
        assert_eq!(result[0].column, 0);
        assert_eq!(result[0].file_path, "<document-0>");
    }

    #[test]
    fn rejects_malformed_rule_tuples() {
        let error = parse_rule("rule", vec![serde_json::json!("warn")]).unwrap_err();
        assert!(error.contains("[severity, options]"));
    }
}
