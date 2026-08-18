//! Parity test suite for the `unique-enum-value-names` rule (spec-029).

use rglint_test_harness::rglint_test_suite;

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

rglint_test_suite!("unique-enum-value-names", root = "../../rules-fixtures");

#[test]
fn case_insensitive_duplicate_in_enum_reported() {
    use rglint_core::{
        LintEngine, LoadedDocuments, Project, ProjectConfig, RuleConfig, RulesConfig, SchemaLoader,
        SchemaSpec, Severity, Siblings,
    };

    let schema_loader = SchemaLoader::new();
    let schema = schema_loader
        .load(
            &SchemaSpec::Inline("enum A { TEST TesT }".to_owned()),
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
            name: "enum-dup".to_owned(),
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
            id: "unique-enum-value-names".to_owned(),
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
        "expected exactly 1 `unique-enum-value-names` diagnostic"
    );

    let d = &all[0];
    assert_eq!(d.rule_id, "unique-enum-value-names");
    assert_eq!(
        d.message,
        "Unexpected case-insensitive enum values duplicates for TesT"
    );
}
