//! Parity test suite + cross-file unit test for the `unique-operation-name`
//! rule (spec-018).

use std::fs;
use std::path::Path;

use rglint_test_harness::rglint_test_suite;

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

rglint_test_suite!("unique-operation-name", root = "../../rules-fixtures");

/// Spec-018 "Extra unit test": two files each defining `query Foo` + one
/// anonymous operation → exactly 1 diagnostic, naming `Foo`, on the 2nd file.
#[test]
fn two_files_with_same_operation_name_yield_one_diagnostic_on_second_file() {
    use rglint_core::{
        DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, RuleConfig, RulesConfig,
        SchemaLoader, Severity, Siblings,
    };

    let root = std::env::temp_dir()
        .join("rglint-unique-operation-name-extra-test")
        .join("two_files");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.graphql"), "query Foo { a }\n").unwrap();
    fs::write(root.join("b.graphql"), "query Foo { b }\n").unwrap();
    fs::write(root.join("c.graphql"), "query { c }\n").unwrap();

    let schema_loader = SchemaLoader::new();
    let schema = schema_loader
        .load(
            &rglint_core::SchemaSpec::Inline("type Query { a: Int b: Int c: Int }".to_owned()),
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
    assert_eq!(siblings.operations().len(), 3, "3 ops across 3 files");

    let project = Project {
        config: ProjectConfig {
            name: "two-files".to_owned(),
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
            id: "unique-operation-name".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("rule resolves");

    let result = engine.lint(&project).expect("lint runs");

    // Exactly one duplicate (occurrence 2; occurrence 1 canonical; anonymous skipped).
    let all = &result.all;
    assert_eq!(
        all.len(),
        1,
        "expected exactly 1 `unique-operation-name` diagnostic"
    );

    let d = &all[0];
    assert_eq!(d.rule_id, "unique-operation-name");
    assert_eq!(d.message, "Operation \"Foo\" is defined multiple times");
    assert_eq!(
        d.file,
        root.join("b.graphql"),
        "diagnostic must attribute to the 2nd file (per-file model)",
    );
}
