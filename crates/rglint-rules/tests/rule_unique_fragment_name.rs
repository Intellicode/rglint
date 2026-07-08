//! Parity test suite + cross-file unit test for the `unique-fragment-name`
//! rule (spec-017).
//!
//! The [`rglint_test_suite!`](rglint_test_harness::rglint_test_suite) macro
//! walks every `valid/` and `invalid/` case directory under the workspace
//! root's `rules-fixtures/unique-fragment-name/` tree and asserts each case
//! passes parity against the rule's actual diagnostics (count + message +
//! line + 0-based column), matching `expected.json` records. Fixtures with
//! a `sibling_documents = [...]` entry in `config.toml` load additional `.graphql`
//! files from the case directory alongside the main source so the
//! `requires_siblings` rule can see a multi-document project — see
//! `crates/rglint-test-harness/src/fixture.rs` for the layout.
//!
//! In addition to the parity suite, an extra unit test exercises the spec's
//! explicit "3 files each defining `fragment X` → exactly 2 diagnostics,
//! both naming `X`, on files 2 and 3" scenario end-to-end through the engine.

use std::fs;
use std::path::{Path, PathBuf};

use rglint_test_harness::rglint_test_suite;

// Force the linker to keep the `rglint-rules` crate's `#[derive(Rule)]`
// submissions alive in this test binary (see
// `tests/rule_no_anonymous_operations.rs` for the rationale).
#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

// Drive every case under `rules-fixtures/unique-fragment-name/{valid,invalid}/`
// through the harness, asserting parity against each case's `expected.json`.
rglint_test_suite!("unique-fragment-name", root = "../../rules-fixtures");

/// Spec-017 "Extra unit test": three files each defining `fragment X` → the
/// engine must emit exactly 2 diagnostics, both naming `X`, attributed to the
/// 2nd and 3rd files (the first occurrence is canonical and unreported). This
/// also pins the per-file attribution rule (each duplicate's diagnostics land
/// on the file owning that duplicate, not the canonical one) which the spec's
/// "Diagnostics are emitted on the file containing each duplicate" wording
/// requires.
#[test]
fn three_files_defining_same_fragment_yield_two_diagnostics_on_later_files() {
    use rglint_core::{
        DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, RuleConfig, RulesConfig,
        SchemaLoader, Severity, Siblings,
    };

    let root = std::env::temp_dir()
        .join("rglint-unique-fragment-name-extra-test")
        .join("three_files");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let same = "fragment X on U { id name }";
    // Content differs from the others so the dedup-by-hash wins: file B and C
    // each define `fragment X` but with distinct type/selection bodies — the
    // dedup keys differ (the byte content differs), so the bundle has 3 docs.
    fs::write(root.join("a.graphql"), "fragment X on WasFirst { id }\n").unwrap();
    fs::write(root.join("b.graphql"), "fragment X on Second { name }\n").unwrap();
    fs::write(
        root.join("c.graphql"),
        "fragment X on Third { description }\n",
    )
    .unwrap();
    // Sanity-check our `same` shortcut isn't actually shared content (which
    // the loader would dedup to a single doc).
    assert_ne!(
        fs::read_to_string(root.join("a.graphql")).unwrap(),
        fs::read_to_string(root.join("b.graphql")).unwrap(),
    );

    let schema_src = "type Query { _: Int } type WasFirst { id: ID! } type Second { name: String } type Third { description: String }";
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
    assert_eq!(
        siblings.fragments_all().len(),
        3,
        "three files × one fragment each → fragments_all has 3 occurrences",
    );

    let project = Project {
        config: ProjectConfig {
            name: "three-files".to_owned(),
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
            id: "unique-fragment-name".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("rule resolves");

    let result = engine.lint(&project).expect("lint runs");

    // Exactly two duplicates (occurrences 2 and 3).
    let all = &result.all;
    assert_eq!(
        all.len(),
        2,
        "expected exactly 2 `unique-fragment-name` diagnostics (one per duplicate after the first)"
    );

    // Both diagnostics must name `X` and carry the rule id.
    for d in all {
        assert_eq!(d.rule_id, "unique-fragment-name");
        assert_eq!(d.message, "Fragment \"X\" is defined multiple times");
    }

    // Attribution: sorted by (file, line, column, rule). Files come in alpha
    // order: b.graphql before c.graphql (a.graphql is canonical → no diag).
    let files: Vec<PathBuf> = all.iter().map(|d| d.file.clone()).collect();
    assert_eq!(
        files,
        vec![root.join("b.graphql"), root.join("c.graphql")],
        "diagnostics must attribute to the 2nd and 3rd files (per-file model)",
    );

    let _ = same; // silence unused-binding lint
}
