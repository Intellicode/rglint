//! Parity test suite + snapshot for the `no-anonymous-operations` rule
//! (spec-016).
//!
//! The [`rglint_test_suite!`](rglint_test_harness::rglint_test_suite) macro
//! walks every `valid/` and `invalid/` case directory under the workspace
//! root's `rules-fixtures/no-anonymous-operations/` tree and asserts each case
//! passes parity against the rule's actual diagnostics (count + message +
//! line + 0-based column, matching `graphql-eslint`'s `expected.json`
//! records). One `#[test]` per case is folded into a single suite-level
//! `#[test]` so the failure report lists every offending case.
//!
//! The `rules-fixtures/` directory lives at the workspace root (one level up
//! from this crate); the macro's `root = "..."` argument points at it relative
//! to the crate's `CARGO_MANIFEST_DIR` (`../../rules-fixtures`).
//!
//! The snapshot test pins one invalid case's `pretty`-reporter rendering (the
//! `^^^` caret diagram) for `insta::assert_snapshot!`.

use std::path::PathBuf;

use rglint_test_harness::{
    build_project, engine_for, load_fixture, render_snapshot, rglint_test_suite,
};

// Force the linker to keep the `rglint-rules` crate's `#[derive(Rule)]`
// submissions alive in this test binary. Without an explicit symbol
// reference, the linker dead-strips `rglint-rules`'s `linkme::distributed_slice`
// statics (the rule registrations) and the engine reports the rule id as
// unknown — `rglint_test_harness` references `rglint-core` (which holds the
// `ALL_RULES` distributed slice) but not `rglint-rules` directly. Reading the
// registry here guarantees the static is retained so the engine's lookup
// finds `no-anonymous-operations`.
#[used]
static _FORCE_LINK_RGLINT_RULES: fn() = || {
    let _ = rglint_rules::all_rules();
};

// Drive every case under `rules-fixtures/no-anonymous-operations/{valid,invalid}/`
// through the harness, asserting parity against the corresponding `expected.json`.
rglint_test_suite!("no-anonymous-operations", root = "../../rules-fixtures");

/// Snapshot (spec-016 Testing): the `<invalid>/01` case's diagnostics rendered
/// in the `pretty` reporter's `^^^`-caret format (spec-057 builds on this same
/// rendering). Pinned via `insta` so a future change to message wording, span,
/// or render style surfaces as a visible diff before merge.
#[test]
fn snapshot_invalid_01_caret_diagram() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("rules-fixtures")
        .join("no-anonymous-operations")
        .join("invalid")
        .join("01");
    let case = load_fixture(&fixture_dir).expect("invalid/01 fixture loads");
    let engine = engine_for("no-anonymous-operations", serde_json::Value::Null)
        .expect("engine resolves `no-anonymous-operations`");
    let project = build_project(&case).expect("inline project builds from invalid/01");
    let result = engine.lint(&project).expect("lint runs");
    assert!(
        !result.all.is_empty(),
        "invalid/01 must emit at least one anonymous-operation diagnostic"
    );

    // Inline documents are loaded under the synthetic `<inline>` path (see
    // `DocumentLoader`'s handling of `DocumentSpec::Inline`). The snapshot
    // renderer matches each diagnostic's `file` against the source it's given,
    // so build a `SourceFile` under that same identifier for the carets to
    // land on the right line(s) rather than being diverted to the off-source
    // bucket.
    let source = rglint_core::SourceFile::new(PathBuf::from("<inline>"), case.source.clone());
    // Render via the harness's snapshot format and pin with `insta` directly
    // (rather than `assert_diagnostic_snapshot`): the wrapper funnels the
    // `#[track_caller]` snapshot path back into the harness crate's source
    // tree, which would collide every caller's snapshot file. Calling
    // `assert_snapshot!` here keeps the `.snap` under
    // `crates/rglint-rules/tests/snapshots/`.
    let rendered = render_snapshot(&result.all, &source);
    insta::assert_snapshot!(rendered);
}
