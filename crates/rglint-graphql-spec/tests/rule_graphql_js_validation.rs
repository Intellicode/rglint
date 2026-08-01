//! Focused parity coverage for the Apollo-backed graphql-eslint wrappers.

use std::path::PathBuf;

use rglint_test_harness::{engine_for, load_fixture, run_fixture};

#[used]
static _FORCE_LINK_GRAPHQL_SPEC_RULES: fn() = || {
    let _ = rglint_graphql_spec::all_spec_rules();
};

const INVALID_CASES: &[(&str, &str)] = &[
    ("01", "fields-on-correct-type"),
    ("02", "known-argument-names"),
    ("03", "no-unused-variables"),
    ("04", "lone-anonymous-operation"),
    ("05", "no-fragment-cycles"),
    ("06", "scalar-leafs"),
    ("07", "variables-in-allowed-position"),
    ("08", "value-literals-of-correct-type"),
    ("09", "known-fragment-names"),
    ("10", "possible-fragment-spread"),
    ("11", "provided-required-arguments"),
    ("12", "unique-variable-names"),
];

#[test]
fn graphql_js_validation_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../rules-fixtures/graphql-js-validation");

    for &(case_id, rule_id) in INVALID_CASES {
        let dir = root.join("invalid").join(case_id);
        let case = load_fixture(&dir).unwrap_or_else(|error| panic!("{case_id}: {error}"));
        let engine = engine_for(rule_id, case.options.clone())
            .unwrap_or_else(|error| panic!("{case_id} ({rule_id}): {error}"));
        run_fixture(&case, &engine)
            .unwrap_or_else(|error| panic!("{case_id} ({rule_id}): {error}"));
    }
}

#[test]
fn graphql_js_validation_valid_sources_stay_clean() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../rules-fixtures/graphql-js-validation");
    let operation = load_fixture(&root.join("valid/01")).expect("valid operation fixture");
    let schema = load_fixture(&root.join("valid/02")).expect("valid schema fixture");

    for rule_id in [
        "fields-on-correct-type",
        "known-directives",
        "known-type-names",
        "no-unused-variables",
        "scalar-leafs",
        "variables-in-allowed-position",
    ] {
        let engine = engine_for(rule_id, operation.options.clone()).unwrap();
        run_fixture(&operation, &engine).unwrap_or_else(|error| panic!("{rule_id}: {error}"));
    }

    for rule_id in [
        "lone-schema-definition",
        "unique-directive-names",
        "unique-field-definition-names",
        "unique-operation-types",
        "unique-type-names",
    ] {
        let engine = engine_for(rule_id, schema.options.clone()).unwrap();
        run_fixture(&schema, &engine).unwrap_or_else(|error| panic!("{rule_id}: {error}"));
    }
}

#[test]
fn every_upstream_rule_id_is_registered() {
    let entries = rglint_graphql_spec::all_spec_rules();
    assert_eq!(entries.len(), 30);
    for entry in entries {
        let engine = engine_for(entry.meta.id, serde_json::Value::Null)
            .unwrap_or_else(|error| panic!("{}: {error}", entry.meta.id));
        drop(engine);
    }
}
