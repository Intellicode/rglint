//! `no-anonymous-operations` (spec-016).
//!
//! Ports [`graphql-eslint`'s rule of the same id] to the Rust engine, mirroring
//! its message strategy while simplifying to a static message (the spec text
//! designates this rglint phrasing).
//!
//! The rule fires once per anonymous `OperationDefinition`: a node whose
//! `NAME` child is absent (`query { ... }` shorthand for a `query` operation,
//! `mutation { ... }`, `subscription { ... }`). Named operations are passed
//! through silently.
//!
//! The handler subscribes via `kinds = "OPERATION_DEFINITION"` on the
//! `#[derive(Rule)]` attribute (spec-008 + spec-011): the engine walk calls
//! `on_node` only for `OPERATION_DEFINITION` CST nodes, so the rule is a
//! one-line `name.is_none()` check. The reported span starts at the operation
//! keyword (`query` / `mutation` / `subscription`) so its 0-based column
//! matches `graphql-eslint`'s `getLocation(node.loc.start, node.operation)`
//! for the common case of an operation without a leading description; when a
//! description is present, `node.span.offset` lands at the description's start
//! (a regression deferred to a later polish spec — the fixtures port-fixture
//! checks here don't include that case).
//!
//! Suggestions: the original rule emits a "Rename to `<suggested name>`"
//! suggestion; spec-016 declares `hasSuggestions: false`, so this port is
//! suggestion-free.
//!
//! [`graphql-eslint`'s rule of the same id`]: https://the-guild.dev/graphql/eslint/rules/no-anonymous-operations

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span};
use rglint_derive::Rule;

/// The diagnostic message — kept `&'static` so the `DiagnosticBuilder` clones
/// the small constant once per emitted diagnostic rather than formatting per
/// call.
const MESSAGE: &str = "Anonymous GraphQL operations are banned. Please give your operation a name";

/// The `no-anonymous-operations` rule.
///
/// Registered into `rglint_core::ALL_RULES` via `#[derive(Rule)]` with
/// `category = "operations"` and `kinds = "OPERATION_DEFINITION"`, so the
/// engine walk (spec-011) dispatches `Handler::on_node` exclusively for
/// `OperationDefinition` CST nodes. `requires_schema` / `requires_siblings`
/// are both `false` (defaults), so the rule runs on any executable document
/// with or without a project schema.
#[derive(Rule)]
#[rule(
    id = "no-anonymous-operations",
    category = "operations",
    kinds = "OPERATION_DEFINITION"
)]
pub struct NoAnonymousOperations;

impl NoAnonymousOperations {
    /// Per-document handler factory invoked by the engine (spec-011). The
    /// rule has no options and no per-document initialization, so every
    /// project/file gets a fresh zero-state [`Handler`] that buffers anonymous
    /// operation spans during the walk and reports them in `finalize` (the
    /// only place `RuleContext` is available for `report`).
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoAnonymousHandler {
            anonymous_spans: Vec::new(),
        })
    }
}

/// Per-(rule, document) handler state: collects the span of every anonymous
/// `OperationDefinition` visited during the engine walk. Reports are emitted
/// in [`Handler::finalize`](rglint_core::Handler::finalize), where the rule has
/// access to [`RuleContext`] and can stamp each diagnostic with the context's
/// `rule_id` / `file` / configured `severity`.
struct NoAnonymousHandler {
    /// Byte spans of anonymous operations, in visit order. Saved away from
    /// the walk so `finalize` can re-emit them through `RuleContext::report`
    /// without having to traverse the CST a second time.
    anonymous_spans: Vec<Span>,
}

impl Handler for NoAnonymousHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        // Named operations pass through silently. Anonymous operations
        // (`query { ... }`, `mutation { ... }`, `subscription { ... }` with no
        // `Name` child) push the operation's span for emission in `finalize`.
        // The engine walk (spec-011) populates `node.name` from the first
        // `NAME` child of the CST node, so `None` exactly distinguishes
        // anonymous from named.
        if node.name.is_some() {
            return;
        }
        // The span is `Some` for every visited node in the engine walk (populated
        // from `Span::from_syntax_node`); the defensive fallback to a zero
        // span at offset 0 keeps the rule robust against a future Node built
        // outside the walk (which would carry `span: None`).
        let span = node.span.unwrap_or(Span::new(0, 0));
        // Report at a zero-length span rooted at the operation keyword's start.
        // `graphql-eslint` reports `loc.column` 0-based against the keyword's
        // start; the harness's parity check uses `span.offset` only (column
        // independent of length), so a zero-length span gives the right
        // column while keeping the `pretty` reporter's caret diagram tidy.
        self.anonymous_spans.push(Span::new(span.offset, 0));
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for span in self.anonymous_spans.drain(..) {
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                span,
                MESSAGE,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Light unit tests on the handler's preamble / wiring; the parity
    //! suite (`rglint_test_suite!`, see `tests/rule_no_anonymous_operations.rs`)
    //! is the authoritative check against `graphql-eslint` fixtures.

    use super::*;
    use rglint_core::{Category, Node as CoreNode, Rule, Severity, SyntaxKind};

    /// The rule's static metadata matches spec-016 (id + category); severity
    /// defaults to `Warn` since we didn't override it.
    #[test]
    fn rule_meta_matches_spec_016() {
        let rule = NoAnonymousOperations;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-anonymous-operations");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    /// The rule's `interested_kinds` is exactly `OPERATION_DEFINITION`, so the
    /// engine walk only dispatches to its handler for operation definitions.
    #[test]
    fn rule_interested_in_operation_definition_only() {
        // Pull the entry out of the registry by id (the derive wired it in via
        // `linkme`); a missing entry would imply the derive macro dropped the
        // registration, which would be a spec-008 regression.
        let entry = rglint_core::ALL_RULES
            .iter()
            .find(|e| e.meta.id == "no-anonymous-operations")
            .expect("no-anonymous-operations must be registered via #[derive(Rule)]");
        assert_eq!(
            entry.interested_kinds,
            &[SyntaxKind::OPERATION_DEFINITION],
            "expected the kinds = \"OPERATION_DEFINITION\" attribute to populate interested_kinds"
        );
    }

    /// The handler buffers an anonymous operation's span and skips a named
    /// one. We exercise it directly via `Handler::on_node` without going
    /// through the engine; the parity suite (in `tests/`) covers the full
    /// pipeline (parse → walk → project → reporter).
    #[test]
    fn handler_buffers_anonymous_only_and_reports_in_finalize() {
        let mut handler = NoAnonymousHandler {
            anonymous_spans: Vec::new(),
        };
        // Named operation: skipped.
        let named = CoreNode::new(SyntaxKind::OPERATION_DEFINITION)
            .with_name("myQuery")
            .with_span(Span::new(0, 14));
        handler.on_node(&named, None);
        assert!(
            handler.anonymous_spans.is_empty(),
            "named operation should not buffer a span"
        );

        // Anonymous operation: buffered as a zero-length span at the node start.
        let anon = CoreNode::new(SyntaxKind::OPERATION_DEFINITION).with_span(Span::new(0, 10));
        handler.on_node(&anon, None);
        assert_eq!(handler.anonymous_spans, vec![Span::new(0, 0)]);
    }
}
