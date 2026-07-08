//! `unique-fragment-name` (spec-017).
//!
//! Ports [`graphql-eslint`'s rule of the same id`]: across every sibling
//! document in a project, no two fragment definitions may share a name. This
//! is the first rule to declare `requires_siblings`, so it is also the first
//! to exercise the engine's `siblings` plumbing (spec-006 + spec-011).
//!
//! ## Detection strategy
//!
//! The rule does its work in [`Handler::finalize`], not on individual node
//! visits, because the violation is inherently project-wide: a fragment name
//! counts as a duplicate only relative to the rest of the project's sibling
//! documents. [`Siblings::fragments_all`] gives the iteration-ordered list of
//! every fragment occurrence including cross-file collisions (the
//! [`Siblings::fragments`] `HashMap`, in contrast, is last-wins and would
//! silently drop duplicates).
//!
//! For each fragment name we count its occurrences across the bundle; the
//! first occurrence is treated as canonical (matches `graphql-eslint`) and
//! every *subsequent* occurrence is reported on the file it lives in, at the
//! fragment definition's span, with the message:
//!
//! ```text
//! Fragment "${name}" is defined multiple times
//! ```
//!
//! ## Per-file attribution
//!
//! The engine calls `Handler::finalize` once per linted source file, with
//! [`RuleContext::report`] stamping every emitted diagnostic with that file's
//! path. We therefore filter `fragments_all` to occurrences whose own
//! [`FragmentDef::source`]'s path matches `ctx.source_code().path()` so each
//! file receives only the duplicates it actually contains (and the engine's
//! sort + reporter attribute them naturally).
//!
//! ## Sibling self-skip
//!
//! Declaring `requires_siblings = true` causes the engine (spec-011) to skip
//! this rule entirely when no sibling documents are loaded (a schema-only
//! lint). When the engine *does* dispatch us, `ctx.siblings` is `Some`; we
//! still guard defensively because the `Handler::finalize` default takes any
//! `&mut RuleContext` and a future caller (e.g. a unit test) might construct
//! one without siblings.
//!
//! [`graphql-eslint`'s rule of the same id`]: https://the-guild.dev/graphql/eslint/rules/unique-fragment-name

use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, RuleContext};
use rglint_derive::Rule;

/// The `unique-fragment-name` rule.
///
/// Registered into `rglint_core::ALL_RULES` via `#[derive(Rule)]` with
/// `category = "operations"` and `requires_siblings = true`. The rule has no
/// `kinds` subscription: every per-file handler does its work in `finalize`
/// against the project's sibling index, so the engine never dispatches
/// `on_node` to it (the default `Handler::on_node` no-op covers it).
#[derive(Rule)]
#[rule(
    id = "unique-fragment-name",
    category = "operations",
    requires_siblings = true
)]
pub struct UniqueFragmentName;

impl UniqueFragmentName {
    /// Per-document handler factory invoked by the engine (spec-011). The rule
    /// is stateless per file — every recurring computation is done in
    /// `finalize` against the sibling index — so the handler holds nothing
    /// and a single zero-state instance is reused across files.
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(UniqueFragmentNameHandler)
    }
}

/// The handler — no per-document state because all detection happens in
/// `finalize` from the shared sibling index.
struct UniqueFragmentNameHandler;

impl Handler for UniqueFragmentNameHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        // The engine should have skipped us for a siblings-less project, but
        // a defensive guard keeps a unit-test-constructed context from
        // panicking.
        let Some(siblings) = ctx.siblings else {
            return;
        };
        let fragments_all = siblings.fragments_all();
        if fragments_all.is_empty() {
            return;
        }

        // Count occurrences per fragment name across the whole project so we
        // can decide whether a given occurrence is a duplicate-after-first.
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for frag in fragments_all {
            *counts.entry(frag.name.as_str()).or_insert(0) += 1;
        }

        // Second pass: a per-name occurrence index lets us distinguish the
        // canonical first occurrence from later duplicates. We only report
        // occurrences that (a) live in *this* linted file and (b) come after
        // the first one AND (c) belong to a name with >1 occurrence in total.
        let mut occurrence: HashMap<&str, usize> = HashMap::new();
        let this_file = ctx.source_code().path().to_path_buf();
        for frag in fragments_all {
            let idx = occurrence.entry(frag.name.as_str()).or_insert(0);
            *idx += 1;
            let total = *counts.get(frag.name.as_str()).unwrap_or(&0);
            if total <= 1 {
                continue;
            }
            // First occurrence (canonical) is not reported; every later one is.
            if *idx == 1 {
                continue;
            }
            // Attribute the diagnostic to the file the duplicate's source
            // points at. Only emit when that file matches the file currently
            // being finalized (the engine's per-file model stamps its own
            // file onto reported diagnostics via `RuleContext::report`).
            if frag.source.path() != this_file {
                continue;
            }
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                frag.span,
                format!("Fragment \"{}\" is defined multiple times", frag.name),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Light unit tests on the handler's wiring; the parity suite
    //! (`rglint_test_suite!`, see `tests/rule_unique_fragment_name.rs`) is
    //! the authoritative check against `graphql-eslint` fixtures.

    use super::*;
    use rglint_core::{Category, Rule, Severity};

    /// The rule's static metadata matches spec-017 (id + category +
    /// `requires_siblings`). `has_suggestions` is `false` per the spec.
    #[test]
    fn rule_meta_matches_spec_017() {
        let rule = UniqueFragmentName;
        let meta = rule.meta();
        assert_eq!(meta.id, "unique-fragment-name");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    /// Without siblings the finalize is a no-op (defensive; the engine would
    /// have skipped us, but this guards a manual `RuleContext`).
    #[test]
    fn finalize_is_noop_without_siblings() {
        use rglint_core::{ProjectConfig, SourceFile};
        use std::path::PathBuf;
        use std::sync::Arc;

        let src = Arc::new(SourceFile::new(
            PathBuf::from("a.graphql"),
            "fragment A on U { a }".to_owned(),
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
            "unique-fragment-name",
            Severity::Warn,
        );
        let mut h = UniqueFragmentNameHandler;
        h.finalize(&mut ctx);
        assert!(ctx.take_diagnostics().is_empty());
    }

    /// `fragments_all` preserves duplicates so the rule can detect them (this
    /// pins the `Siblings` contract spec-017 depends on).
    #[test]
    fn siblings_exposes_duplicate_fragment_defs_via_fragments_all() {
        use rglint_core::{DocumentLoader, DocumentSpec};
        use std::collections::HashMap;
        use std::path::Path;

        let loader = DocumentLoader::new();
        let a = "fragment X on U { a }".to_owned();
        let b = "fragment X on V { b }".to_owned();
        let c = "fragment Y on W { c }".to_owned();
        // Each document separate so a parse builds an isolated executable doc.
        let mut docs = loader
            .load(
                &DocumentSpec::Inline(a.clone()),
                Path::new("a.graphql"),
                None,
            )
            .expect("load a");
        let more_b = loader
            .load(&DocumentSpec::Inline(b), Path::new("b.graphql"), None)
            .expect("load b");
        let more_c = loader
            .load(&DocumentSpec::Inline(c), Path::new("c.graphql"), None)
            .expect("load c");
        docs.docs.extend(more_b.docs);
        docs.docs.extend(more_c.docs);
        // `by_file` isn't important to this test; the engine's attribution for
        // synthesized inline docs is what it is.
        let _ = docs.by_file;

        let siblings = rglint_core::Siblings::from_documents(&docs);
        let mut by_name: HashMap<&str, usize> = HashMap::new();
        for f in siblings.fragments_all() {
            *by_name.entry(f.name.as_str()).or_insert(0) += 1;
        }
        assert_eq!(by_name["X"], 2, "X appears twice");
        assert_eq!(by_name["Y"], 1, "Y appears once");
    }
}
