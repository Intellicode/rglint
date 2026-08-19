//! The rule registry and shared helpers.
//!
//! [`all_rules`] returns the static slice of every rule struct annotated with
//! `#[derive(Rule)]` across this crate, aggregated by `linkme` into
//! `rglint_core::ALL_RULES`. The engine (spec-011) iterates this slice to build
//! configured handlers; docs generation (PLAN §8) reads `meta()` from each
//! entry.
//!
//! # Example
//!
//! A struct annotated with `#[derive(Rule)]` is automatically discoverable
//! through `all_rules()`:
//!
//! ```
//! use rglint_core::{Handler, Rule, RuleContext};
//! use rglint_derive::Rule as DeriveRule;
//!
//! #[derive(DeriveRule)]
//! #[rule(id = "__rg_doctest_rule", category = "operations", severity = "error")]
//! struct MyRule;
//!
//! impl MyRule {
//!     fn handler(
//!         &self,
//!         _ctx: &mut RuleContext,
//!     ) -> Box<dyn Handler> {
//!         Box::new(Noop)
//!     }
//! }
//!
//! struct Noop;
//! impl Handler for Noop {}
//!
//! # // the test harness links `linkme` submissions from this doctest's
//! # // compilation unit too, so the entry is discoverable:
//! let entry = rglint_rules::all_rules()
//!     .iter()
//!     .find(|e| e.meta.id == "__rg_doctest_rule")
//!     .expect("derive-annotated rule should be registered");
//! assert_eq!(entry.meta.id, "__rg_doctest_rule");
//! assert_eq!(entry.meta.severity, rglint_core::Severity::Error);
//! let rule = (entry.factory)();
//! assert_eq!(rule.meta().id, "__rg_doctest_rule");
//! ```

#![allow(dead_code)]

pub mod meta;
pub mod operations;
pub mod schema;
pub mod shared;

pub use rglint_core::{Category, Handler, Rule, RuleEntry, RuleMeta};

/// The complete registry of built-in rules, aggregated via `linkme`.
///
/// Iteration is zero-cost: the slice is a linker-populated static. Rules that
/// add `#[derive(Rule)]` appear here automatically; rules that don't, don't
/// (see the negative test below).
pub fn all_rules() -> &'static [RuleEntry] {
    &rglint_core::ALL_RULES
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rglint_core::{Handler, RuleContext};
    use rglint_derive::Rule as DeriveRule;

    use crate::all_rules;

    /// A rule annotated with `#[derive(Rule)]`; must be discoverable through
    /// `all_rules()`.
    #[derive(DeriveRule)]
    #[rule(id = "__rg_test_derived_rule", category = "other")]
    struct DerivedRule;

    impl DerivedRule {
        fn handler(&self, _ctx: &mut RuleContext) -> std::boxed::Box<dyn Handler> {
            std::boxed::Box::new(NoopHandler)
        }
    }

    /// A rule *not* annotated with `#[derive(Rule)]`; must NOT appear in
    /// `all_rules()` (verifies the derive is what drives registration).
    struct UnregisteredRule;

    impl UnregisteredRule {
        fn handler(&self, _ctx: &mut RuleContext) -> std::boxed::Box<dyn Handler> {
            std::boxed::Box::new(NoopHandler)
        }
    }

    impl rglint_core::Rule for UnregisteredRule {
        fn meta(&self) -> &'static rglint_core::RuleMeta {
            static META: rglint_core::RuleMeta = rglint_core::RuleMeta::new(
                "__rg_test_unregistered_rule",
                rglint_core::Category::Other,
                rglint_core::Severity::Warn,
                "",
                None,
                None,
                false,
                false,
                false,
                None,
                false,
            );
            &META
        }
        fn create(&self, ctx: &mut RuleContext) -> std::boxed::Box<dyn Handler> {
            UnregisteredRule::handler(self, ctx)
        }
    }

    struct NoopHandler;
    impl Handler for NoopHandler {}

    #[test]
    fn derived_rule_is_registered() {
        let entry = all_rules()
            .iter()
            .find(|e| e.meta.id == "__rg_test_derived_rule")
            .unwrap_or_else(|| panic!("derived rule not in all_rules()"));
        assert_eq!(entry.meta.id, "__rg_test_derived_rule");
        assert_eq!(entry.meta.category, rglint_core::Category::Other);
        assert_eq!(entry.meta.severity, rglint_core::Severity::Warn);
        assert!(entry.interested_kinds.is_empty());
        let rule = (entry.factory)();
        assert_eq!(rule.meta().id, "__rg_test_derived_rule");
    }

    #[test]
    fn unregistered_rule_is_not_in_registry() {
        let present = all_rules()
            .iter()
            .any(|e| e.meta.id == "__rg_test_unregistered_rule");
        assert!(
            !present,
            "UnregisteredRule appeared in all_rules() without #[derive(Rule)]"
        );
    }

    #[test]
    fn registry_is_nonempty() {
        // The derive above contributes at least one entry; guard against the
        // linker accidentally dropping the linkme section.
        assert!(!all_rules().is_empty());
    }

    #[test]
    fn registered_rule_fixtures_pass_in_library_test_binary() {
        // Exercise the registered handlers together in the library test binary.
        // Besides checking the merged registry boundary, this keeps coverage
        // instrumentation from losing rule executions split across many small
        // integration-test binaries.
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("rules-fixtures");
        let mut suites_run = 0;
        let mut failures = Vec::new();

        for entry in all_rules() {
            let rule_id = entry.meta.id;
            let suite_root = fixture_root.join(rule_id);
            if rule_id.starts_with("__") || !suite_root.is_dir() {
                continue;
            }
            suites_run += 1;
            for (case_id, error) in rglint_test_harness::run_suite(rule_id, &suite_root) {
                failures.push(format!("{rule_id}/{case_id}: {error}"));
            }
        }

        assert!(
            suites_run > 0,
            "no registered rule fixture suites were found"
        );
        assert!(
            failures.is_empty(),
            "registered rule fixture failures:\n{}",
            failures.join("\n")
        );
    }
}
