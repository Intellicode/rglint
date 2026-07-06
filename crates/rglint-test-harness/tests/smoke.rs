//! spec-014 smoke test: drive the harness end-to-end with a hand-rolled
//! `no-anonymous-operations` stand-in rule.
//!
//! No real rule exists yet (spec-016 onward); this test defines a throwaway
//! rule via `#[derive(Rule)]` that re-parses the source in `finalize` and
//! reports one diagnostic per anonymous operation. That is enough to exercise
//! the harness plumbing — fixture loading, project building, engine run,
//! parity comparison, snapshot rendering, property helpers, and the
//! `rglint_test_suite!` macro — exactly the surface every subsequent rule spec
//! will rely on.

use std::path::PathBuf;

use rglint_core::{DiagnosticBuilder, Handler, LintEngine, RuleContext, SourceFile, Span};
use rglint_derive::Rule as DeriveRule;
use rglint_test_harness::{
    assert_diagnostic_snapshot, assert_no_panic, engine_for, load_fixture, prop_parse_roundtrip,
    rglint_test_suite, run_fixture, FixtureCase,
};

/// A stand-in for the `no-anonymous-operations` rule (spec-016). Re-parses the
/// source in `finalize` (the engine's walk doesn't dispatch `on_node` for
/// derive-registered rules yet — `interested_kinds` is empty until a later spec
/// wires typed AST access — so we re-parse here, which is exactly what a
/// faithful stand-in needs) and reports one diagnostic per operation that has
/// no `Name` child.
#[derive(DeriveRule)]
#[rule(
    id = "no-anonymous-operations",
    category = "operations",
    severity = "error"
)]
struct NoAnonymousOperations;

impl NoAnonymousOperations {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoAnonHandler)
    }
}

struct NoAnonHandler;

impl Handler for NoAnonHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        // The engine's walk doesn't dispatch `on_node` for derive-registered
        // rules yet (`interested_kinds` is empty until a later spec wires typed
        // AST access), so re-parse the source here to find anonymous
        // operations. This is exactly the stand-in `no-anonymous-operations`
        // (spec-016) would do over the typed AST.
        let text = ctx.source_code().source();
        let anon_spans = collect_anonymous_operation_spans(text);
        for span in &anon_spans {
            ctx.report(DiagnosticBuilder::new(
                "no-anonymous-operations",
                PathBuf::from(ctx.source_code().path()),
                *span,
                "Anonymous operation. Give it a name.",
            ));
        }
    }
}

/// Walk `source` with `apollo-parser` and return the span of every
/// `OPERATION_DEFINITION` lacking a `Name` child. The span is the operation
/// node's full text range, matching where `graphql-eslint` points its
/// diagnostic for this rule.
fn collect_anonymous_operation_spans(source: &str) -> Vec<Span> {
    use apollo_parser::cst::CstNode;
    let tree = apollo_parser::Parser::new(source).parse();
    let root = tree.document();
    let mut out = Vec::new();
    for node in root.syntax().descendants() {
        if node.kind() == apollo_parser::SyntaxKind::OPERATION_DEFINITION {
            let has_name = node
                .children()
                .any(|c| c.kind() == apollo_parser::SyntaxKind::NAME);
            if !has_name {
                out.push(Span::from_syntax_node(&node));
            }
        }
    }
    out
}

/// Absolute path to the harness's bundled fixture tree for this rule.
fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("no-anonymous-operations")
}

fn load(case: &str, sub: &str) -> FixtureCase {
    let dir = fixtures_root().join(sub).join(case);
    load_fixture(&dir).unwrap_or_else(|e| panic!("load {sub}/{case}: {e}"))
}

#[test]
fn hand_rolled_invalid_fixture_passes_parity() {
    let case = load("01", "invalid");
    let engine = engine_for("no-anonymous-operations", case.options.clone()).unwrap();
    let outcome = run_fixture(&case, &engine).expect("parity should hold for case 01");
    assert_eq!(outcome.actual.len(), 1, "one anonymous operation");
    assert_eq!(outcome.actual[0].rule, "no-anonymous-operations");
    assert_eq!(outcome.actual[0].line, 1);
    assert_eq!(outcome.actual[0].column, 0);
}

#[test]
fn mutated_expected_message_yields_informative_failure() {
    // Load the case, then corrupt the expected message and confirm run_fixture
    // fails with a Parity error whose body names the message difference.
    let mut case = load("01", "invalid");
    case.expected[0].message = "totally wrong message".to_owned();
    let engine = engine_for("no-anonymous-operations", case.options.clone()).unwrap();
    let err = run_fixture(&case, &engine).expect_err("mutated expected must fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("message differs"),
        "parity error should mention message diff: {msg}"
    );
    assert!(
        msg.contains("Anonymous operation"),
        "parity error should show the actual message: {msg}"
    );
}

#[test]
fn valid_named_operation_case_passes() {
    let case = load("01", "valid");
    let engine = engine_for("no-anonymous-operations", case.options.clone()).unwrap();
    let outcome = run_fixture(&case, &engine).expect("named operation -> no diagnostics");
    assert!(outcome.actual.is_empty());
}

#[test]
fn two_diagnostic_snapshot_is_stable() {
    // Construct a source + two synthetic diagnostics over it and assert the
    // snapshot renderer produces a stable `.snap`. Decoupling from the engine
    // keeps the snapshot a pure check of the caret-diagram format (spec-057's
    // substrate), independent of how a particular rule emits diagnostics.
    use rglint_core::Diagnostic;
    let source = SourceFile::new(
        PathBuf::from("case.graphql"),
        "query { a }\nquery { b }\n".to_owned(),
    );
    let d1 = DiagnosticBuilder::new(
        "no-anonymous-operations",
        PathBuf::from("case.graphql"),
        Span::new(0, 6),
        "Anonymous operation. Give it a name.",
    )
    .finish();
    let d2 = DiagnosticBuilder::new(
        "no-anonymous-operations",
        PathBuf::from("case.graphql"),
        Span::new(12, 6),
        "Anonymous operation. Give it a name.",
    )
    .finish();
    let diags: Vec<Diagnostic> = vec![d1, d2];
    assert_diagnostic_snapshot(&diags, source.as_ref());
}

#[test]
fn prop_parse_roundtrip_helper_works_on_sample() {
    assert!(prop_parse_roundtrip("query GetUser { user { id name } }"));
    assert!(!prop_parse_roundtrip("query { "));
}

#[test]
fn assert_no_panic_helper_surfaces_garbage() {
    let engine = LintEngine::new(&rglint_core::RulesConfig::default()).unwrap();
    let n = assert_no_panic("query { x ", &engine).expect("garbage -> ≥1 diag");
    assert!(n >= 1);
}

// The macro-based suite: walks `tests/fixtures/no-anonymous-operations/`...
// Actually the macro uses `<CARGO_MANIFEST_DIR>/<root>/<rule_id>`. We point it
// at `tests/fixtures` so the bundled tree is discovered.
rglint_test_suite!("no-anonymous-operations", root = "tests/fixtures");
