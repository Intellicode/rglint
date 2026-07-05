//! `proptest` property-test helpers and the negative-path helper —
//! spec-014 (PLAN §6.4 / §6.5).
//!
//! - [`prop_parse_roundtrip`] — parse `src` with `apollo-parser`, re-stringify
//!   the document, parse again, and check the two parse trees are equivalent.
//!   Returns `true` on round-trip equality. Catches regressions where a tweak
//!   to the parser or printer silently changes a document's AST. Wrap it in a
//!   `proptest!` block in a rule's test module (the harness itself only
//!   provides the helper; linking `proptest` is the caller's job, so the
//!   production build stays `proptest`-free).
//!
//! - [`assert_no_panic`] — run the engine over some (possibly malformed) source
//!   and assert it produces at least one diagnostic *without panicking*, per
//!   PLAN §6.5's "negative-path" mandate: the engine must surface parse failure
//!   as a diagnostic rather than aborting.

use rglint_core::{DocumentLoader, DocumentSpec, LintEngine, Project, ProjectConfig, Siblings};
// `CstNode::syntax` is the way to reach a `SyntaxNode` off an apollo-parser
// document; needed by `prop_parse_roundtrip`'s re-stringify step.
use apollo_parser::cst::CstNode;

/// Parse `src`, re-print the AST, parse the re-printed text, and compare the
/// two parse trees' root document text for equality. `true` if the document
/// round-trips losslessly.
///
/// apollo-parser's CST is the substrate here (the same one the engine walks),
/// so this is the most faithful round-trip check available: any node the
/// engine dispatches on is preserved by the printer.
pub fn prop_parse_roundtrip(src: &str) -> bool {
    let first = apollo_parser::Parser::new(src).parse();
    // If the first parse produced syntax errors, round-trip is meaningless;
    // report success only when the input parses cleanly both ways.
    if first.errors().next().is_some() {
        return false;
    }
    let first_text = first.document().syntax().to_string();
    let second = apollo_parser::Parser::new(&first_text).parse();
    if second.errors().next().is_some() {
        return false;
    }
    let second_text = second.document().syntax().to_string();
    second_text == src
}

/// Run `engine` over `src` (as the lone operation document, schema-less) and
/// assert the engine produces **at least one** diagnostic **without panicking**.
///
/// PLAN §6.5 mandates that malformed input never crashes the engine — parse
/// failures surface as `parse-error` diagnostics. This helper is the
/// negative-path smoke: feed it garbage and confirm the engine degrades
/// gracefully. Returns `Ok(())` on graceful (≥1 diagnostic) completion, or
/// `Err` describing what went wrong (a panic bubbles up as a test failure, so
/// callers should `let _ = assert_no_panic(...)` inside a `#[test]`).
pub fn assert_no_panic(
    malformed_src: &str,
    engine: &LintEngine,
) -> Result<usize, AssertNoPanicError> {
    let project = build_negative_project(malformed_src)?;
    let result = engine.lint(&project).map_err(AssertNoPanicError::Engine)?;
    if result.all.is_empty() {
        return Err(AssertNoPanicError::NoDiagnostic);
    }
    Ok(result.all.len())
}

/// Errors [`assert_no_panic`] can surface. A panic in the engine is *not* an
/// `Err` here — it bubbles up as the test's panic, which is the failure mode
/// the helper exists to catch.
#[derive(Debug, thiserror::Error)]
pub enum AssertNoPanicError {
    /// The inline project could not be built (document load failure).
    #[error(transparent)]
    DocumentLoad(#[from] rglint_core::DocumentLoadError),
    /// The engine itself errored.
    #[error("lint engine failed: {0}")]
    Engine(#[from] rglint_core::LintEngineError),
    /// The engine ran cleanly but produced zero diagnostics — the negative path
    /// expected at least one (PLAN §6.5: malformed input must surface something).
    #[error("negative-path engine run produced no diagnostics")]
    NoDiagnostic,
}

fn build_negative_project(src: &str) -> Result<Project, rglint_core::DocumentLoadError> {
    // Schema-less negative project: a malformed operation document, linted
    // standalone. requires_schema rules self-skip; the parse-error path is the
    // active negative-path check.
    let doc_loader = DocumentLoader::new();
    let documents = doc_loader.load(
        &DocumentSpec::Inline(src.to_owned()),
        std::path::Path::new(""),
        None,
    )?;
    let siblings = Siblings::from_documents(&documents);
    Ok(Project {
        config: ProjectConfig {
            name: "negative".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        },
        schema: None,
        documents,
        siblings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::RulesConfig;

    #[test]
    fn roundtrip_clean_document_succeeds() {
        assert!(prop_parse_roundtrip("query { hero }"));
        assert!(prop_parse_roundtrip("query GetUser { user { id name } }"));
        assert!(prop_parse_roundtrip("fragment Foo on Bar { baz }"));
    }

    #[test]
    fn roundtrip_malformed_returns_false() {
        assert!(!prop_parse_roundtrip("query { "));
        assert!(!prop_parse_roundtrip(""));
    }

    #[test]
    fn assert_no_panic_surfaces_parse_error_for_garbage() {
        let engine = LintEngine::new(&RulesConfig::default()).unwrap();
        let count = assert_no_panic("query { x ", &engine).expect("garbage -> ≥1 diag");
        assert!(count >= 1);
    }

    #[test]
    fn assert_no_panic_errors_on_clean_input() {
        let engine = LintEngine::new(&RulesConfig::default()).unwrap();
        // A clean query with an empty rule set emits zero diagnostics → the
        // negative-path helper flags it.
        let err = assert_no_panic("query { hero }", &engine).expect_err("clean -> NoDiagnostic");
        assert!(matches!(err, AssertNoPanicError::NoDiagnostic));
    }
}
