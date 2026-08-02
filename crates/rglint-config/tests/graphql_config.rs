use std::fs;

use rglint_config::{discover_graphql_config, load_graphql_config, ConfigError};
use tempfile::tempdir;

#[test]
fn loads_multi_project_yaml_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/graphqlrc/multi.yaml");
    let config = load_graphql_config(&path).expect("multi-project GraphQL config loads");

    assert!(config.rules.is_empty());
    assert_eq!(config.projects.len(), 2);
    assert_eq!(config.projects[0].name, "admin");
    assert_eq!(config.projects[1].name, "web");
    assert_eq!(config.projects[0].ignore, vec!["admin/generated/**"]);
    assert_eq!(
        config.projects[0].schema.as_ref().unwrap(),
        &rglint_config::SchemaSpecRaw::Single("admin/schema.graphqls".to_owned())
    );
    assert_eq!(
        config.projects[1].documents.as_ref().unwrap(),
        &rglint_config::DocumentSpecRaw::Single("web/**/*.graphql".to_owned())
    );
}

#[test]
fn loads_legacy_json_fixture_as_default_project() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/graphqlrc/.graphqlconfig.json");
    let config = load_graphql_config(&path).expect("legacy GraphQL config loads");

    assert_eq!(config.projects.len(), 1);
    assert_eq!(config.projects[0].name, "default");
    assert_eq!(
        config.projects[0].schema.as_ref().unwrap(),
        &rglint_config::SchemaSpecRaw::Single("schema.graphql".to_owned())
    );
    assert_eq!(
        config.projects[0].documents.as_ref().unwrap(),
        &rglint_config::DocumentSpecRaw::Multiple(vec![
            "src/**/*.graphql".to_owned(),
            "tests/**/*.graphql".to_owned()
        ])
    );
}

#[test]
fn rejects_http_schema_object_with_dedicated_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".graphqlrc.yml");
    fs::write(
        &path,
        "schema:\n  http: https://example.test/schema.graphql\n",
    )
    .unwrap();

    let error = load_graphql_config(&path).unwrap_err();
    assert!(matches!(error, ConfigError::UnsupportedRemoteSchema { .. }));
    assert!(error
        .to_string()
        .contains("HTTP schema documents not supported yet"));
}

#[test]
fn maps_top_level_project_values_and_include_exclude() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(".graphqlrc.yml");
    fs::write(
        &path,
        "schema:\n  web: web/schema.graphql\n  admin: admin/schema.graphql\ndocuments:\n  web: web/**/*.graphql\n  admin: admin/**/*.graphql\n",
    )
    .unwrap();
    let config = load_graphql_config(&path).unwrap();
    assert_eq!(config.projects.len(), 2);
    assert_eq!(config.projects[0].name, "admin");
    assert_eq!(config.projects[1].name, "web");

    let include_path = dir.path().join("include.graphqlrc");
    fs::write(
        &include_path,
        "projects:\n  web:\n    schema: schema.graphql\n    include: src/**/*.graphql\n    exclude: generated/**\n",
    )
    .unwrap();
    let config = load_graphql_config(&include_path).unwrap();
    assert_eq!(
        config.projects[0].documents.as_ref().unwrap(),
        &rglint_config::DocumentSpecRaw::Single("src/**/*.graphql".to_owned())
    );
    assert_eq!(config.projects[0].ignore, vec!["generated/**"]);
}

#[test]
fn discovers_nearest_graphql_config_with_name_precedence() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("packages/web/src");
    fs::create_dir_all(&nested).unwrap();
    fs::write(dir.path().join(".graphqlconfig"), "schema: root.graphql\n").unwrap();
    fs::write(
        dir.path().join("packages/.graphqlrc"),
        "schema: package.graphql\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/.graphqlrc.json"),
        "{\"schema\":\"package-json.graphql\"}",
    )
    .unwrap();

    assert_eq!(
        discover_graphql_config(&nested),
        Some(dir.path().join("packages/.graphqlrc"))
    );
}
