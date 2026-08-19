//! Integration test for spec-007 project resolution.
//!
//! Mirrors the spec's `examples/multiple-projects-graphql-config` shape: a
//! 2-project `web` + `admin` workspace (`tests/fixtures/project/multi`),
//! each with its own schema and one operation document, resolved by
//! [`rglint_core::ProjectResolver`] into independent
//! [`rglint_core::Project`]s.
//!
//! Config-file parsing (`.rglintrc` / `.graphqlrc`) is owned by spec-054 /
//! spec-055 (not yet implemented); this test builds the
//! [`rglint_core::ProjectConfig`] list inline — the form those loaders will
//! produce once wired in (per spec-007's "Dependencies" note: "for tests can
//! use inline `ProjectConfig`").

use std::path::PathBuf;

use rglint_core::{DocumentSpec, ProjectConfig, ProjectResolver, SchemaSpec};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project")
}

#[test]
fn two_project_fixture_resolves_into_independent_projects() {
    let multi = fixture_root().join("multi");
    let configs = vec![
        ProjectConfig {
            name: "web".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("web/schema.graphqls"))),
            documents: Some(DocumentSpec::Files(vec![PathBuf::from("web/doc.graphql")])),
            ignore: Vec::new(),
        },
        ProjectConfig {
            name: "admin".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("admin/schema.graphqls"))),
            documents: Some(DocumentSpec::Files(vec![PathBuf::from(
                "admin/doc.graphql",
            )])),
            ignore: Vec::new(),
        },
    ];

    let resolver = ProjectResolver::new(multi.clone());
    let projects = resolver.resolve(&configs).expect("two-project resolve");

    assert_eq!(projects.len(), 2, "both projects resolved");

    // Each project carries its own schema; the web Query has `greeting` and
    // the admin Query has `count` — schemas must not be shared between them.
    let web_schema = projects[0]
        .schema
        .as_ref()
        .expect("web has a schema")
        .clone();
    let admin_schema = projects[1]
        .schema
        .as_ref()
        .expect("admin has a schema")
        .clone();
    let web_query = web_schema
        .compiler
        .get_object("Query")
        .expect("web Query object");
    let admin_query = admin_schema
        .compiler
        .get_object("Query")
        .expect("admin Query object");
    assert!(
        web_query
            .fields
            .iter()
            .any(|(name, _)| name.as_str() == "greeting"),
        "web schema carries `greeting`"
    );
    assert!(
        admin_query
            .fields
            .iter()
            .any(|(name, _)| name.as_str() == "count"),
        "admin schema carries `count`"
    );
    assert!(
        !web_query
            .fields
            .iter()
            .any(|(name, _)| name.as_str() == "count"),
        "web schema is not the admin schema (no shared schema object)"
    );

    // Documents + siblings per project.
    assert_eq!(projects[0].documents.docs.len(), 1);
    assert_eq!(projects[1].documents.docs.len(), 1);
    assert!(projects[0].siblings.is_available());
    assert!(projects[1].siblings.is_available());
    assert_eq!(
        projects[0].siblings.operations()[0].name.as_deref(),
        Some("Greeting")
    );
    assert_eq!(
        projects[1].siblings.operations()[0].name.as_deref(),
        Some("Count")
    );
}

#[test]
fn default_synthesized_project_resolves_single_named_default() {
    // spec-007 Behavior: top-level schema/documents (no `projects` key)
    // synthesizes a single default-named project. spec-054 owns the synthesis;
    // this test drives the resolver with the synthesized config output.
    let dir = fixture_root().join("default");
    let cfg = ProjectConfig {
        name: "default".to_owned(),
        schema: Some(SchemaSpec::File(PathBuf::from("schema.graphqls"))),
        documents: Some(DocumentSpec::Files(vec![PathBuf::from("doc.graphql")])),
        ignore: Vec::new(),
    };
    let resolver = ProjectResolver::new(dir);
    let projects = resolver
        .resolve(std::slice::from_ref(&cfg))
        .expect("default resolve");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].config.name, "default");
    assert!(projects[0].schema.is_some());
    assert_eq!(projects[0].documents.docs.len(), 1);
}
