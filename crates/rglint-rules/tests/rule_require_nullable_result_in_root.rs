use rglint_core::{
    LintEngine, LoadedDocuments, Project, ProjectConfig, RuleConfig, RulesConfig, SchemaLoader,
    SchemaSpec, Severity, Siblings,
};
use rglint_test_harness::rglint_test_suite;

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

rglint_test_suite!(
    "require-nullable-result-in-root",
    root = "../../rules-fixtures"
);

#[test]
fn emits_remove_suggestion_for_trailing_bang() {
    let schema_loader = SchemaLoader::new();
    let schema = schema_loader
        .load(
            &SchemaSpec::Inline("type Query { user: User! } type User { id: ID! }".to_owned()),
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
            name: "fixture:suggestion".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema: Some(schema),
        documents,
        siblings,
    };
    let result = LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: "require-nullable-result-in-root".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("rule resolves")
    .lint(&project)
    .expect("lint runs");

    let diag = result.all.first().expect("diagnostic");
    assert_eq!(
        diag.message,
        "Unexpected non-null result type \"User\" in type \"Query\""
    );
    assert_eq!(diag.suggestions.len(), 1);
    assert_eq!(diag.suggestions[0].desc, "Make type \"User\" nullable");
}
