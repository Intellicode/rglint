//! `unique-operation-name` (spec-018).
//!
//! Ports [`graphql-eslint`'s rule of the same id`]: across every sibling
//! document in a project, no two **named** operations may share a name.
//! Anonymous operations (spec-016 covers those) are ignored.
//!
//! ## Detection strategy
//!
//! Mirrors `unique-fragment-name` (spec-017): the rule does its work in
//! [`Handler::finalize`] against the sibling index. [`Siblings::operations`]
//! returns every operation across the bundle in iteration order (including
//! cross-file duplicate names); anonymous entries (name `None`) are skipped.
//!
//! For each operation name we count its occurrences; the first occurrence is
//! canonical (not reported) and every *subsequent* occurrence is reported on
//! the file it lives in, at the operation definition's span, with:
//!
//! ```text
//! Operation "${name}" is defined multiple times
//! ```

use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, RuleContext};
use rglint_derive::Rule;

/// The `unique-operation-name` rule.
///
/// Registered via `#[derive(Rule)]` with `category = "operations"` and
/// `requires_siblings = true`. No per-node handler needed — the work is
/// done in `finalize` against the sibling index.
#[derive(Rule)]
#[rule(
    id = "unique-operation-name",
    category = "operations",
    requires_siblings = true
)]
pub struct UniqueOperationName;

impl UniqueOperationName {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(UniqueOperationNameHandler)
    }
}

struct UniqueOperationNameHandler;

impl Handler for UniqueOperationNameHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(siblings) = ctx.siblings else {
            return;
        };
        let operations = siblings.operations();
        if operations.is_empty() {
            return;
        }

        // Count occurrences per operation name (skip anonymous).
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for op in operations {
            let Some(name) = op.name.as_deref() else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }

        // Second pass: report duplicates after the first for this file.
        let mut occurrence: HashMap<&str, usize> = HashMap::new();
        let this_file = ctx.source_code().path().to_path_buf();
        for op in operations {
            let Some(name) = op.name.as_deref() else {
                continue;
            };
            let idx = occurrence.entry(name).or_insert(0);
            *idx += 1;
            let total = *counts.get(name).unwrap_or(&0);
            if total <= 1 {
                continue;
            }
            if *idx == 1 {
                continue;
            }
            if op.source.path() != this_file {
                continue;
            }
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                op.span,
                format!("Operation \"{}\" is defined multiple times", name),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_018() {
        let rule = UniqueOperationName;
        let meta = rule.meta();
        assert_eq!(meta.id, "unique-operation-name");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn finalize_is_noop_without_siblings() {
        use rglint_core::{ProjectConfig, SourceFile};
        use std::path::PathBuf;
        use std::sync::Arc;

        let src = Arc::new(SourceFile::new(
            PathBuf::from("a.graphql"),
            "query Foo { a }".to_owned(),
        ));
        let project = ProjectConfig {
            name: "p".to_owned(),
            schema: None,
            documents: None,
            ignore: Vec::new(),
        };
        let mut ctx = RuleContext::new(
            &src,
            None,
            None,
            &project,
            &serde_json::Value::Null,
            "unique-operation-name",
            Severity::Warn,
        );
        let mut h = UniqueOperationNameHandler;
        h.finalize(&mut ctx);
        assert!(ctx.take_diagnostics().is_empty());
    }

    #[test]
    fn operations_exposes_duplicate_names() {
        use rglint_core::{DocumentLoader, DocumentSpec};
        use std::collections::HashMap;
        use std::path::Path;

        let loader = DocumentLoader::new();
        let a = "query Foo { a }".to_owned();
        let b = "query Foo { b }".to_owned();
        let c = "query Bar { c }".to_owned();
        let mut docs = loader
            .load(&DocumentSpec::Inline(a), Path::new("a.graphql"), None)
            .expect("load a");
        let more_b = loader
            .load(&DocumentSpec::Inline(b), Path::new("b.graphql"), None)
            .expect("load b");
        let more_c = loader
            .load(&DocumentSpec::Inline(c), Path::new("c.graphql"), None)
            .expect("load c");
        docs.docs.extend(more_b.docs);
        docs.docs.extend(more_c.docs);
        let _ = docs.by_file;

        let siblings = rglint_core::Siblings::from_documents(&docs);
        let mut by_name: HashMap<&str, usize> = HashMap::new();
        for op in siblings.operations() {
            if let Some(name) = op.name.as_deref() {
                *by_name.entry(name).or_insert(0) += 1;
            }
        }
        assert_eq!(by_name["Foo"], 2, "Foo appears twice");
        assert_eq!(by_name["Bar"], 1, "Bar appears once");
    }
}
