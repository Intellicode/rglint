//! Cross-cutting engine and configuration invariants (spec-070).
//!
//! These tests intentionally exercise the public integration boundary: config
//! normalization, project resolution, registry linking, and `LintEngine`.
//! Rule-specific behavior remains owned by the rule fixture suites.

use std::fs;
use std::path::PathBuf;

use rglint_config::{Config, Format};
use rglint_core::{
    DocumentSpec, LintEngine, Project, ProjectConfig, ProjectResolver, RuleConfig, RulesConfig,
    SchemaSpec, Severity,
};
use tempfile::tempdir;

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

#[used]
static _FORCE_LINK_GRAPHQL_SPEC_RULES: fn() = || {
    let _ = rglint_graphql_spec::all_spec_rules();
};

fn inline_project(name: &str, source: &str) -> Project {
    let config = ProjectConfig {
        name: name.to_owned(),
        schema: None,
        documents: Some(DocumentSpec::Inline(source.to_owned())),
        ignore: Vec::new(),
    };
    ProjectResolver::new(PathBuf::from("."))
        .resolve(&[config])
        .expect("inline project resolves")
        .into_iter()
        .next()
        .expect("one inline project")
}

fn engine_for(id: &str, severity: Severity, options: serde_json::Value) -> LintEngine {
    LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: id.to_owned(),
            severity,
            options,
        }],
    })
    .unwrap_or_else(|error| panic!("engine resolves {id}: {error}"))
}

#[test]
fn disabling_rule_via_normalized_config_suppresses_diagnostics() {
    let config = Config {
        projects: Vec::new(),
        rules: Default::default(),
        ignore: Vec::new(),
        format: Format::Pretty,
    };
    let result = engine_for_config(&config).lint(&inline_project("disabled", "query { field }"));

    assert!(result.expect("disabled config lints").all.is_empty());
}

fn engine_for_config(config: &Config) -> LintEngine {
    LintEngine::new(&config.rules_config()).expect("normalized config resolves")
}

#[test]
fn parse_errors_abort_only_the_malformed_file() {
    let root = tempdir().expect("temporary workspace");
    let bad = root.path().join("bad.graphql");
    let good = root.path().join("good.graphql");
    fs::write(&bad, "query Broken {").expect("write malformed operation");
    fs::write(&good, "query { field }").expect("write valid operation");

    let project_config = ProjectConfig {
        name: "parse-errors".to_owned(),
        schema: None,
        documents: Some(DocumentSpec::Files(vec![bad.clone(), good.clone()])),
        ignore: Vec::new(),
    };
    let project = ProjectResolver::new(root.path().to_path_buf())
        .resolve(&[project_config])
        .expect("project resolves with per-file parse errors")
        .into_iter()
        .next()
        .expect("one project");
    let result = engine_for(
        "no-anonymous-operations",
        Severity::Warn,
        serde_json::Value::Null,
    )
    .lint(&project)
    .expect("lint continues after one malformed file");

    let bad_diagnostics: Vec<_> = result
        .all
        .iter()
        .filter(|diagnostic| diagnostic.file == bad)
        .collect();
    assert!(
        !bad_diagnostics.is_empty(),
        "bad file reports a parse error"
    );
    assert!(bad_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule_id == rglint_core::PARSE_ERROR_RULE_ID));
    assert!(result.all.iter().any(
        |diagnostic| diagnostic.file == good && diagnostic.rule_id == "no-anonymous-operations"
    ));
}

#[test]
fn multi_project_lints_are_isolated() {
    let root = tempdir().expect("temporary workspace");
    let web_schema = root.path().join("web.graphqls");
    let web_doc = root.path().join("web.graphql");
    let admin_schema = root.path().join("admin.graphqls");
    let admin_doc = root.path().join("admin.graphql");
    fs::write(
        &web_schema,
        "type Query { user: User } type User { id: String! }",
    )
    .expect("write web schema");
    fs::write(&web_doc, "query Web { user { id } }").expect("write web document");
    fs::write(&admin_schema, "type Query { name: String }").expect("write admin schema");
    fs::write(&admin_doc, "query Admin { name }").expect("write admin document");

    let configs = vec![
        ProjectConfig {
            name: "web".to_owned(),
            schema: Some(SchemaSpec::File(web_schema)),
            documents: Some(DocumentSpec::Files(vec![web_doc])),
            ignore: Vec::new(),
        },
        ProjectConfig {
            name: "admin".to_owned(),
            schema: Some(SchemaSpec::File(admin_schema)),
            documents: Some(DocumentSpec::Files(vec![admin_doc])),
            ignore: Vec::new(),
        },
    ];
    let projects = ProjectResolver::new(root.path().to_path_buf())
        .resolve(&configs)
        .expect("projects resolve independently");
    let engine = engine_for(
        "strict-id-in-types",
        Severity::Warn,
        serde_json::Value::Null,
    );
    let web = engine.lint(&projects[0]).expect("web lint");
    let admin = engine.lint(&projects[1]).expect("admin lint");

    assert_eq!(web.all.len(), 1, "web's ID field is reported");
    assert!(admin.all.is_empty(), "admin cannot inherit web's schema");
}

#[test]
fn severity_off_produces_zero_diagnostics() {
    let result = engine_for(
        "no-anonymous-operations",
        Severity::Off,
        serde_json::Value::Null,
    )
    .lint(&inline_project("off", "query { field }"))
    .expect("off lint");
    assert!(result.all.is_empty());
    assert!(result.by_file.values().all(Vec::is_empty));
}

#[test]
fn requires_schema_rule_self_skips_without_schema() {
    let result = engine_for(
        "strict-id-in-types",
        Severity::Error,
        serde_json::Value::Null,
    )
    .lint(&inline_project("schema-less", "query { id }"))
    .expect("schema-less lint");
    assert!(result.all.is_empty());
}

#[test]
fn option_schema_rejects_malformed_options_at_validation_boundary() {
    let entry = rglint_rules::all_rules()
        .iter()
        .find(|entry| entry.meta.id == "require-selections")
        .expect("require-selections is registered");
    assert!(entry.meta.option_schema().is_some());

    let config = Config {
        projects: Vec::new(),
        rules: [(
            "require-selections".to_owned(),
            (Severity::Error, serde_json::json!({"selectionSet": 7})),
        )]
        .into_iter()
        .collect(),
        ignore: Vec::new(),
        format: Format::Pretty,
    };
    let error = config
        .validate(&[entry])
        .expect_err("malformed options must be rejected");
    match error {
        rglint_config::ConfigError::InvalidRuleOptions { errors } => {
            assert!(!errors.is_empty());
            assert!(errors
                .iter()
                .all(|error| error.rule_id == "require-selections"));
        }
        other => panic!("unexpected validation error: {other:?}"),
    }
}
