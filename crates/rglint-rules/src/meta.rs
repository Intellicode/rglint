//! Helper constructors for [`RuleMeta`].
//!
//! [`RuleMeta`] is normally produced by the `Rule` derive (a `static` built
//! with [`RuleMeta::new`]). This module adds a runtime [`RuleMetaBuilder`] for
//! tests, fixtures, and `xtask` code that constructs metadata without going
//! through the derive (e.g. for dynamic / config-described rules).

use rglint_core::{Category, RuleMeta, Severity};

/// A fluent builder for [`RuleMeta`]. `id` and `category` are required; all
/// other fields default to their spec values. Call [`build`](Self::build) to
/// produce the descriptor (non-`const` — for `const` use `RuleMeta::new`).
#[derive(Debug, Clone)]
pub struct RuleMetaBuilder {
    id: Option<&'static str>,
    category: Category,
    severity: Severity,
    docs: &'static str,
    option_schema_src: Option<&'static str>,
    default_options_src: Option<&'static str>,
    requires_schema: bool,
    requires_siblings: bool,
    deprecated: bool,
    replaced_by: Option<&'static str>,
    has_suggestions: bool,
}

impl RuleMetaBuilder {
    /// Start building a rule descriptor; `category` defaults to
    /// [`Category::Other`] until overridden.
    pub fn new(id: &'static str) -> Self {
        Self {
            id: Some(id),
            category: Category::Other,
            severity: Severity::default(),
            docs: "",
            option_schema_src: None,
            default_options_src: None,
            requires_schema: false,
            requires_siblings: false,
            deprecated: false,
            replaced_by: None,
            has_suggestions: false,
        }
    }

    /// Set the rule category. Required.
    #[must_use]
    pub fn category(mut self, c: Category) -> Self {
        self.category = c;
        self
    }

    /// Override the default severity.
    #[must_use]
    pub fn severity(mut self, s: Severity) -> Self {
        self.severity = s;
        self
    }

    /// Attach documentation.
    #[must_use]
    pub fn docs(mut self, docs: &'static str) -> Self {
        self.docs = docs;
        self
    }

    /// Provide the rule's `options` JSON-Schema source (compiled lazily).
    #[must_use]
    pub fn option_schema(mut self, src: &'static str) -> Self {
        self.option_schema_src = Some(src);
        self
    }

    /// Provide the rule's default options JSON source (parsed lazily).
    #[must_use]
    pub fn default_options(mut self, src: &'static str) -> Self {
        self.default_options_src = Some(src);
        self
    }

    /// Mark the rule as requiring a schema to run.
    #[must_use]
    pub fn requires_schema(mut self, v: bool) -> Self {
        self.requires_schema = v;
        self
    }

    /// Mark the rule as requiring sibling operations to run.
    #[must_use]
    pub fn requires_siblings(mut self, v: bool) -> Self {
        self.requires_siblings = v;
        self
    }

    /// Deprecate the rule, optionally pointing at its replacement.
    #[must_use]
    pub fn deprecated(mut self, replaced_by: Option<&'static str>) -> Self {
        self.deprecated = true;
        self.replaced_by = replaced_by;
        self
    }

    /// Declare whether the rule emits suggestions (drives `--fix`).
    #[must_use]
    pub fn has_suggestions(mut self, v: bool) -> Self {
        self.has_suggestions = v;
        self
    }

    /// Build the [`RuleMeta`]. Panics if `id` was never set.
    pub fn build(self) -> RuleMeta {
        let id = self.id.expect("RuleMetaBuilder requires an id");
        RuleMeta::new(
            id,
            self.category,
            self.severity,
            self.docs,
            self.option_schema_src,
            self.default_options_src,
            self.requires_schema,
            self.requires_siblings,
            self.deprecated,
            self.replaced_by,
            self.has_suggestions,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_roundtrip() {
        let m = RuleMetaBuilder::new("no-x")
            .category(Category::Operations)
            .severity(Severity::Error)
            .docs("no x docs")
            .requires_schema(true)
            .has_suggestions(true)
            .build();
        assert_eq!(m.id, "no-x");
        assert_eq!(m.category, Category::Operations);
        assert_eq!(m.severity, Severity::Error);
        assert_eq!(m.docs, "no x docs");
        assert!(m.requires_schema);
        assert!(m.has_suggestions);
        assert!(!m.requires_siblings);
        assert!(!m.deprecated);
        assert_eq!(m.replaced_by, None);
    }

    #[test]
    fn lazy_option_schema_compiles() {
        let m = RuleMetaBuilder::new("opts-rule")
            .option_schema(r#"{"type":"object"}"#)
            .build();
        assert!(m.option_schema().is_some(), "validator should compile");
    }

    #[test]
    fn lazy_default_options_parse() {
        let m = RuleMetaBuilder::new("opts-rule")
            .default_options(r#"{"maxDepth":3}"#)
            .build();
        let opts = m.default_options().expect("default_options should parse");
        assert_eq!(opts["maxDepth"], 3);
    }
}
