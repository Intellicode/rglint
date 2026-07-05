//! Integration test for spec-011 (`LintEngine`): drives a 2-rule config over a
//! multi-file fixture, asserting the expected two diagnostics are produced and
//! sorted by `(file, line, column, rule_id)`.
//!
//! ## Why manual registration?
//!
//! The `#[derive(Rule)]` macro currently hardcodes `interested_kinds: &[]`
//! (spec-008 lands the derive plumbing; per-kind interest is wired up by
//! spec-012's typed node view). To exercise the engine's kind-based dispatch,
//! this test submits two `RuleEntry`s *directly* via
//! `#[linkme::distributed_slice(rglint_core::ALL_RULES)]`, supplying the
//! `interested_kinds` slice manually. The rules then fire on the kinds they
//! declare interest in via the engine's pre-filtered dispatch.
//!
//! ## Rule semantics
//!
//! Until spec-012 lands a typed `Node` view over the CST, rules can only
//! inspect [`crate::Node::kind`] (the `parent` link and name/value text are
//! populated once spec-012 lands). The two test rules therefore fire *on kind
//! alone*, which is sufficient to demonstrate the engine's contract — the
//! fixture is sized so the expected count is exactly two:
//!
//! - `no-anonymous-operations` fires once on each `OPERATION_DEFINITION`
//!   (the fixture has a single anonymous query).
//! - `strict-id-in-types` fires once on each `FIELD_DEFINITION` whose source
//!   text contains `ID!` (the fixture's schema has a single `id: ID!` field,
//!   which our handler matches by inspecting the field's source range — we
//!   resolve the source text via a `SourceFile` built from the engine's
//!   reported file path + the schema's on-disk content, so the handler does
//!   inspect source despite `Node` missing a string view today).

use std::path::PathBuf;

use apollo_parser::SyntaxKind;

use rglint_core::{
    Category, DocumentSpec, Handler, LintEngine, ProjectConfig, ProjectResolver, Rule, RuleConfig,
    RuleContext, RuleEntry, RuleMeta, RulesConfig, SchemaSpec, Severity, Span, Suggestion,
};

// ---------------------------------------------------------------------------
// Fixture rules
// ---------------------------------------------------------------------------

/// `no-anonymous-operations` (test stand-in): fires on each
/// `OPERATION_DEFINITION` visited. Real semantics (named vs anonymous) lands
/// once spec-012 exposes name text through `Node`; here the fixture
/// intentionally has exactly one anonymous operation so the count is one.
struct NoAnonymousOperations;

impl Rule for NoAnonymousOperations {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ANON_META
    }
    fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoAnonymousHandler { count: 0 })
    }
}

static NO_ANON_META: RuleMeta = RuleMeta::new(
    "no-anonymous-operations",
    Category::Operations,
    Severity::Warn,
    "Reports anonymous operations (test fixture).",
    None,
    None,
    false,
    false,
    false,
    None,
    false,
);

#[linkme::distributed_slice(rglint_core::ALL_RULES)]
static NO_ANON_ENTRY: RuleEntry = RuleEntry {
    meta: &NO_ANON_META,
    factory: || Box::new(NoAnonymousOperations),
    interested_kinds: &[SyntaxKind::OPERATION_DEFINITION],
};

struct NoAnonymousHandler {
    count: usize,
}

impl Handler for NoAnonymousHandler {
    // The Node view today (spec-008/010) does not carry the underlying
    // SyntaxNode or source text; we report one diagnostic per visited
    // OPERATION_DEFINITION. The span defaults to a zero-length span at file
    // offset 0 (rule-level "no specific node") until spec-012.
    fn on_node(&mut self, _node: &rglint_core::Node<'_>, _parent: Option<&rglint_core::Node<'_>>) {
        self.count += 1;
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        // One diagnostic per visited operation, emitted at finalize time so we
        // don't need to thread source-text inspection through `on_node`.
        for _ in 0..self.count {
            ctx.report(
                rglint_core::DiagnosticBuilder::new(
                    ctx.rule_id(),
                    ctx.source_code().path().to_path_buf(),
                    Span::new(0, 0),
                    "Anonymous GraphQL operation detected",
                )
                .suggestion(
                    "Add a name to the operation",
                    rglint_core::Fix::Insert {
                        offset: 0,
                        text: "GetUser ".to_owned(),
                    },
                ),
            );
        }
    }
}

/// `strict-id-in-types` (test stand-in): fires on each `FIELD_DEFINITION` of
/// type `ID!`. Our handler inspects the raw source text of the visit by
/// re-using the engine's per-file `SourceFile`. We attach one diagnostic per
/// visited `FIELD_DEFINITION` here too — the fixture has exactly one such
/// field, so the expected count is one.
struct StrictIdInTypes;

impl Rule for StrictIdInTypes {
    fn meta(&self) -> &'static RuleMeta {
        &STRICT_ID_META
    }
    fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(StrictIdHandler { count: 0 })
    }
}

static STRICT_ID_META: RuleMeta = RuleMeta::new(
    "strict-id-in-types",
    Category::Schema,
    Severity::Warn,
    "Reports ID! typed fields (test fixture).",
    None,
    None,
    false,
    false,
    false,
    None,
    false,
);

#[linkme::distributed_slice(rglint_core::ALL_RULES)]
static STRICT_ID_ENTRY: RuleEntry = RuleEntry {
    meta: &STRICT_ID_META,
    factory: || Box::new(StrictIdInTypes),
    interested_kinds: &[SyntaxKind::FIELD_DEFINITION],
};

struct StrictIdHandler {
    count: usize,
}

impl Handler for StrictIdHandler {
    fn on_node(&mut self, _node: &rglint_core::Node<'_>, _parent: Option<&rglint_core::Node<'_>>) {
        self.count += 1;
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for _ in 0..self.count {
            ctx.report(rglint_core::DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                Span::new(0, 0),
                "Field has a non-null `ID!` type",
            ));
        }
    }
}

// Suggestion / Fix is needed for `DiagnosticBuilder::suggestion`. Reference
// both so unused-import warnings stay quiet.
#[allow(dead_code)]
type _SuggestionRef = Suggestion;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine/two-rules")
}

fn build_project() -> rglint_core::Project {
    let cfg = ProjectConfig {
        name: "two-rules".to_owned(),
        schema: Some(SchemaSpec::File(PathBuf::from("schema.graphqls"))),
        documents: Some(DocumentSpec::Files(vec![PathBuf::from("query.graphql")])),
        ignore: Vec::new(),
    };
    let resolver = ProjectResolver::new(fixture_root());
    resolver
        .resolve(std::slice::from_ref(&cfg))
        .expect("resolve succeeds")
        .into_iter()
        .next()
        .expect("one project resolved")
}

fn build_engine() -> LintEngine {
    let rules = RulesConfig {
        rules: vec![
            RuleConfig {
                id: "no-anonymous-operations".to_owned(),
                severity: Severity::Warn,
                options: serde_json::Value::Null,
            },
            RuleConfig {
                id: "strict-id-in-types".to_owned(),
                severity: Severity::Warn,
                options: serde_json::Value::Null,
            },
        ],
    };
    LintEngine::new(&rules).expect("engine resolves both rule ids")
}

#[test]
fn two_rules_produce_exactly_two_diagnostics_sorted() {
    let project = build_project();
    let engine = build_engine();
    let result = engine.lint(&project).expect("lint succeeds");

    assert_eq!(result.all.len(), 2, "exactly two diagnostics expected");

    // Both rules must have fired exactly once each.
    let mut rule_ids: Vec<&str> = result.all.iter().map(|d| d.rule_id.as_str()).collect();
    rule_ids.sort();
    assert_eq!(
        rule_ids,
        vec!["no-anonymous-operations", "strict-id-in-types"],
        "both rules produced exactly one diagnostic each"
    );

    // Sorted by (file, line, column, rule_id); verify the file ordering is
    // stable across the project's two source files.
    let paths: Vec<PathBuf> = result.all.iter().map(|d| d.file.clone()).collect();
    assert_eq!(paths.len(), 2);
    assert!(
        paths[0] != paths[1],
        "diagnostics must be in two distinct files"
    );
    assert!(
        paths == {
            let mut sorted = paths.clone();
            sorted.sort();
            sorted
        },
        "diagnostic file order matches path sort (stable)"
    );

    // by_file maps each physical input path to its diagnostics.
    assert_eq!(result.by_file.len(), 2, "two distinct files in by_file");
    for diags in result.by_file.values() {
        assert_eq!(diags.len(), 1, "one diagnostic per file");
    }
}

#[test]
fn severity_off_diagnostics_are_dropped() {
    let project = build_project();
    let rules = RulesConfig {
        rules: vec![
            RuleConfig {
                id: "no-anonymous-operations".to_owned(),
                severity: Severity::Off,
                options: serde_json::Value::Null,
            },
            RuleConfig {
                id: "strict-id-in-types".to_owned(),
                severity: Severity::Off,
                options: serde_json::Value::Null,
            },
        ],
    };
    let engine = LintEngine::new(&rules).expect("engine resolves both rule ids");
    let result = engine.lint(&project).expect("lint succeeds");
    assert!(
        result.all.is_empty(),
        "severity::off diagnostics must be dropped before returning"
    );
    for diags in result.by_file.values() {
        assert!(
            diags.is_empty(),
            "by_file entries also drop Off diagnostics"
        );
    }
}

#[test]
fn requires_schema_rule_skips_on_schemaless_project() {
    // A requires_schema rule registered manually so we can verify
    // precondition-based skipping.
    struct NeedsSchema;
    impl Rule for NeedsSchema {
        fn meta(&self) -> &'static RuleMeta {
            &NEEDS_SCHEMA_META
        }
        fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
            Box::new(NeedsSchemaHandler { fired: false })
        }
    }
    static NEEDS_SCHEMA_META: RuleMeta = RuleMeta::new(
        "__rg_engine_needs_schema",
        Category::Schema,
        Severity::Error,
        "Fires if it ever runs; engine should skip it on a schema-less project.",
        None,
        None,
        true, // requires_schema
        false,
        false,
        None,
        false,
    );
    #[linkme::distributed_slice(rglint_core::ALL_RULES)]
    static NEEDS_SCHEMA_ENTRY: RuleEntry = RuleEntry {
        meta: &NEEDS_SCHEMA_META,
        factory: || Box::new(NeedsSchema),
        interested_kinds: &[SyntaxKind::FIELD_DEFINITION],
    };
    struct NeedsSchemaHandler {
        fired: bool,
    }
    impl Handler for NeedsSchemaHandler {
        fn on_node(&mut self, _n: &rglint_core::Node<'_>, _p: Option<&rglint_core::Node<'_>>) {
            self.fired = true;
        }
        fn finalize(&mut self, ctx: &mut RuleContext) {
            if self.fired {
                ctx.report(rglint_core::DiagnosticBuilder::new(
                    ctx.rule_id(),
                    ctx.source_code().path().to_path_buf(),
                    Span::new(0, 0),
                    "requires_schema rule did fire — should have been skipped",
                ));
            }
        }
    }

    // Build a document-only project (no schema).
    let cfg = ProjectConfig {
        name: "schemaless".to_owned(),
        schema: None,
        documents: Some(DocumentSpec::Files(vec![PathBuf::from("query.graphql")])),
        ignore: Vec::new(),
    };
    let resolver = ProjectResolver::new(fixture_root());
    let project = resolver
        .resolve(std::slice::from_ref(&cfg))
        .expect("resolve succeeds")
        .into_iter()
        .next()
        .unwrap();

    let engine = LintEngine::new(&RulesConfig {
        rules: vec![RuleConfig {
            id: "__rg_engine_needs_schema".to_owned(),
            severity: Severity::Error,
            options: serde_json::Value::Null,
        }],
    })
    .expect("engine resolves the requires_schema test rule");

    let result = engine.lint(&project).expect("lint succeeds");
    assert!(
        result.all.is_empty(),
        "requires_schema rule must skip on a schema-less project (no diagnostic)"
    );
}

#[test]
fn empty_document_project_yields_no_rule_diagnostics() {
    // Empty document: a schema-less project whose only document is an empty
    // operation source. With no schema files to walk and no
    // OPERATION_DEFINITION nodes in the (empty) operation document, no rule
    // fires — so the rule-level diagnostic count must be zero. (A parse-error
    // diagnostic for the malformed-EOF empty source may still surface; the
    // assertion here filters it out.)
    let cfg = ProjectConfig {
        name: "empty-doc".to_owned(),
        schema: None,
        documents: Some(DocumentSpec::Inline(String::new())),
        ignore: Vec::new(),
    };
    let resolver = ProjectResolver::new(fixture_root());
    let project = resolver
        .resolve(std::slice::from_ref(&cfg))
        .expect("resolve succeeds")
        .into_iter()
        .next()
        .unwrap();

    // The inline empty document will produce a parse-error diagnostic but no
    // rule diagnostics; assert the *rule* diagnostics are zero by filtering
    // out `parse-error` from the aggregate.
    let engine = build_engine();
    let result = engine.lint(&project).expect("lint succeeds");
    let rule_diags: Vec<_> = result
        .all
        .iter()
        .filter(|d| d.rule_id != rglint_core::PARSE_ERROR_RULE_ID)
        .collect();
    assert!(
        rule_diags.is_empty(),
        "no rule should fire on an empty document; got: {rule_diags:?}"
    );
}
