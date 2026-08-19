//! Parity test suite for the `no-duplicate-fields` rule (spec-019).
//!
//! The [`rglint_test_suite!`](rglint_test_harness::rglint_test_suite) macro
//! walks every `valid/` and `invalid/` case directory under the workspace
//! root's `rules-fixtures/no-duplicate-fields/` tree and asserts each case
//! passes parity against the rule's actual diagnostics (count + message +
//! line + 0-based column), matching `expected.json` records.
//!
//! In addition to the parity suite, an extra unit test exercises the spec's
//! "within a type, duplicate fields are reported" scenario end-to-end through
//! the engine.

use rglint_test_harness::rglint_test_suite;

// Force the linker to keep the `rglint-rules` crate's `#[derive(Rule)]`
// submissions alive in this test binary.
#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

// Drive every case under `rules-fixtures/no-duplicate-fields/{valid,invalid}/`
// through the harness, asserting parity against each case's `expected.json`.
rglint_test_suite!("no-duplicate-fields", root = "../../rules-fixtures");

/// Spec-019 "Extra unit test": a schema type with two fields sharing the same
/// name → exactly 1 diagnostic on the duplicate occurrence.
#[test]
fn duplicate_field_in_schema_type_reported_once() {
    use rglint_core::{
        LintEngine, LoadedDocuments, Project, ProjectConfig, RuleConfig, RulesConfig, SchemaLoader,
        SchemaSpec, Severity, Siblings,
    };

    let schema_loader = SchemaLoader::new();
    let schema = schema_loader
        .load(
            &SchemaSpec::Inline("type Query { a: Int a: Int }".to_owned()),
            std::path::Path::new(""),
        )
        .expect("schema loads");

    let documents = LoadedDocuments {
        docs: Vec::new(),
        by_file: std::collections::HashMap::new(),
    };

    let siblings = Siblings::from_documents(&documents);

    let project = Project {
        config: ProjectConfig {
            name: "schema-dup".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema: Some(schema),
        documents,
        siblings,
    };

    let engine = LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: "no-duplicate-fields".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("rule resolves");

    let result = engine.lint(&project).expect("lint runs");

    let all = &result.all;
    assert_eq!(
        all.len(),
        1,
        "expected exactly 1 `no-duplicate-fields` diagnostic"
    );

    let d = &all[0];
    assert_eq!(d.rule_id, "no-duplicate-fields");
    assert_eq!(d.message, "Field \"a\" is defined multiple times");
}

/// Spec-019 "Extra unit test": a selection set with two fields sharing the
/// same name → exactly 1 diagnostic on the duplicate occurrence.
#[test]
fn duplicate_field_in_selection_set_reported_once() {
    use std::fs;
    use std::path::Path;

    use rglint_core::{
        DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, RuleConfig, RulesConfig,
        SchemaLoader, Severity, Siblings,
    };

    let root = std::env::temp_dir()
        .join("rglint-no-duplicate-fields-extra-test")
        .join("selection_dup");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("op.graphql"), "{ a a }\n").unwrap();

    let schema_src = "type Query { a: Int }";
    let schema_loader = SchemaLoader::new();
    let schema = schema_loader
        .load(
            &rglint_core::SchemaSpec::Inline(schema_src.to_owned()),
            Path::new(""),
        )
        .expect("schema loads");

    let doc_loader = DocumentLoader::new();
    let documents = doc_loader
        .load(
            &DocumentSpec::Glob("*.graphql".to_owned()),
            &root,
            Some(&schema.compiler),
        )
        .expect("documents load");

    let siblings = Siblings::from_documents(&documents);

    let project = Project {
        config: ProjectConfig {
            name: "selection-dup".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema: Some(schema),
        documents,
        siblings,
    };

    let engine = LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: "no-duplicate-fields".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("rule resolves");

    let result = engine.lint(&project).expect("lint runs");

    // Exactly one duplicate.
    let all = &result.all;
    assert_eq!(
        all.len(),
        1,
        "expected exactly 1 `no-duplicate-fields` diagnostic"
    );

    let d = &all[0];
    assert_eq!(d.rule_id, "no-duplicate-fields");
    assert_eq!(d.message, "Field \"a\" is selected multiple times");
}
