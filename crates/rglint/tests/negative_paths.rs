//! Registry-wide malformed-input smoke tests (spec-070).
//!
//! The test is intentionally registry-driven so adding a rule automatically
//! adds it to the negative-path contract. The macro keeps the assertion shape
//! named and readable without adding a test-only parametrization dependency.

use rglint_core::{
    DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, RuleConfig, RulesConfig,
    Severity, Siblings,
};

#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

#[used]
static _FORCE_LINK_GRAPHQL_SPEC_RULES: fn() = || {
    let _ = rglint_graphql_spec::all_spec_rules();
};

macro_rules! negative_path {
    ($rule_id:expr) => {{
        let engine = LintEngine::new(&RulesConfig {
            rules: vec![RuleConfig {
                id: $rule_id.to_owned(),
                severity: Severity::Error,
                options: serde_json::Value::Null,
            }],
        })
        .unwrap_or_else(|error| panic!("engine resolves {}: {error}", $rule_id));
        let project = malformed_project();
        let result = engine
            .lint(&project)
            .unwrap_or_else(|error| panic!("{} must not panic or fail: {error}", $rule_id));
        assert!(
            result
                .all
                .iter()
                .any(|diagnostic| diagnostic.rule_id == rglint_core::PARSE_ERROR_RULE_ID),
            "{} must surface a parse-error diagnostic for malformed input",
            $rule_id
        );
    }};
}

fn malformed_project() -> Project {
    let documents = DocumentLoader::new()
        .load(
            &DocumentSpec::Inline("query Broken {".to_owned()),
            std::path::Path::new(""),
            None,
        )
        .expect("malformed source remains loadable as a diagnostic");
    let siblings = Siblings::from_documents(&documents);
    Project {
        config: ProjectConfig {
            name: "negative-path".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema: None,
        documents,
        siblings,
    }
}

#[test]
fn every_registered_rule_handles_malformed_input_without_panicking() {
    let _ = rglint_rules::all_rules();
    let _ = rglint_graphql_spec::all_spec_rules();

    let mut rule_ids: Vec<&'static str> = rglint_core::ALL_RULES
        .iter()
        .map(|entry| entry.meta.id)
        .collect();
    rule_ids.sort_unstable();
    rule_ids.dedup();
    assert!(
        !rule_ids.is_empty(),
        "the integration registry must be linked"
    );

    for rule_id in rule_ids {
        negative_path!(rule_id);
    }
}
