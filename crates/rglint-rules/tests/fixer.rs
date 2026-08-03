//! Integration coverage for spec-061 using a real operation-side rule.

use std::path::PathBuf;

use rglint_core::{
    DocumentSpec, Fixer, LintEngine, ProjectConfig, ProjectResolver, RuleConfig, RulesConfig,
    Severity,
};

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

#[test]
fn alphabetize_fixes_two_operation_selection_swaps() {
    let dir = tempfile::tempdir().expect("temporary project directory");
    let query_path = dir.path().join("query.graphql");
    std::fs::write(
        &query_path,
        "query Example {\n  hero { zed alpha }\n  viewer { zed alpha }\n}\n",
    )
    .expect("write operation fixture");

    let config = ProjectConfig {
        name: "fixer-integration".to_owned(),
        schema: None,
        documents: Some(DocumentSpec::Files(vec![PathBuf::from("query.graphql")])),
        ignore: Vec::new(),
    };
    let mut project = ProjectResolver::new(dir.path().to_path_buf())
        .resolve(&[config])
        .expect("resolve project")
        .pop()
        .expect("one project");
    let engine = LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: "alphabetize".to_owned(),
            severity: Severity::Warn,
            options: serde_json::json!({ "selections": ["OperationDefinition"] }),
        }],
    })
    .expect("resolve rule");

    let before = engine.lint(&project).expect("initial lint");
    assert_eq!(before.all.len(), 2, "both adjacent swaps need fixing");

    let summary = Fixer::new(&engine).fix(&mut project).expect("apply fixes");
    assert_eq!(summary.passes, 1);
    assert_eq!(summary.files_changed, 1);
    assert_eq!(summary.remaining, 0);
    assert_eq!(
        std::fs::read_to_string(query_path).expect("read fixed operation"),
        "query Example {\n  hero { alpha zed }\n  viewer { alpha zed }\n}\n"
    );
}
