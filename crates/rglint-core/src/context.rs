//! The per-rule, per-document handle that rules receive in `Rule::create` and
//! `Handler::finalize` — spec-009 (PLAN §4.2).
//!
//! [`RuleContext`] is what a rule sees of the engine: the source file being
//! linted, the (optional) parsed schema, the (optional) cross-document
//! siblings index, the project config, the rule's typed options, and the
//! `report()` sink. It owns the diagnostics buffer it writes into; the engine
//! (spec-011) drains it via [`RuleContext::take_diagnostics`] after
//! `Handler::finalize` returns.
//!
//! ## Severity / `rule_id` ownership
//!
//! `rule_id` and `severity` are set by the engine when constructing the
//! context per-rule-per-file (spec-011), not by the rule itself — rules never
//! know their own configured severity (matches eslint semantics, see the spec
//! "Risks / Notes" section). [`RuleContext::report`] therefore stamps the
//! context's `rule_id` / `file` / `severity` onto every diagnostic; values a
//! rule sets on the [`DiagnosticBuilder`] for those fields are overwritten. The
//! builder's `span`, `message`, `suggestions`, and `data` are preserved
//! verbatim — those are the rule's domain. A future revision may honor an
//! explicit rule-side severity override on the builder; v1 does not, per the
//! spec's "(rare)" note.

use serde::de::DeserializeOwned;

use crate::diagnostics::{Diagnostic, DiagnosticBuilder, Severity};
use crate::node::Node;
use crate::project::ProjectConfig;
use crate::siblings::Siblings;
use crate::source::SourceFile;

/// Errors a [`RuleContext`] accessor can surface. The engine (spec-011)
/// converts these into config / runtime diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum RuleContextError {
    /// [`RuleContext::require_schema`] was called on a context whose `schema`
    /// is `None`. The engine should have skipped a `requires_schema` rule, so
    /// an `Err` here is a defensive check naming the offending `rule_id`.
    #[error("rule `{rule_id}` requires a schema but none is loaded for this project")]
    SchemaMissing {
        /// The rule id passed to `require_schema`.
        rule_id: String,
    },
    /// [`RuleContext::require_operations`] was called on a context whose
    /// `siblings` is `None` (no operations were loaded for this project).
    #[error("rule `{rule_id}` requires sibling operations but none are loaded for this project")]
    SiblingsMissing {
        /// The rule id passed to `require_operations`.
        rule_id: String,
    },
    /// [`RuleContext::option`] could not deserialize the configured options
    /// into `T`. The engine converts this into a config-error diagnostic.
    #[error("rule `{rule_id}`: invalid options: {message}")]
    OptionsInvalid {
        /// The rule id of the context.
        rule_id: String,
        /// The underlying serde error message.
        message: String,
    },
}

/// Per-rule, per-document rule execution context (PLAN §4.2 / spec-009).
///
/// Build one with [`RuleContext::new`]; the engine (spec-011) constructs a
/// fresh context for each `(rule, document)` pair. Rules receive `&mut
/// RuleContext` in [`Rule::create`](crate::Rule::create) and
/// [`Handler::finalize`](crate::Handler::finalize).
pub struct RuleContext<'a> {
    /// The source file being linted.
    pub file: &'a SourceFile,
    /// The project's parsed schema, if any. `None` for schema-less projects;
    /// rules declare `requires_schema` in their [`RuleMeta`](crate::RuleMeta) so
    /// the engine skips them up front.
    pub schema: Option<&'a apollo_compiler::Schema>,
    /// The cross-document siblings index, if available.
    pub siblings: Option<&'a Siblings>,
    /// The resolved project config.
    pub project: &'a ProjectConfig,
    /// The rule's configured options (raw JSON). Read typed via
    /// [`option`](Self::option) or raw via [`options_raw`](Self::options_raw).
    options: &'a serde_json::Value,
    /// Diagnostics buffered from [`report`](Self::report); drained by the
    /// engine via [`take_diagnostics`](Self::take_diagnostics) after
    /// `Handler::finalize`.
    diagnostics: Vec<Diagnostic>,
    /// The rule id this context was built for (set by the engine).
    rule_id: &'static str,
    /// The configured severity for this rule in this project (set by the
    /// engine; overrides whatever the rule sets on a `DiagnosticBuilder`).
    severity: Severity,
}

impl<'a> RuleContext<'a> {
    /// Construct a fresh context. Called by the engine (spec-011) once per
    /// `(rule, document)`; also used by tests.
    pub fn new(
        file: &'a SourceFile,
        schema: Option<&'a apollo_compiler::Schema>,
        siblings: Option<&'a Siblings>,
        project: &'a ProjectConfig,
        options: &'a serde_json::Value,
        rule_id: &'static str,
        severity: Severity,
    ) -> Self {
        Self {
            file,
            schema,
            siblings,
            project,
            options,
            diagnostics: Vec::new(),
            rule_id,
            severity,
        }
    }

    /// Push a diagnostic, stamping the context's `rule_id`, `file`, and
    /// configured `severity` onto it (overriding whatever the rule set on the
    /// builder). The builder's `span`, `message`, `suggestions`, and `data`
    /// are preserved verbatim.
    pub fn report(&mut self, b: DiagnosticBuilder) {
        let mut diag = b.finish();
        diag.rule_id = self.rule_id.to_owned();
        diag.file = self.file.path().to_path_buf();
        diag.severity = self.severity;
        self.diagnostics.push(diag);
    }

    /// The source file being linted.
    pub fn source_code(&self) -> &SourceFile {
        self.file
    }

    /// Returns the project schema, or an [`RuleContextError::SchemaMissing`]
    /// naming `rule_id` if the project is schema-less. The engine should have
    /// skipped a `requires_schema` rule, so an `Err` here is a defensive
    /// check.
    pub fn require_schema(
        &self,
        rule_id: &str,
    ) -> Result<&apollo_compiler::Schema, RuleContextError> {
        self.schema
            .ok_or_else(|| RuleContextError::SchemaMissing {
                rule_id: rule_id.to_owned(),
            })
    }

    /// Returns the siblings index, or an [`RuleContextError::SiblingsMissing`]
    /// naming `rule_id` if no siblings were loaded for this project.
    pub fn require_operations(
        &self,
        rule_id: &str,
    ) -> Result<&Siblings, RuleContextError> {
        self.siblings
            .ok_or_else(|| RuleContextError::SiblingsMissing {
                rule_id: rule_id.to_owned(),
            })
    }

    /// Deserialize the rule's configured `options` into a strongly-typed
    /// struct `T`. On failure returns [`RuleContextError::OptionsInvalid`]; the
    /// engine converts that into a config-error diagnostic.
    pub fn option<T: DeserializeOwned>(&self) -> Result<T, RuleContextError> {
        serde_json::from_value::<T>(self.options.clone()).map_err(|e| {
            RuleContextError::OptionsInvalid {
                rule_id: self.rule_id.to_owned(),
                message: e.to_string(),
            }
        })
    }

    /// The raw JSON value of the rule's `options`. Prefer
    /// [`option`](Self::option) for typed access.
    pub fn options_raw(&self) -> &serde_json::Value {
        self.options
    }

    /// Best-effort node-name helper. Delegates to spec-012's
    /// [`node_name`][crate::node_name] over the typed `Node`. Returns an
    /// empty string for nameless nodes (anonymous operations, `SelectionSet`,
    /// …) — the `String` (rather than `Option`) return shape is fixed here so
    /// rules can be written against it today and message-formatting code can
    /// interpolate the result directly.
    pub fn node_name(&self, node: &Node<'_>) -> String {
        crate::node_name(node).unwrap_or_default()
    }

    /// Drain the buffered diagnostics. The engine calls this once after
    /// `Handler::finalize` returns; subsequent calls return an empty `Vec`.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    /// The rule id this context was built for.
    pub fn rule_id(&self) -> &'static str {
        self.rule_id
    }

    /// The configured severity for this rule in this project.
    pub fn severity(&self) -> Severity {
        self.severity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::diagnostics::Fix;
    use crate::location::Span;
    use crate::project::ProjectConfig;

    fn make_source() -> Arc<SourceFile> {
        SourceFile::new(
            PathBuf::from("test.graphql"),
            "type Query { x: Int }".to_owned(),
        )
    }

    fn empty_project() -> ProjectConfig {
        ProjectConfig {
            name: "test".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DepthOpts {
        max_depth: u32,
    }

    #[test]
    fn option_deserializes_camel_case_json() {
        // spec-009 Testing: `{"maxDepth": 7}` -> `DepthOpts { max_depth: 7 }`.
        let src = make_source();
        let project = empty_project();
        let options = serde_json::json!({ "maxDepth": 7 });
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &options,
            "depth-rule",
            Severity::Warn,
        );
        let parsed: DepthOpts = ctx.option().expect("options deserialize");
        assert_eq!(parsed.max_depth, 7);
    }

    #[test]
    fn option_invalid_returns_options_invalid_naming_rule() {
        let src = make_source();
        let project = empty_project();
        let options = serde_json::json!({ "maxDepth": "not-a-number" });
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &options,
            "depth-rule",
            Severity::Warn,
        );
        let err = ctx
            .option::<DepthOpts>()
            .expect_err("invalid options must error");
        assert!(matches!(err, RuleContextError::OptionsInvalid { .. }));
        let msg = format!("{err}");
        assert!(
            msg.contains("depth-rule"),
            "error names the rule: {msg}"
        );
    }

    #[test]
    fn require_schema_errors_when_none_and_names_rule() {
        // spec-009 Testing: `require_schema("x")` on a schema-less context
        // returns an `Err` whose message names the rule.
        let src = make_source();
        let project = empty_project();
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "schema-rule",
            Severity::Warn,
        );
        let err = ctx
            .require_schema("schema-rule")
            .expect_err("schema missing");
        assert!(matches!(err, RuleContextError::SchemaMissing { .. }));
        let msg = format!("{err}");
        assert!(
            msg.contains("schema-rule"),
            "error names the rule: {msg}"
        );
    }

    #[test]
    fn require_operations_errors_when_none_and_names_rule() {
        let src = make_source();
        let project = empty_project();
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "siblings-rule",
            Severity::Warn,
        );
        let err = ctx
            .require_operations("siblings-rule")
            .expect_err("siblings missing");
        assert!(matches!(err, RuleContextError::SiblingsMissing { .. }));
        let msg = format!("{err}");
        assert!(
            msg.contains("siblings-rule"),
            "error names the rule: {msg}"
        );
    }

    #[test]
    fn report_accumulates_and_stamps_context_rule_id_file_severity() {
        // spec-009 Testing: `report()` 3 times -> `take_diagnostics().len()
        // == 3`, all carrying the context's `rule_id` and `file`.
        let src = make_source();
        let project = empty_project();
        let mut ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "my-rule",
            Severity::Error,
        );
        // Builders carry a *wrong* rule_id and file; report must override them
        // with the context's.
        for i in 0..3u32 {
            ctx.report(DiagnosticBuilder::new(
                "wrong-rule-id",
                PathBuf::from("wrong-file.graphql"),
                Span::new(i as usize, 1),
                format!("offense {i}"),
            ));
        }
        let diags = ctx.take_diagnostics();
        assert_eq!(diags.len(), 3);
        for d in &diags {
            assert_eq!(d.rule_id, "my-rule", "rule_id stamped from context");
            assert_eq!(
                d.file,
                PathBuf::from("test.graphql"),
                "file stamped from context"
            );
            assert_eq!(d.severity, Severity::Error, "severity stamped from context");
        }
        // Buffer is drained after `take_diagnostics`.
        assert!(ctx.take_diagnostics().is_empty(), "buffer drained");
    }

    #[test]
    fn report_preserves_builder_span_message_suggestions_data() {
        let src = make_source();
        let project = empty_project();
        let mut ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "my-rule",
            Severity::Warn,
        );
        ctx.report(
            DiagnosticBuilder::new(
                "ignored-id",
                PathBuf::from("ignored.graphql"),
                Span::new(5, 2),
                "hello",
            )
            .suggestion("fix it", Fix::Remove { span: Span::new(5, 2) })
            .data(serde_json::json!({ "k": 1 })),
        );
        let d = &ctx.take_diagnostics()[0];
        assert_eq!(d.span, Span::new(5, 2));
        assert_eq!(d.message, "hello");
        assert_eq!(d.suggestions.len(), 1);
        assert_eq!(d.data, serde_json::json!({ "k": 1 }));
    }

    #[test]
    fn accessors_return_context_fields() {
        let src = make_source();
        let project = empty_project();
        let options = serde_json::json!({ "x": 1 });
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &options,
            "rid",
            Severity::Warn,
        );
        assert_eq!(ctx.rule_id(), "rid");
        assert_eq!(ctx.severity(), Severity::Warn);
        assert!(
            std::ptr::eq(ctx.source_code(), src.as_ref()),
            "source_code returns the context's file"
        );
        assert_eq!(ctx.options_raw(), &options);
    }

    #[test]
    fn node_name_delegates_to_spec_012_helper() {
        // spec-012 lands the real `node_name`; `RuleContext::node_name` wraps
        // it as a `String` (empty for nameless nodes) so rules can interpolate
        // the result directly into messages.
        let src = make_source();
        let project = empty_project();
        let ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "rid",
            Severity::Warn,
        );
        use apollo_parser::SyntaxKind;
        // A nameless node → empty string (the `None` projection).
        let anon = Node::new(SyntaxKind::NAME);
        assert!(ctx.node_name(&anon).is_empty());
        // A named node → the name verbatim.
        let named = Node::new(SyntaxKind::OBJECT_TYPE_DEFINITION).with_name("Query");
        assert_eq!(ctx.node_name(&named), "Query");
    }

    #[test]
    fn rule_context_and_error_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RuleContext<'_>>();
        assert_send_sync::<RuleContextError>();
    }
}