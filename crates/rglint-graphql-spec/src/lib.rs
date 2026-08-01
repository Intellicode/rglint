//! GraphQL specification validation rules backed by `apollo-compiler`.
//!
//! The upstream graphql-eslint plugin exposes one rule id per graphql-js
//! validation function. Apollo already performs the validation, so this crate
//! adapts its diagnostics into the rglint rule registry and keeps the rule-id
//! filtering at the boundary.

mod names;
mod spec_rules;

pub use names::rule_id_for;
pub use spec_rules::{all_spec_rules, SpecRule};
