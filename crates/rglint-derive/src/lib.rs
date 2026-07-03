//! Procedural macros for rglint.
//!
//! Currently provides `#[derive(Rule)]` (spec-008), which emits:
//!
//! - a `static RULE_META_…: RuleMeta` built with `RuleMeta::new`,
//! - an `impl Rule for …` whose `meta()` borrows that static and whose
//!   `create()` forwards to an *inherent* `handler(&self, ctx) -> Box<dyn
//!   Handler>` method the rule author writes on the struct (separate name so
//!   it cannot shadow / recurse into the trait method), and
//! - a `#[linkme::distributed_slice(rglint_core::ALL_RULES)]` submission so
//!   `rglint_rules::all_rules()` discovers the rule with zero manual list
//!   maintenance.

mod rule_derive;

#[proc_macro_derive(Rule, attributes(rule))]
pub fn rule_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    rule_derive::rule_derive_impl(input)
}
